//! One admitted source pass and one aggregate typed pass over disjoint spans.
//! Retained output uses the existing standard DTO layout and response charges.

use crate::{BidderError, BidderErrorKind};
use auctioneer_openrtb::ext::bid::ExtBid;
use auctioneer_openrtb::types::bid_response::{Bid, BidResponse, SeatBid};
use auctioneer_runtime::bounded_json::{
    BoundedJsonError, ProprietaryJsonError, SharedDecodeBudget,
    from_proprietary_source_seed_within, from_slice_shared_seed_within, from_slice_shared_within,
};
use serde::Deserialize;
use serde::de::value::{MapAccessDeserializer, StringDeserializer};
use serde::de::{DeserializeSeed, Error as DeError, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::value::RawValue;
use serde_json::{Map, Value};
use std::fmt;
use std::marker::PhantomData;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum CandidateFailure {
    Syntax,
    Nodes,
    RawCapture,
    Bytes,
    Depth,
}

/// Only the existing standard Bid tree is admitted. These policies change
/// extension presence/defaults, never the target layout or scalar coercions.
#[derive(Clone, Copy, Debug)]
pub(crate) enum CandidatePolicy {
    OptionalExtension,
    RequiredExtensionNullDefault,
}

#[derive(Debug)]
pub(crate) struct DecodedSource {
    response: BidResponse,
    failures: Vec<CandidateFailure>,
    #[cfg(test)]
    source_nodes: u32,
    #[cfg(test)]
    remaining_nodes: u32,
    #[cfg(test)]
    remaining_bytes: usize,
    #[cfg(test)]
    precharged_work_bytes: usize,
}

impl DecodedSource {
    pub(crate) fn into_parts(self) -> (BidResponse, Vec<BidderError>) {
        // Expand diagnostics only after the entire source commits. Failed
        // sources retain neither bids, extension trees nor candidate errors.
        let errors = self
            .failures
            .into_iter()
            .map(CandidateFailure::diagnostic)
            .collect();
        (self.response, errors)
    }
}

#[cfg(test)]
pub(super) fn decode_source(
    bytes: &[u8],
    max_nodes: u32,
) -> Result<DecodedSource, Vec<BidderError>> {
    decode_source_with_policy(
        bytes,
        max_nodes,
        CandidatePolicy::OptionalExtension,
        |_bid| true,
    )
}

pub(crate) fn decode_source_with_policy(
    bytes: &[u8],
    max_nodes: u32,
    policy: CandidatePolicy,
    prefilter: fn(&Bid) -> bool,
) -> Result<DecodedSource, Vec<BidderError>> {
    // All work is reserved before lexical admission. Candidate spans are
    // disjoint and share the remaining one-source-length typed allowance.
    let (work, typed_bytes) = reserve_work(bytes.len())?;
    let _: usize = work;
    let mut session = Session {
        typed: SharedDecodeBudget::new(max_nodes, typed_bytes),
        policy,
        prefilter,
        source_failure: None,
        failures: Vec::new(),
    };
    let parsed = from_proprietary_source_seed_within(
        bytes,
        max_nodes,
        ResponseSeed {
            session: &mut session,
        },
    );
    let (source_nodes, response) = parsed.map_err(|error| {
        vec![
            session
                .source_failure
                .unwrap_or_else(|| failure_kind(&error))
                .diagnostic(),
        ]
    })?;
    let failure_count = u32::try_from(session.failures.len()).map_err(|_error| budget_errors())?;
    let failure_bytes = failure_storage_bound(session.failures.len())?;
    if failure_count > source_nodes || session.failures.capacity() > failure_bytes {
        return Err(budget_errors());
    }
    Ok(DecodedSource {
        response,
        failures: session.failures,
        #[cfg(test)]
        source_nodes,
        #[cfg(test)]
        remaining_nodes: session.typed.remaining_nodes(),
        #[cfg(test)]
        remaining_bytes: session.typed.remaining_bytes(),
        #[cfg(test)]
        precharged_work_bytes: work,
    })
}

fn reserve_work(bytes: usize) -> Result<(usize, usize), Vec<BidderError>> {
    // Input-byte scan credits, not instruction counts: lexical B + source B +
    // captured-span UTF-8 R + typed R, where disjoint raw spans give R <= B.
    let work = bytes.checked_mul(4).ok_or_else(budget_errors)?;
    let source_work = bytes.checked_mul(3).ok_or_else(budget_errors)?;
    let typed_bytes = work.checked_sub(source_work).ok_or_else(budget_errors)?;
    Ok((work, typed_bytes))
}

fn failure_storage_bound(failures: usize) -> Result<usize, Vec<BidderError>> {
    // One byte per refusal. Geometric Vec growth retains at most eight bytes
    // initially, then old + new allocations fit 3F; lexical admission gives F <= N.
    failures
        .checked_mul(3)
        .map(|bytes| bytes.max(8))
        .ok_or_else(budget_errors)
}

fn budget_errors() -> Vec<BidderError> {
    vec![CandidateFailure::Bytes.diagnostic()]
}

fn failure_kind(error: &ProprietaryJsonError) -> CandidateFailure {
    match error {
        ProprietaryJsonError::ByteBudget => CandidateFailure::Bytes,
        ProprietaryJsonError::DepthBudget => CandidateFailure::Depth,
        ProprietaryJsonError::Decode(BoundedJsonError::Syntax(_)) => CandidateFailure::Syntax,
        ProprietaryJsonError::Decode(BoundedJsonError::NodeBudget(_)) => CandidateFailure::Nodes,
        ProprietaryJsonError::Decode(BoundedJsonError::RawValue(_)) => CandidateFailure::RawCapture,
    }
}

impl CandidateFailure {
    fn diagnostic(self) -> BidderError {
        let mut error = match self {
            Self::Syntax => BidderError::failed_to_unmarshal("invalid bidder response"),
            Self::Nodes | Self::RawCapture | Self::Bytes | Self::Depth => {
                BidderError::bad_server_response("bidder response exceeded the decode budget")
            }
        };
        error.kind = BidderErrorKind::ResponseParse;
        error
    }
}

struct Session {
    typed: SharedDecodeBudget,
    policy: CandidatePolicy,
    prefilter: fn(&Bid) -> bool,
    // Only a parser-owned observation may classify a source visitor failure as
    // a structural refusal. Upstream text never establishes that provenance.
    source_failure: Option<CandidateFailure>,
    // Allocated only on a candidate refusal after lexical/depth admission;
    // neither failures nor successful bids escape a refused whole source.
    failures: Vec<CandidateFailure>,
}

impl Session {
    fn candidate<E: DeError>(&mut self, raw: &RawValue) -> Result<Option<Bid>, E> {
        let mut extension_failure = false;
        let parsed = from_slice_shared_seed_within(
            raw.get().as_bytes(),
            &mut self.typed,
            CandidateSeed {
                policy: self.policy,
                prefilter: self.prefilter,
                extension_failure: &mut extension_failure,
            },
        );
        match parsed {
            Ok(bid) => Ok(Some(bid)),
            Err(error) => {
                let failure = failure_kind(&error);
                if extension_failure && failure == CandidateFailure::Syntax {
                    self.failures.push(failure);
                    Ok(None)
                } else {
                    // Standard DTO errors and every structural refusal roll
                    // back the whole source, regardless of earlier successes.
                    self.source_failure = Some(failure);
                    Err(E::custom("invalid bidder response candidate"))
                }
            }
        }
    }

    fn field<E: DeError>(&mut self, raw: &RawValue) -> Result<Value, E> {
        from_slice_shared_within(raw.get().as_bytes(), &mut self.typed).map_err(|error| {
            self.source_failure = Some(failure_kind(&error));
            E::custom("invalid bidder response field")
        })
    }
}

struct CandidateSeed<'a> {
    policy: CandidatePolicy,
    prefilter: fn(&Bid) -> bool,
    extension_failure: &'a mut bool,
}

impl<'de> DeserializeSeed<'de> for CandidateSeed<'_> {
    type Value = Bid;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "deserialize_map admits only a candidate object; Serde defaults refuse other shapes"
)]
impl<'de> Visitor<'de> for CandidateSeed<'_> {
    type Value = Bid;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bidder response candidate object")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        // Standard fields still use Bid's derive. The extension is moved from
        // the same counted stream, never reparsed or materialized as a Value.
        let mut extension_present = false;
        let mut extension = None;
        let mut bid = Bid::deserialize(MapAccessDeserializer::new(CandidateMap {
            inner: map,
            seen_extension: false,
            extension_present: &mut extension_present,
            extension_failure: &mut *self.extension_failure,
            extension: &mut extension,
        }))
        .inspect_err(|_error| {
            // Includes missing/duplicate standard fields reported by Bid after
            // the map returns, not only errors from a standard value seed.
            *self.extension_failure = false;
        })?;
        bid.ext = extension;
        if !(self.prefilter)(&bid) {
            *self.extension_failure = false;
            return Err(A::Error::custom("invalid bidder response standard fields"));
        }
        if *self.extension_failure {
            return Err(A::Error::custom("invalid bidder response extension"));
        }
        if matches!(self.policy, CandidatePolicy::RequiredExtensionNullDefault) {
            if !extension_present {
                *self.extension_failure = true;
                return Err(A::Error::missing_field("ext"));
            }
            // Option<ExtBid> normally collapses absence and null. Only this
            // closed policy restores present-null to the source's empty DTO.
            bid.ext.get_or_insert_default();
        }
        Ok(bid)
    }
}

struct CandidateMap<'a, A> {
    inner: A,
    seen_extension: bool,
    extension_present: &'a mut bool,
    extension_failure: &'a mut bool,
    extension: &'a mut Option<ExtBid>,
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for CandidateMap<'_, A> {
    type Error = A::Error;

    fn next_key_seed<S>(&mut self, seed: S) -> Result<Option<S::Value>, Self::Error>
    where
        S: DeserializeSeed<'de>,
    {
        while let Some(key) = self.inner.next_key::<String>()? {
            if key.eq_ignore_ascii_case("ext") {
                if self.seen_extension {
                    *self.extension_failure = true;
                    let _ignored: IgnoredAny = self.inner.next_value()?;
                    continue;
                }
                self.seen_extension = true;
            }
            if key == "ext" {
                *self.extension_present = true;
                *self.extension = self.inner.next_value_seed(ExtensionSeed {
                    failure: &mut *self.extension_failure,
                })?;
                continue;
            }
            // The counted map already charged this key. Replay only the owned
            // string to Bid's field seed, without a JSON read or case rewrite.
            return seed
                .deserialize(StringDeserializer::<A::Error>::new(key))
                .map(Some);
        }
        Ok(None)
    }

    fn next_value_seed<S>(&mut self, seed: S) -> Result<S::Value, Self::Error>
    where
        S: DeserializeSeed<'de>,
    {
        self.inner.next_value_seed(seed)
    }

    fn next_key<K>(&mut self) -> Result<Option<K>, Self::Error>
    where
        K: Deserialize<'de>,
    {
        self.next_key_seed(PhantomData)
    }

    fn next_value<V>(&mut self) -> Result<V, Self::Error>
    where
        V: Deserialize<'de>,
    {
        self.next_value_seed(PhantomData)
    }

    fn next_entry_seed<K, V>(
        &mut self,
        kseed: K,
        vseed: V,
    ) -> Result<Option<(K::Value, V::Value)>, Self::Error>
    where
        K: DeserializeSeed<'de>,
        V: DeserializeSeed<'de>,
    {
        match self.next_key_seed(kseed)? {
            Some(key) => self.next_value_seed(vseed).map(|value| Some((key, value))),
            None => Ok(None),
        }
    }

    fn next_entry<K, V>(&mut self) -> Result<Option<(K, V)>, Self::Error>
    where
        K: Deserialize<'de>,
        V: Deserialize<'de>,
    {
        self.next_entry_seed(PhantomData, PhantomData)
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

struct ExtensionSeed<'a> {
    failure: &'a mut bool,
}

impl<'de> DeserializeSeed<'de> for ExtensionSeed<'_> {
    type Value = Option<ExtBid>;

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        match Option::<ExtBid>::deserialize(CompleteDeserializer(deserializer)) {
            Ok(extension) => Ok(extension),
            Err(_error) => {
                // Counted structural refusals remain sticky in the surrounding
                // shared ledger and take priority when CandidateSeed fails.
                *self.failure = true;
                Ok(None)
            }
        }
    }
}

// Complete only the failing extension's unread tail in its original counted
// stream. Wrapping visitor results lets JSON consume closing delimiters before
// the type refusal propagates. No source spans, extra tree or new budget exist.
struct CompleteDeserializer<D>(D);
struct CompleteVisitor<V>(V);
struct CompleteSeed<S>(S);
struct CompleteMap<A> {
    inner: A,
    pending_value: bool,
}
struct CompleteSeq<A>(A);

impl<'de, S: DeserializeSeed<'de>> DeserializeSeed<'de> for CompleteSeed<S> {
    type Value = S::Value;

    fn deserialize<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        self.0.deserialize(CompleteDeserializer(deserializer))
    }
}

impl<'de, D: serde::Deserializer<'de>> serde::Deserializer<'de> for CompleteDeserializer<D> {
    type Error = D::Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        // Dispatch by actual shape so a type mismatch cannot leave an unread
        // container behind. The unchanged DTO visitor decides scalar validity.
        self.0
            .deserialize_any(CompleteVisitor(visitor))?
            .ok_or_else(|| Self::Error::custom("invalid bidder response extension"))
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.0
            .deserialize_option(CompleteVisitor(visitor))?
            .ok_or_else(|| Self::Error::custom("invalid bidder response extension"))
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.0
            .deserialize_newtype_struct(name, CompleteVisitor(visitor))?
            .ok_or_else(|| Self::Error::custom("invalid bidder response extension"))
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf unit unit_struct seq tuple tuple_struct map struct enum
        identifier ignored_any
    }

    fn is_human_readable(&self) -> bool {
        self.0.is_human_readable()
    }
}

macro_rules! complete_scalar {
    ($($method:ident($value:ident: $ty:ty)),* $(,)?) => {
        $(fn $method<E: DeError>(self, $value: $ty) -> Result<Self::Value, E> {
            Ok(self.0.$method::<E>($value).ok())
        })*
    };
}

impl<'de, V: Visitor<'de>> Visitor<'de> for CompleteVisitor<V> {
    type Value = Option<V::Value>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.expecting(formatter)
    }

    complete_scalar! {
        visit_bool(value: bool),
        visit_i8(value: i8), visit_i16(value: i16), visit_i32(value: i32),
        visit_i64(value: i64), visit_i128(value: i128),
        visit_u8(value: u8), visit_u16(value: u16), visit_u32(value: u32),
        visit_u64(value: u64), visit_u128(value: u128),
        visit_f32(value: f32), visit_f64(value: f64), visit_char(value: char),
        visit_str(value: &str), visit_borrowed_str(value: &'de str),
        visit_string(value: String), visit_bytes(value: &[u8]),
        visit_borrowed_bytes(value: &'de [u8]), visit_byte_buf(value: Vec<u8>),
    }

    fn visit_none<E: DeError>(self) -> Result<Self::Value, E> {
        Ok(self.0.visit_none::<E>().ok())
    }

    fn visit_unit<E: DeError>(self) -> Result<Self::Value, E> {
        Ok(self.0.visit_unit::<E>().ok())
    }

    fn visit_some<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        Ok(self.0.visit_some(CompleteDeserializer(deserializer)).ok())
    }

    fn visit_newtype_struct<D: serde::Deserializer<'de>>(
        self,
        deserializer: D,
    ) -> Result<Self::Value, D::Error> {
        Ok(self
            .0
            .visit_newtype_struct(CompleteDeserializer(deserializer))
            .ok())
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
        let mut completed = CompleteMap {
            inner: map,
            pending_value: false,
        };
        let result = self.0.visit_map(&mut completed);
        if completed.pending_value {
            let _ignored: IgnoredAny = completed.inner.next_value()?;
        }
        while completed
            .inner
            .next_entry::<IgnoredAny, IgnoredAny>()?
            .is_some()
        {}
        Ok(result.ok())
    }

    fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<Self::Value, A::Error> {
        let mut completed = CompleteSeq(seq);
        let result = self.0.visit_seq(&mut completed);
        while completed.0.next_element::<IgnoredAny>()?.is_some() {}
        Ok(result.ok())
    }
}

impl<'de, A: MapAccess<'de>> MapAccess<'de> for CompleteMap<A> {
    type Error = A::Error;

    fn next_key_seed<S: DeserializeSeed<'de>>(
        &mut self,
        seed: S,
    ) -> Result<Option<S::Value>, Self::Error> {
        let key = self.inner.next_key_seed(seed)?;
        self.pending_value = key.is_some();
        Ok(key)
    }

    fn next_value_seed<S: DeserializeSeed<'de>>(
        &mut self,
        seed: S,
    ) -> Result<S::Value, Self::Error> {
        self.pending_value = false;
        self.inner.next_value_seed(CompleteSeed(seed))
    }

    fn size_hint(&self) -> Option<usize> {
        self.inner.size_hint()
    }
}

impl<'de, A: SeqAccess<'de>> SeqAccess<'de> for CompleteSeq<A> {
    type Error = A::Error;

    fn next_element_seed<S: DeserializeSeed<'de>>(
        &mut self,
        seed: S,
    ) -> Result<Option<S::Value>, Self::Error> {
        self.0.next_element_seed(CompleteSeed(seed))
    }

    fn size_hint(&self) -> Option<usize> {
        self.0.size_hint()
    }
}

fn recognized<E: DeError>(seen: &mut u8, bit: u8, field: &'static str) -> Result<(), E> {
    if *seen & bit != 0 {
        return Err(E::duplicate_field(field));
    }
    *seen |= bit;
    Ok(())
}

struct ResponseSeed<'a> {
    session: &'a mut Session,
}

impl<'de> DeserializeSeed<'de> for ResponseSeed<'_> {
    type Value = BidResponse;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "deserialize_map admits only an object; Serde defaults refuse other shapes"
)]
impl<'de> Visitor<'de> for ResponseSeed<'_> {
    type Value = BidResponse;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bidder response object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut response = BidResponse {
            id: String::new(),
            seatbid: Vec::new(),
            nbr: None,
            cur: String::new(),
            ext: None,
            other: Map::new(),
        };
        let mut seen = 0;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "id" => {
                    recognized(&mut seen, 1, "id")?;
                    response.id = map.next_value()?;
                }
                "seatbid" => {
                    recognized(&mut seen, 2, "seatbid")?;
                    response.seatbid = map.next_value_seed(SeatsSeed {
                        session: self.session,
                    })?;
                }
                "nbr" => {
                    recognized(&mut seen, 4, "nbr")?;
                    response.nbr = map.next_value()?;
                }
                "cur" => {
                    recognized(&mut seen, 8, "cur")?;
                    response.cur = map.next_value()?;
                }
                "ext" => {
                    recognized(&mut seen, 16, "ext")?;
                    let raw: &RawValue = map.next_value()?;
                    let value = self.session.field(raw)?;
                    response.ext = (!value.is_null()).then_some(value);
                }
                _ => {
                    let raw: &RawValue = map.next_value()?;
                    let value = self.session.field(raw)?;
                    let _replaced = response.other.insert(key, value);
                }
            }
        }
        if seen & 1 == 0 {
            return Err(A::Error::missing_field("id"));
        }
        Ok(response)
    }
}

struct SeatsSeed<'a> {
    session: &'a mut Session,
}

impl<'de> DeserializeSeed<'de> for SeatsSeed<'_> {
    type Value = Vec<SeatBid>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "deserialize_seq admits only the seat array; Serde defaults refuse other shapes"
)]
impl<'de> Visitor<'de> for SeatsSeed<'_> {
    type Value = Vec<SeatBid>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bidder response seat array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut seats = Vec::new();
        while let Some(seat) = seq.next_element_seed(SeatSeed {
            session: self.session,
        })? {
            seats.push(seat);
        }
        Ok(seats)
    }
}

struct SeatSeed<'a> {
    session: &'a mut Session,
}

impl<'de> DeserializeSeed<'de> for SeatSeed<'_> {
    type Value = SeatBid;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "deserialize_map admits only a seat object; Serde defaults refuse other shapes"
)]
impl<'de> Visitor<'de> for SeatSeed<'_> {
    type Value = SeatBid;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bidder response seat object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut seat = SeatBid {
            seat: String::new(),
            group: 0,
            bid: Vec::new(),
            other: Map::new(),
        };
        let mut seen = 0;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "seat" => {
                    recognized(&mut seen, 1, "seat")?;
                    seat.seat = map.next_value()?;
                }
                "group" => {
                    recognized(&mut seen, 2, "group")?;
                    seat.group = map.next_value()?;
                }
                "bid" => {
                    recognized(&mut seen, 4, "bid")?;
                    seat.bid = map.next_value_seed(BidsSeed {
                        session: self.session,
                    })?;
                }
                _ => {
                    let raw: &RawValue = map.next_value()?;
                    let value = self.session.field(raw)?;
                    let _replaced = seat.other.insert(key, value);
                }
            }
        }
        if seen & 4 == 0 {
            return Err(A::Error::missing_field("bid"));
        }
        Ok(seat)
    }
}

struct BidsSeed<'a> {
    session: &'a mut Session,
}

impl<'de> DeserializeSeed<'de> for BidsSeed<'_> {
    type Value = Vec<Bid>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "deserialize_seq admits only the bid array; Serde defaults refuse other shapes"
)]
impl<'de> Visitor<'de> for BidsSeed<'_> {
    type Value = Vec<Bid>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bidder response candidate array")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut bids = Vec::new();
        while let Some(raw) = seq.next_element::<&RawValue>()? {
            // No raw span survives this immediate typed call. Only successes
            // occupy Bid slots; compact refusals stay session-owned until commit.
            if let Some(bid) = self.session.candidate::<A::Error>(raw)? {
                bids.push(bid);
            }
        }
        Ok(bids)
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(target_arch = "wasm32"))]
    use super::DecodedSource;
    use super::{
        CandidateFailure, ResponseSeed, SeatSeed, SeatsSeed, decode_source, failure_storage_bound,
        reserve_work,
    };
    use crate::BidderErrorKind;
    use auctioneer_openrtb::types::bid_response::{Bid, BidResponse, SeatBid};
    #[cfg(not(target_arch = "wasm32"))]
    use auctioneer_runtime::bounded_json::from_slice_within;
    use auctioneer_runtime::bounded_json::{
        BoundedJsonError, ProprietaryJsonError, count_json_nodes,
    };
    use serde::de::DeserializeSeed;
    use serde::ser::SerializeMap as _;
    use serde::{Serialize, Serializer};
    use serde_json::value::RawValue;
    use serde_json::{Value, json};
    use std::any::TypeId;

    #[cfg(not(target_arch = "wasm32"))]
    use std::thread::spawn;

    #[cfg(not(target_arch = "wasm32"))]
    use auctioneer_runtime::test_support::alloc_track;

    #[cfg(not(target_arch = "wasm32"))]
    #[global_allocator]
    static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

    fn good_bid(id: &str) -> Value {
        json!({"id":id, "impid":"fake_imp", "price":1_i32})
    }

    fn invalid_extension_bid(id: &str) -> Value {
        let mut bid = good_bid(id);
        bid["ext"] = json!(false);
        bid
    }

    #[test]
    fn required_extension_policy_distinguishes_absence_from_null_in_the_typed_visitor() {
        use super::{CandidatePolicy, decode_source_with_policy};
        use auctioneer_runtime::bounded_json::{SharedDecodeBudget, from_slice_shared_seed_within};

        let mut present_null = good_bid("fake_null");
        present_null["ext"] = Value::Null;
        let mut extension_failure = false;
        let mut budget = SharedDecodeBudget::new(256, 256);
        let bytes = encode(&present_null);
        let candidate = from_slice_shared_seed_within(
            &bytes,
            &mut budget,
            super::CandidateSeed {
                policy: CandidatePolicy::RequiredExtensionNullDefault,
                prefilter: |_bid| true,
                extension_failure: &mut extension_failure,
            },
        )
        .expect("present-null typed candidate");
        assert!(candidate.ext.is_some());
        assert!(!extension_failure);
        let absent_bytes = encode(&good_bid("fake_absent"));
        let mut absent_failure = false;
        let absent = from_slice_shared_seed_within(
            &absent_bytes,
            &mut budget,
            super::CandidateSeed {
                policy: CandidatePolicy::RequiredExtensionNullDefault,
                prefilter: |_bid| true,
                extension_failure: &mut absent_failure,
            },
        );
        assert!(absent_failure, "missing extension provenance: {absent:?}");
        assert!(matches!(
            absent,
            Err(ProprietaryJsonError::Decode(BoundedJsonError::Syntax(_)))
        ));
        let document =
            json!({"id":"r", "seatbid":[{"bid":[good_bid("fake_absent"), present_null]}]});
        let decoded = decode_source_with_policy(
            &encode(&document),
            256,
            CandidatePolicy::RequiredExtensionNullDefault,
            |_bid| true,
        )
        .expect("local absence only");
        assert_eq!(decoded.failures, [CandidateFailure::Syntax]);
        let bids = &decoded.response.seatbid[0].bid;
        assert_eq!(bids.len(), 1);
        assert_eq!(bids[0].id, "fake_null");
        assert!(
            bids[0]
                .ext
                .as_ref()
                .expect("present-null empty default")
                .other
                .is_empty()
        );
    }

    #[test]
    fn standard_field_failures_do_not_inherit_the_last_extension_key_scope() {
        for fields in [
            r#""ext":{},"price":"fake_invalid""#,
            r#""ext":{},"price":1"#,
        ] {
            // The second candidate is missing impid. Neither error originated
            // in the preceding, successfully decoded extension field.
            let body = format!(r#"{{"id":"r","seatbid":[{{"bid":[{{"id":"b",{fields}}}]}}]}}"#);
            assert_source_failure(body.as_bytes(), 256, 14);
        }
    }

    #[test]
    fn extension_failures_never_mask_standard_failures_in_either_field_order() {
        for extension in [
            r#""ext":false"#,
            r#""ext":{"origbidcpm":{},"fake_tail":[1,2]}"#,
            r#""ext":{"prebid":{"targeting":{"fake_key":false},"fake_tail":[1,2]}}"#,
            r#""ext":{},"ext":{}"#,
            r#""EXT":{},"ext":{}"#,
        ] {
            for standard in [
                r#""id":false,"impid":"i","price":1"#,
                r#""id":"b","impid":[],"price":1"#,
                r#""id":"b","impid":"i","price":"bad""#,
                r#""id":"b","impid":"i","price":1,"w":false"#,
                r#""id":"b","impid":"i","price":1,"id":"duplicate""#,
                r#""id":"b","price":1"#,
            ] {
                for fields in [
                    format!("{extension},{standard}"),
                    format!("{standard},{extension}"),
                ] {
                    let body = format!(
                        r#"{{"id":"r","seatbid":[{{"bid":[{{"id":"first","impid":"i","price":1}},{{{fields}}},{{"id":"last","impid":"i","price":1}}]}}]}}"#
                    );
                    assert_source_failure(body.as_bytes(), 256, 14);
                }
            }
        }
    }

    #[test]
    fn completed_extension_seeds_preserve_standard_dto_scalar_rules() {
        use super::{CandidatePolicy, CandidateSeed};
        use auctioneer_runtime::bounded_json::{SharedDecodeBudget, from_slice_shared_seed_within};

        for extension in [
            json!(null),
            json!({"origbidcpm":u64::MAX,"origbidcur":"USD"}),
            json!({"prebid":{"dealpriority":i64::MIN,"dealtiersatisfied":true}}),
            json!({"prebid":{"dealpriority":i64::MAX,"targeting":{"key":"value"}}}),
            json!({"prebid":{"meta":{"nested":[1_i32,true,null]},"other":[1_i32,2_i32]}}),
            json!(false),
            json!([]),
            json!({"origbidcpm":"1"}),
            json!({"origbidcpm":[{"nested":[1_i32,2_i32]}]}),
            json!({"origbidcur":1_i32}),
            json!({"prebid":{"dealpriority":u64::MAX}}),
            json!({"prebid":{"dealpriority":1.0_f64}}),
            json!({"prebid":{"dealpriority":"1"}}),
            json!({"prebid":{"dealtiersatisfied":1_i32}}),
            json!({"prebid":{"targeting":{"key":false},"tail":[1_i32,2_i32]}}),
        ] {
            let mut document = good_bid("fake_bid");
            document["ext"] = extension;
            let bytes = encode(&document);
            let expected = serde_json::from_slice::<Bid>(&bytes);
            let nodes = count_json_nodes(&bytes, 256).unwrap();
            let mut budget = SharedDecodeBudget::new(nodes, bytes.len());
            let mut extension_failure = false;
            let actual = from_slice_shared_seed_within(
                &bytes,
                &mut budget,
                CandidateSeed {
                    policy: CandidatePolicy::OptionalExtension,
                    prefilter: |_bid| true,
                    extension_failure: &mut extension_failure,
                },
            );
            assert_eq!(actual.is_ok(), expected.is_ok(), "{document}");
            assert_eq!(extension_failure, expected.is_err(), "{document}");
            assert_eq!(budget.remaining_nodes(), 0, "{document}");
            assert_eq!(budget.remaining_bytes(), 0);
            if let (Ok(actual_bid), Ok(expected_bid)) = (actual, expected) {
                assert_eq!(
                    serde_json::to_value(actual_bid).unwrap(),
                    serde_json::to_value(expected_bid).unwrap()
                );
            }
        }
    }

    #[test]
    fn failed_extension_tails_and_duplicates_complete_once_on_the_shared_ledger() {
        use super::{CandidatePolicy, Session};
        use auctioneer_runtime::bounded_json::SharedDecodeBudget;

        for extension in [
            r#"{"origbidcpm":1,"origbidcpm":{"nested":[1,2]},"tail":[1,2]}"#,
            r#"{"prebid":{"targeting":{"first":"ok","bad":false,"last":[1,2]},"tail":[1,2]},"tail":[1,2]}"#,
        ] {
            let body = format!(r#"{{"ext":{extension},"id":"fake_bid","impid":"i","price":1}}"#);
            let nodes = count_json_nodes(body.as_bytes(), 256).unwrap();
            let raw: &RawValue = serde_json::from_str(&body).unwrap();
            for allowance in [nodes, nodes - 1] {
                let mut session = Session {
                    typed: SharedDecodeBudget::new(allowance, body.len()),
                    policy: CandidatePolicy::OptionalExtension,
                    prefilter: |_bid| true,
                    source_failure: None,
                    failures: Vec::new(),
                };
                let result: Result<_, serde_json::Error> = session.candidate(raw);
                assert_eq!(session.typed.remaining_nodes(), 0);
                assert_eq!(session.typed.remaining_bytes(), 0);
                if allowance == nodes {
                    assert!(result.unwrap().is_none());
                    assert_eq!(session.failures, [CandidateFailure::Syntax]);
                    assert!(session.source_failure.is_none());
                } else {
                    result.expect_err("source node exhaustion cannot become a local refusal");
                    assert!(session.failures.is_empty());
                    assert_eq!(session.source_failure, Some(CandidateFailure::Nodes));
                }
            }
        }
    }

    fn encode(document: &Value) -> Vec<u8> {
        serde_json::to_vec(document).expect("source fixture")
    }

    fn assert_source_failure(bytes: &[u8], nodes: u32, code: u32) {
        let errors = decode_source(bytes, nodes).expect_err("source failure rolls back all output");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        assert_eq!(
            errors[0].message,
            if code == 14 {
                "invalid bidder response"
            } else {
                "bidder response exceeded the decode budget"
            }
        );
    }

    #[test]
    fn standard_source_visitors_build_the_exact_funded_output_types() {
        // Check the seeds' actual outputs, not aliases that could drift apart.
        assert_eq!(
            TypeId::of::<<ResponseSeed<'static> as DeserializeSeed<'static>>::Value>(),
            TypeId::of::<BidResponse>(),
        );
        assert_eq!(
            TypeId::of::<<SeatSeed<'static> as DeserializeSeed<'static>>::Value>(),
            TypeId::of::<SeatBid>(),
        );
        assert_eq!(
            TypeId::of::<<SeatsSeed<'static> as DeserializeSeed<'static>>::Value>(),
            TypeId::of::<Vec<SeatBid>>(),
        );
        assert_eq!(
            TypeId::of::<<super::CandidateSeed<'static> as DeserializeSeed<'static>>::Value>(),
            TypeId::of::<Bid>(),
        );
    }

    #[test]
    fn proprietary_streaming_work_reserves_lexical_source_utf8_and_typed_visits() {
        let body = encode(&json!({"id":"r"}));
        assert_eq!(body.len(), 10);
        let decoded = decode_source(&body, 3).expect("narrow source");
        // Literal fixture expectation, independent of the reservation helper's
        // multipliers: lexical + structural + captured UTF-8 + typed credits.
        assert_eq!(decoded.precharged_work_bytes, 40);
        assert_eq!(decoded.remaining_bytes, 10);
        assert_eq!(decoded.remaining_nodes, 3);
    }

    #[test]
    fn proprietary_streaming_work_overflow_refuses_before_decoding() {
        let first_unfundable = usize::MAX
            .checked_div(4)
            .and_then(|bytes| bytes.checked_add(1))
            .expect("first length above four-credit arithmetic capacity");
        let errors =
            reserve_work(first_unfundable).expect_err("work overflow before source access");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 4);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
        assert_eq!(
            errors[0].message,
            "bidder response exceeded the decode budget"
        );
    }

    #[test]
    fn proprietary_streaming_failure_storage_is_compact_and_source_bounded() {
        let mut failures = Vec::new();
        assert_eq!(failures.capacity(), 0);
        for count in 1..=1025 {
            let previous_capacity = failures.capacity();
            failures.push(CandidateFailure::Syntax);
            let capacity = failures.capacity();
            let peak = if capacity == previous_capacity {
                capacity
            } else {
                previous_capacity
                    .checked_add(capacity)
                    .expect("growth peak")
            };
            assert!(peak <= failure_storage_bound(count).expect("compact source bound"));
        }
        assert_eq!(failure_storage_bound(0).expect("empty bound"), 8);
        let first_unfundable = usize::MAX
            .checked_div(3)
            .and_then(|count| count.checked_add(1))
            .expect("first overflowing failure count");
        let errors = failure_storage_bound(first_unfundable).expect_err("checked source bound");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 4);
        assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);

        let body = encode(&json!({"id":"fake_response", "seatbid":[{"bid":[]}]}));
        let decoded = decode_source(&body, u32::MAX).expect("no attached-cap preallocation");
        assert!(decoded.failures.is_empty());
        assert_eq!(decoded.failures.capacity(), 0);
        assert_eq!(decoded.response.seatbid[0].bid.capacity(), 0);
    }

    #[test]
    fn proprietary_streaming_failure_kinds_keep_fixed_diagnostics() {
        for (failure, code, message) in [
            (CandidateFailure::Syntax, 14, "invalid bidder response"),
            (
                CandidateFailure::Nodes,
                4,
                "bidder response exceeded the decode budget",
            ),
            (
                CandidateFailure::RawCapture,
                4,
                "bidder response exceeded the decode budget",
            ),
            (
                CandidateFailure::Bytes,
                4,
                "bidder response exceeded the decode budget",
            ),
            (
                CandidateFailure::Depth,
                4,
                "bidder response exceeded the decode budget",
            ),
        ] {
            let error = failure.diagnostic();
            assert_eq!(error.code, code);
            assert_eq!(error.kind, BidderErrorKind::ResponseParse);
            assert_eq!(error.message, message);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn measured_source(document: &Value, standard_valid: bool) -> DecodedSource {
        use auctioneer_runtime::budget::{MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR};

        let body = encode(document);
        assert!(
            body.len() <= 256 * 1024,
            "fixture must fit the shipped decoded cap"
        );
        let nodes = count_json_nodes(&body, 1 << 16).expect("fixture fits the shipped node cap");
        let wire = u64::try_from(body.len()).expect("fixture decoded bytes");
        let charge = wire
            .checked_mul(MIN_RESPONSE_PARSE_FACTOR)
            .and_then(|bytes| {
                let node_bytes = u64::from(nodes).checked_mul(MIN_RESPONSE_NODE_PEAK_BYTES)?;
                bytes.checked_add(node_bytes)
            })
            .expect("unchanged installed response charge");
        let standard = from_slice_within::<BidResponse>(&body, nodes);
        assert_eq!(standard.is_ok(), standard_valid);
        drop(standard);

        let baseline = alloc_track::reset_peak();
        let decoded =
            decode_source(&body, nodes).expect("valid source with isolated candidate failures");
        let peak = alloc_track::peak_since(baseline);
        assert!(peak > 0, "allocator must observe source output");
        let margin = peak
            .checked_mul(6)
            .and_then(|bytes| bytes.checked_add(4))
            .and_then(|bytes| bytes.checked_div(5))
            .expect("twenty percent margin");
        assert!(
            margin <= charge,
            "wire={wire} nodes={nodes} peak={peak} margin={margin} charge={charge}"
        );
        decoded
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn proprietary_streaming_allocation_covers_primitive_only_candidate_arrays() {
        use auctioneer_runtime::budget::{MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR};

        for primitive in [
            json!(0_i32),
            json!(null),
            json!(true),
            json!("x".repeat(64)),
        ] {
            for count in [1_usize, 4, 5, 32, 33, 256, 257, 1024, 1025] {
                let candidate_primitive = primitive.clone();
                spawn(move || {
                    let document =
                        json!({"id":"r", "seatbid":[{"bid":vec![candidate_primitive.clone(); count]}]});
                    let bytes = encode(&document);
                    let nodes = count_json_nodes(&bytes, 1 << 16).expect("source nodes");
                    let baseline = alloc_track::reset_peak();
                    let errors = decode_source(&bytes, nodes).expect_err("nonobject Bid is source-fatal");
                    let peak = alloc_track::peak_since(baseline);
                    let charge = u64::try_from(bytes.len()).expect("wire bytes")
                        .checked_mul(MIN_RESPONSE_PARSE_FACTOR)
                        .and_then(|wire| wire.checked_add(u64::from(nodes)
                            .checked_mul(MIN_RESPONSE_NODE_PEAK_BYTES)?))
                        .expect("existing response charge");
                    assert!(peak > 0 && peak <= charge);
                    assert_eq!(errors.len(), 1);
                    assert_eq!(errors[0].code, 14);
                    assert_eq!(errors[0].kind, BidderErrorKind::ResponseParse);
                })
                .join()
                .expect("isolated primitive allocation control");
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn proprietary_streaming_allocation_covers_known_bid_extensions() {
        let mut payload = json!({"fake_leaf":[true, false, null]});
        for _ in 0_usize..8 {
            payload = json!({"fake_branch":payload});
        }
        let valid = json!({
            "origbidcpm":1.25_f64, "origbidcur":"EUR",
            "prebid":{
                "type":"banner", "bidid":"fake_bidid", "dealpriority":7_i64,
                "dealtiersatisfied":true, "targetbiddercode":"fake_alias",
                "targeting":(0_usize..8).map(|index|
                    (format!("fake_key_{index}"), Value::String("x".repeat(64))))
                    .collect::<serde_json::Map<String, Value>>(),
                "meta":payload.clone(), "fake_prebid":payload.clone()
            },
            "fake_extension":payload
        });
        let extensions = [
            (valid.clone(), true),
            (json!(null), true),
            (json!([]), false),
            (json!({"origbidcpm":"fake_invalid"}), false),
            (json!({"origbidcur":{}}), false),
            (json!({"prebid":{"type":{}}}), false),
            (json!({"prebid":{"targeting":{"fake_key":7_i32}}}), false),
            (json!({"prebid":{"dealpriority":false}}), false),
            (json!({"prebid":{"dealtiersatisfied":[]}}), false),
        ];
        for (extension, accepted) in extensions {
            for count in [1_usize, 32, 128] {
                let candidate_extension = extension.clone();
                let sibling_extension = valid.clone();
                spawn(move || {
                    let candidate_bids: Vec<_> = (0..count)
                        .map(|index| {
                            let mut bid = good_bid(&format!("fake_bid_{index}"));
                            bid["ext"] = if index.is_multiple_of(2) {
                                candidate_extension.clone()
                            } else {
                                sibling_extension.clone()
                            };
                            bid
                        })
                        .collect();
                    let document = json!({"id":"r", "seatbid":[{"bid":candidate_bids}],
                        "ext":{"fledge_auction_configs":{"fake_imp":{"fake_keep":true}}}});
                    let decoded = measured_source(&document, accepted);
                    let expected_ids: Vec<_> = (0..count)
                        .filter(|index| accepted || !index.is_multiple_of(2))
                        .map(|index| format!("fake_bid_{index}"))
                        .collect();
                    let bids = &decoded.response.seatbid[0].bid;
                    assert_eq!(
                        bids.iter().map(|bid| &bid.id).collect::<Vec<_>>(),
                        expected_ids.iter().collect::<Vec<_>>()
                    );
                    let refused = count
                        .checked_sub(expected_ids.len())
                        .expect("failure count");
                    assert_eq!(decoded.failures, vec![CandidateFailure::Syntax; refused]);
                    assert_eq!(decoded.response.ext.as_ref(), Some(&document["ext"]));
                })
                .join()
                .expect("isolated known-extension allocation control");
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn closed_policies_and_materialized_failures_fit_existing_response_charge() {
        use super::{CandidatePolicy, decode_source_with_policy};
        use auctioneer_runtime::budget::{MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR};

        for policy in [
            CandidatePolicy::OptionalExtension,
            CandidatePolicy::RequiredExtensionNullDefault,
        ] {
            for count in [1_usize, 4, 5, 32, 33, 256, 257, 1024, 1025] {
                for all_invalid in [false, true] {
                    spawn(move || {
                        let candidates: Vec<_> = (0..count).map(|index| {
                            let mut bid = good_bid(&format!("fake_bid_{index}"));
                            bid["ext"] = if all_invalid || index.is_multiple_of(2) {
                                json!({"prebid":{"targeting":{"fake_key":false}}})
                            } else {
                                Value::Null
                            };
                            bid
                        }).collect();
                        let body = encode(&json!({"id":"r", "seatbid":[{"bid":candidates}]}));
                        let nodes = count_json_nodes(&body, 1 << 16).expect("shipped node cap");
                        assert!(body.len() <= 256 * 1024, "shipped byte cap");
                        let charge = u64::try_from(body.len()).expect("wire bytes")
                            .checked_mul(MIN_RESPONSE_PARSE_FACTOR)
                            .and_then(|bytes| bytes.checked_add(u64::from(nodes)
                                .checked_mul(MIN_RESPONSE_NODE_PEAK_BYTES)?))
                            .expect("unchanged installed response charge");
                        let baseline = alloc_track::reset_peak();
                        let (response, errors) = decode_source_with_policy(&body, nodes, policy, |_bid| true)
                            .expect("admitted source").into_parts();
                        let peak = alloc_track::peak_since(baseline);
                        let margin = peak.checked_mul(6).and_then(|bytes| bytes.checked_add(4))
                            .and_then(|bytes| bytes.checked_div(5)).expect("twenty percent margin");
                        assert!(peak > 0 && margin <= charge,
                            "policy={policy:?} count={count} all_invalid={all_invalid} peak={peak} charge={charge}");
                        let accepted = if all_invalid { 0 } else { count.checked_div(2).expect("nonzero divisor") };
                        assert_eq!(response.seatbid[0].bid.len(), accepted);
                        assert_eq!(errors.len(), count.checked_sub(accepted).expect("refusals"));
                        assert!(errors.iter().all(|error| error.code == 14
                            && error.kind == BidderErrorKind::ResponseParse
                            && error.message == "invalid bidder response"));
                        for bid in &response.seatbid[0].bid {
                            assert_eq!(bid.ext.is_some(), matches!(policy, CandidatePolicy::RequiredExtensionNullDefault));
                        }
                    }).join().expect("isolated closed-policy allocation control");
                }
            }
        }
    }

    #[test]
    fn structural_candidate_refusals_cannot_be_downgraded_to_extension_errors() {
        use super::{CandidatePolicy, Session};
        use auctioneer_runtime::bounded_json::SharedDecodeBudget;

        // Extension comes last so node exhaustion happens while reading that
        // value, after all required standard fields were successfully visited.
        let raw: &RawValue = serde_json::from_slice(
            br#"{"id":"fake_bad","impid":"fake_imp","price":1,"ext":false}"#,
        )
        .expect("raw fixture span");
        for (nodes, bytes) in [(8, raw.get().len()), (256, 0)] {
            let mut session = Session {
                typed: SharedDecodeBudget::new(nodes, bytes),
                policy: CandidatePolicy::OptionalExtension,
                prefilter: |_bid| true,
                source_failure: None,
                failures: Vec::new(),
            };
            let result: Result<_, serde_json::Error> = session.candidate(raw);
            result.expect_err("structural exhaustion cannot become a local extension refusal");
            assert!(session.failures.is_empty());
            assert_eq!(
                session
                    .source_failure
                    .expect("parser-owned structural failure")
                    .diagnostic()
                    .code,
                4
            );
        }
    }

    struct CandidateKeys<'a> {
        id: &'a str,
        extension_keys: &'a [&'a str],
    }

    impl Serialize for CandidateKeys<'_> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let fields = self
                .extension_keys
                .len()
                .checked_add(3)
                .expect("fixture field count");
            let mut map = serializer.serialize_map(Some(fields))?;
            map.serialize_entry("id", self.id)?;
            map.serialize_entry("impid", "fake_imp")?;
            map.serialize_entry("price", &1_i32)?;
            for key in self.extension_keys {
                map.serialize_entry(key, &Value::Null)?;
            }
            map.end()
        }
    }

    #[test]
    fn proprietary_streaming_candidate_extension_keys_refuse_ambiguity_locally() {
        #[derive(Serialize)]
        struct Seat<'a> {
            bid: [CandidateKeys<'a>; 3],
        }
        #[derive(Serialize)]
        struct Root<'a> {
            id: &'a str,
            seatbid: [Seat<'a>; 1],
        }

        for keys in [
            ["ext", "EXT"],
            ["EXT", "ext"],
            ["ext", "ext"],
            ["EXT", "EXT"],
            ["eXt", "ExT"],
        ] {
            let root = Root {
                id: "r",
                seatbid: [Seat {
                    bid: [
                        CandidateKeys {
                            id: "fake_first",
                            extension_keys: &[],
                        },
                        CandidateKeys {
                            id: "fake_ambiguous",
                            extension_keys: &keys,
                        },
                        CandidateKeys {
                            id: "fake_last",
                            extension_keys: &[],
                        },
                    ],
                }],
            };
            let body =
                serde_json::to_vec(&root).expect("duplicate candidate keys remain on the wire");
            let decoded =
                decode_source(&body, 256).expect("candidate ambiguity does not poison the source");
            assert_eq!(decoded.response.seatbid.len(), 1);
            let bids = &decoded.response.seatbid[0].bid;
            assert_eq!(
                bids.iter().map(|bid| bid.id.as_str()).collect::<Vec<_>>(),
                ["fake_first", "fake_last"]
            );
            assert_eq!(decoded.failures, [CandidateFailure::Syntax]);
            let error = decoded.failures[0].diagnostic();
            assert_eq!(error.code, 14);
            assert_eq!(error.kind, BidderErrorKind::ResponseParse);
            assert_eq!(error.message, "invalid bidder response");
            assert!(decoded.response.ext.is_none());
        }
    }

    #[test]
    fn proprietary_streaming_candidate_single_extension_spelling_is_unchanged() {
        for key in ["ext", "EXT", "eXt"] {
            for extension in [
                json!(null),
                json!({"origbidcpm":1.25_f64}),
                json!({"origbidcpm":"fake_invalid"}),
            ] {
                let mut fixture_bid = good_bid("fake_bid");
                fixture_bid
                    .as_object_mut()
                    .expect("candidate object")
                    .insert(key.into(), extension.clone());
                let decoded = decode_source(
                    &encode(&json!({"id":"r", "seatbid":[{"bid":[fixture_bid]}]})),
                    256,
                )
                .expect("single spelling source");
                assert_eq!(decoded.response.seatbid.len(), 1);
                let bids = &decoded.response.seatbid[0].bid;
                if key == "ext" && extension["origbidcpm"].is_string() {
                    assert!(bids.is_empty());
                    assert_eq!(decoded.failures, [CandidateFailure::Syntax]);
                } else {
                    assert!(decoded.failures.is_empty());
                    assert_eq!(bids.len(), 1);
                    let bid = &bids[0];
                    assert_eq!(bid.id, "fake_bid");
                    if key == "ext" {
                        assert!(!bid.other.contains_key(key));
                        assert_eq!(bid.ext.is_none(), extension.is_null());
                    } else {
                        assert!(bid.ext.is_none());
                        assert_eq!(bid.other.get(key), Some(&extension));
                    }
                }
            }
        }
    }

    #[test]
    fn proprietary_streaming_keeps_typed_failures_siblings_and_root_configs() {
        let mut bad = good_bid("fake_bad");
        bad["ext"] = json!({"prebid":{"targeting":{"fake_key":7_i32}}});
        let document = json!({
            "id":"fake_response", "cur":"EUR", "nbr":0_i32,
            "fake_root":{"keep":true},
            "seatbid":[
                {"seat":"fake_first", "group":1_i32, "fake_seat":[true],
                 "bid":[good_bid("fake_good_first"), bad]},
                {"seat":"fake_refused", "group":1_i32, "fake_seat":{"fake_keep":true},
                 "bid":[invalid_extension_bid("fake_bad_first"), invalid_extension_bid("fake_bad_last")]},
                {"seat":"fake_last", "bid":[good_bid("fake_good_last")]}
            ],
            "ext":{"fledge_auction_configs":{"fake_imp":{"keep":true}}}
        });
        let decoded =
            decode_source(&encode(&document), 256).expect("candidate-local typed failure");
        assert_eq!(decoded.failures, [CandidateFailure::Syntax; 3]);
        let response = decoded.response;
        assert_eq!(response.id, "fake_response");
        assert_eq!(response.cur, "EUR");
        assert_eq!(response.nbr, Some(0_i32));
        assert_eq!(
            response.other.get("fake_root"),
            Some(&document["fake_root"])
        );
        assert_eq!(response.ext, Some(document["ext"].clone()));
        assert_eq!(response.seatbid.len(), 3);
        let mut seats = response.seatbid.into_iter();
        let first = seats.next().expect("first seat");
        assert_eq!(first.seat, "fake_first");
        assert_eq!(first.group, 1_i32);
        assert_eq!(first.other.get("fake_seat"), Some(&json!([true])));
        assert_eq!(first.bid.len(), 1);
        assert_eq!(first.bid[0].id, "fake_good_first");
        let refused = seats.next().expect("all-refused seat remains present");
        assert_eq!(refused.seat, "fake_refused");
        assert_eq!(refused.group, 1_i32);
        assert_eq!(
            refused.other.get("fake_seat"),
            Some(&json!({"fake_keep":true}))
        );
        assert!(refused.bid.is_empty());
        assert_eq!(refused.bid.capacity(), 0);
        let last = seats.next().expect("last seat");
        assert_eq!(last.seat, "fake_last");
        assert_eq!(last.group, 0_i32);
        assert!(last.other.is_empty());
        assert_eq!(last.bid.len(), 1);
        assert_eq!(last.bid[0].id, "fake_good_last");
        assert!(seats.next().is_none());
    }

    #[test]
    fn proprietary_streaming_spends_one_shared_candidate_ledger() {
        let first = good_bid("fake_first");
        let last = good_bid("fake_last");
        let invalid = invalid_extension_bid("fake_bad");
        let body = encode(&json!({"id":"fake_response", "seatbid":[
            {"bid":[first.clone(), invalid.clone()]}, {"bid":[last.clone()]}
        ]}));
        let nodes = count_json_nodes(&body, 256).expect("fixture source count");
        let decoded = decode_source(&body, nodes).expect("shared candidate ledger");
        assert_eq!(decoded.source_nodes, nodes);
        // Each narrow Bid is seven nodes. The failed extension candidate now
        // spends all nine nodes so its later standard fields cannot be hidden.
        // Completing that disjoint span neither renews nor refunds the ledger.
        assert_eq!(
            decoded.remaining_nodes,
            nodes.checked_sub(23).expect("typed visits")
        );
        let visited = encode(&first)
            .len()
            .checked_add(encode(&invalid).len())
            .and_then(|bytes| bytes.checked_add(encode(&last).len()))
            .expect("candidate bytes");
        assert_eq!(
            decoded.remaining_bytes,
            body.len().checked_sub(visited).expect("remaining bytes")
        );
        assert_eq!(decoded.response.seatbid.len(), 2);
        assert_eq!(decoded.response.seatbid[0].bid.len(), 1);
        assert_eq!(decoded.response.seatbid[1].bid.len(), 1);
        assert_eq!(decoded.response.seatbid[0].bid[0].id, "fake_first");
        assert_eq!(decoded.response.seatbid[1].bid[0].id, "fake_last");
        assert_eq!(decoded.failures, [CandidateFailure::Syntax]);
    }

    #[test]
    fn proprietary_streaming_rolls_back_malformed_and_trailing_source() {
        let body = encode(&json!({"id":"fake_response", "seatbid":[
            {"bid":[good_bid("fake_good"), invalid_extension_bid("fake_bad")]}
        ]}));
        decode_source(&body, 256).expect("valid source before rollback mutations");
        let mut malformed = body.clone();
        assert_eq!(malformed.pop(), Some(b'}'));
        assert_source_failure(&malformed, 256, 14);
        let mut trailing = body;
        trailing.extend_from_slice(b" trailing");
        assert_source_failure(&trailing, 256, 14);
    }

    #[test]
    fn proprietary_streaming_retains_required_fields_and_typed_duplicate_refusals() {
        struct DuplicateRoot;
        impl Serialize for DuplicateRoot {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut root = serializer.serialize_map(Some(2))?;
                root.serialize_entry("id", "fake_first")?;
                root.serialize_entry("id", "fake_duplicate")?;
                root.end()
            }
        }

        struct DuplicateSeat;
        impl Serialize for DuplicateSeat {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut seat = serializer.serialize_map(Some(2))?;
                let empty: &[Value] = &[];
                seat.serialize_entry("bid", empty)?;
                seat.serialize_entry("bid", empty)?;
                seat.end()
            }
        }

        #[derive(Serialize)]
        struct SeatRoot<'a> {
            id: &'a str,
            seatbid: [DuplicateSeat; 1],
        }

        let valid = encode(&json!({"id":"fake_response", "seatbid":[{"bid":[]}]}));
        decode_source(&valid, 256).expect("required source fields");
        for invalid in [
            json!({"seatbid":[]}),
            json!({"id":"fake_response", "seatbid":[{}]}),
            json!({"id":"fake_response", "cur":null}),
            json!({"id":"fake_response", "seatbid":[{"bid":[], "group":"fake_invalid"}]}),
        ] {
            assert_source_failure(&encode(&invalid), 256, 14);
        }
        let root = serde_json::to_vec(&DuplicateRoot).expect("duplicate recognized root key");
        assert_source_failure(&root, 256, 14);
        let duplicate = serde_json::to_vec(&SeatRoot {
            id: "fake_response",
            seatbid: [DuplicateSeat],
        })
        .expect("duplicate recognized seat key");
        assert_source_failure(&duplicate, 256, 14);
    }

    #[test]
    fn proprietary_streaming_uses_source_nodes_and_depth_before_candidate_allocation() {
        let body =
            encode(&json!({"id":"fake_response", "seatbid":[{"bid":[good_bid("fake_good")]}]}));
        let nodes = count_json_nodes(&body, 256).expect("fixture source count");
        decode_source(&body, nodes).expect("inclusive source node budget");
        assert_source_failure(&body, nodes.checked_sub(1).expect("one fewer node"), 4);
        for depth in [63_usize, 64, 65] {
            let mut value = json!(null);
            // The response root itself is the first container.
            for _ in 1..depth {
                value = json!([value]);
            }
            let nested = encode(&json!({"id":"fake_response", "fake_unknown":value}));
            if depth <= 64 {
                decode_source(&nested, 256).expect("inclusive fixed source depth");
            } else {
                assert_source_failure(&nested, 256, 4);
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn proprietary_streaming_retained_outputs_fit_existing_response_charge() {
        use auctioneer_runtime::budget::{MIN_RESPONSE_NODE_PEAK_BYTES, MIN_RESPONSE_PARSE_FACTOR};

        for count in [1_usize, 32, 256] {
            for depth in [0_usize, 8, 48] {
                for mixed in [false, true] {
                    spawn(move || {
                    let mut payload = json!({"fake_leaf":true});
                    for _ in 0..depth {
                        payload = json!({"fake_branch":payload});
                    }
                    let bids: Vec<_> = (0..count).map(|index| {
                        let mut bid = good_bid(&format!("fake_bid_{index}"));
                        bid["fake_payload"] = payload.clone();
                        if mixed && index.is_multiple_of(2) {
                            // Typed extension failure after retaining nested
                            // metadata exercises partially allocated targets.
                            bid["ext"] = json!({"prebid":{"meta":payload.clone(),
                                "targeting":{"fake_invalid":7_i32}}});
                        }
                        bid
                    }).collect();
                    let body = encode(&json!({"id":"fake_response", "seatbid":[{"bid":bids}],
                        "ext":{"fledge_auction_configs":{"fake_imp":{"fake_keep":true}}}}));
                    let nodes = count_json_nodes(&body, 1 << 16).expect("funded source shape");
                    let wire = u64::try_from(body.len()).expect("fixture decoded bytes");
                    let charge = wire.checked_mul(MIN_RESPONSE_PARSE_FACTOR)
                        .and_then(|bytes| {
                            let node_bytes = u64::from(nodes).checked_mul(MIN_RESPONSE_NODE_PEAK_BYTES)?;
                            bytes.checked_add(node_bytes)
                        })
                        .expect("existing installed response charge");
                    let standard_baseline = alloc_track::reset_peak();
                    let standard = from_slice_within::<BidResponse>(&body, nodes);
                    let standard_peak = alloc_track::peak_since(standard_baseline);
                    assert!(standard_peak > 0);
                    assert!(standard_peak <= charge, "standard output exceeds installed charge");
                    assert_eq!(standard.is_err(), mixed);
                    drop(standard);
                    let source_baseline = alloc_track::reset_peak();
                    let decoded = decode_source(&body, nodes).expect("source plus aggregate typed output");
                    let peak = alloc_track::peak_since(source_baseline);
                    assert!(peak > 0, "allocation harness must observe retained output");
                    let margin = peak.checked_mul(6).and_then(|bytes| bytes.checked_add(4))
                        .and_then(|bytes| bytes.checked_div(5)).expect("twenty percent margin");
                    assert!(margin <= charge,
                        "count={count} depth={depth} mixed={mixed} source/typed peak={peak} margin={margin} charge={charge}");
                    let expected_ids: Vec<_> = (0..count)
                        .filter(|index| !mixed || !index.is_multiple_of(2))
                        .map(|index| format!("fake_bid_{index}"))
                        .collect();
                    assert_eq!(decoded.response.seatbid[0].bid.iter()
                        .map(|bid| &bid.id).collect::<Vec<_>>(),
                        expected_ids.iter().collect::<Vec<_>>());
                    let refused = count.checked_sub(expected_ids.len()).expect("failure count");
                    assert_eq!(decoded.failures, vec![CandidateFailure::Syntax; refused]);
                    assert_eq!(decoded.response.ext,
                        Some(json!({"fledge_auction_configs":{"fake_imp":{"fake_keep":true}}})));
                }).join().expect("isolated allocation control");
                }
            }
        }
    }
}
