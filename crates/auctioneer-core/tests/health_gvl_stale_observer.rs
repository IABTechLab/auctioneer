//! The `/_auctioneer/health/gvl` stale-GCL counter, fed from a real
//! auction.
//!
//! Sole test in its own binary on purpose, for the same reason as its
//! `health_gvl_observer` sibling: both counters are process singletons,
//! so a second test driving a GDPR auction in this binary would make
//! the before/after deltas meaningless.
//!
//! The distinction this file pins is that warning 10019 reaches the
//! counter through the SAME path a request takes — the resolver emits
//! it, the handler serialises it into `ext.warnings`, and the auction
//! handler's response scanner records it. A test that incremented the
//! singleton directly would pass with that scanner arm deleted.

mod support;

#[cfg(test)]
mod config_funding_support;
use config_funding_support as config_funding;

#[cfg(test)]
use auctioneer_bidder::test_support::serving_catalog;

#[cfg(test)]
mod fanout_limits;

#[cfg(test)]
mod tests {
    use crate::config_funding;
    use crate::serving_catalog;
    use crate::support::ctx_with_config;
    use async_trait::async_trait;
    use auctioneer_core::handlers::health_gvl::{counter, gcl_stale_counter};
    use auctioneer_core::handlers::{TestFanoutHandle, auction};
    use auctioneer_core::host_primitives::{TestClockHandle, TestRngHandle};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutError,
        FanoutInternalError, TimeoutCause,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use auctioneer_runtime::{Clock, FakeClock, Rng, SeededRng};
    use edgezero_core::body::Body;
    use edgezero_core::http::StatusCode;
    use futures::executor::block_on;
    use http::{Method, Request, Uri};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;
    use time::OffsetDateTime;

    /// GDPR on, TCF2 on, and purpose 2 explicitly NOT enforced. The last
    /// part is what makes the privacy effect observable on a real
    /// outbound call: under the unusable-consent default row
    /// `AllowBidRequest = !p2_enforced`, so the bidder survives fan-out
    /// while its identifiers are still stripped.
    const CONFIG: &str = "[gdpr]\n\
                          enabled = true\n\n\
                          [gdpr.tcf2]\n\
                          enabled = true\n\n\
                          [gdpr.tcf2.purposes.2]\n\
                          enforce_purpose = \"disabled\"\n";

    /// A `(spec 3, list)` pair the baked archive does not carry. The
    /// staleness check fires BEFORE vendor-list selection, so this
    /// fixture never reaches GVL lookup and the specific version is
    /// immaterial — it is held absent so that a regression which let
    /// the parse run past the stale gate would surface as 10018 here
    /// rather than as silence.
    const ABSENT_VENDOR_LIST_VERSION: u16 = 999;
    /// Policy 4 maps to GVL specification version 3.
    const TCF_POLICY_VERSION: u8 = 4;
    /// Google LLC — present in the baked Global CMP List with no
    /// deletion date, so CMP authentication succeeds and the resolver
    /// reaches GVL selection.
    const CMP_ID: u16 = 300;
    /// Deciseconds since the epoch for 2024-02-01T00:00:00Z: after the
    /// policy-v3 cutoff and day-truncated, which the decoder requires.
    const CREATED_DECISECONDS: u64 = 17_067_456_000;

    const WARNING_GVL_VERSION_UNAVAILABLE: u64 = 10018;
    const WARNING_GCL_STALE: u64 = 10019;
    const WIDE_CAP: usize = 1 << 20;

    // ---------------------------------------------------------------
    // A minimal TCF v2 encoder — only the fields this fixture pins.
    // ---------------------------------------------------------------

    const B64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    /// MSB-first bit accumulator; the final byte is zero-padded.
    #[derive(Default)]
    struct Bits {
        bytes: Vec<u8>,
        len: usize,
    }

    impl Bits {
        fn push_bit(&mut self, bit: bool) {
            let index = self.len.wrapping_div(8);
            if index == self.bytes.len() {
                self.bytes.push(0);
            }
            if bit {
                let offset = u32::try_from(self.len.wrapping_rem(8)).unwrap_or(0);
                if let Some(byte) = self.bytes.get_mut(index) {
                    *byte |= 0x80_u8.wrapping_shr(offset);
                }
            }
            self.len = self.len.wrapping_add(1);
        }

        fn push(&mut self, value: u64, width: u32) {
            for shift in (0..width).rev() {
                self.push_bit(value.wrapping_shr(shift) & 1 == 1);
            }
        }

        fn into_base64url(self) -> String {
            let mut out = String::new();
            // `as_chunks` hands back fixed-size arrays, so the three
            // bytes of a full group are reachable by pattern rather
            // than by fallible indexing that could never fail.
            let (chunks, rest) = self.bytes.as_chunks::<3>();
            for &[b0, b1, b2] in chunks {
                push_six(&mut out, b0.wrapping_shr(2));
                push_six(&mut out, b0.wrapping_shl(4) & 0x30 | b1.wrapping_shr(4));
                push_six(&mut out, b1.wrapping_shl(2) & 0x3C | b2.wrapping_shr(6));
                push_six(&mut out, b2 & 0x3F);
            }
            if let Some(&b0) = rest.first() {
                push_six(&mut out, b0.wrapping_shr(2));
                match rest.get(1) {
                    Some(&b1) => {
                        push_six(&mut out, b0.wrapping_shl(4) & 0x30 | b1.wrapping_shr(4));
                        push_six(&mut out, b1.wrapping_shl(2) & 0x3C);
                    }
                    None => push_six(&mut out, b0.wrapping_shl(4) & 0x30),
                }
            }
            out
        }
    }

    fn push_six(out: &mut String, six_bits: u8) {
        let code = B64URL
            .get(usize::from(six_bits & 0x3F))
            .copied()
            .unwrap_or(b'A');
        out.push(char::from(code));
    }

    /// 6-bit alphabet code for an ASCII uppercase letter (`b'A'` → 0).
    fn letter(character: u8) -> u64 {
        u64::from(character.wrapping_sub(b'A'))
    }

    /// A structurally valid TC string naming a vendor-list revision the
    /// baked archive does not carry. Purpose 1 consent only; no vendors,
    /// no publisher restrictions.
    fn consent_naming_an_absent_gvl() -> String {
        let mut core = Bits::default();
        core.push(2, 6); // Version
        core.push(CREATED_DECISECONDS, 36); // Created
        core.push(CREATED_DECISECONDS, 36); // LastUpdated (must equal Created)
        core.push(u64::from(CMP_ID), 12);
        core.push(2, 12); // CmpVersion
        core.push(1, 6); // ConsentScreen
        core.push(letter(b'E'), 6); // ConsentLanguage
        core.push(letter(b'N'), 6);
        core.push(u64::from(ABSENT_VENDOR_LIST_VERSION), 12);
        core.push(u64::from(TCF_POLICY_VERSION), 6);
        core.push_bit(true); // IsServiceSpecific
        core.push_bit(false); // UseNonStandardStacks
        core.push(0, 12); // SpecialFeatureOptIns 1..=12
        for id in 1_u8..=24 {
            core.push_bit(id == 1); // PurposesConsent — purpose 1 only
        }
        core.push(0, 24); // PurposesLITransparency
        core.push_bit(false); // PurposeOneTreatment
        core.push(letter(b'D'), 6); // PublisherCC
        core.push(letter(b'E'), 6);
        core.push(0, 16); // vendor consent MaxVendorId
        core.push_bit(false); //   bitfield encoding, zero entries
        core.push(0, 16); // vendor LI MaxVendorId
        core.push_bit(false);
        core.push(0, 12); // NumPubRestrictions

        // A core-only string is structurally invalid, so append the
        // canonical minimal Disclosed Vendors segment.
        let mut disclosed = Bits::default();
        disclosed.push(1, 3); // SegmentType = DisclosedVendors
        disclosed.push(1, 16); // MaxVendorId
        disclosed.push_bit(false); // bitfield encoding
        disclosed.push_bit(true); // vendor 1 disclosed

        format!(
            "{core}.{disclosed}",
            core = core.into_base64url(),
            disclosed = disclosed.into_base64url(),
        )
    }

    // ---------------------------------------------------------------
    // Fixture request + recording fanout
    // ---------------------------------------------------------------

    /// `regs.gdpr = 1` puts GDPR in scope through the WIRE signal: with
    /// an ambiguous wire the baked `default_value = "0"` geo fallback
    /// resolves to "not in scope" and enforcement never activates.
    /// `user.id` and `device.ifa` are the concrete identifiers whose
    /// removal is the observation point.
    fn fixture_body(consent: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "id": "auc-gvl",
            "tmax": 1000,
            "cur": ["USD"],
            "regs": { "gdpr": 1 },
            "site": {
                "publisher": { "id": "acct-gvl" },
                "page": "https://pub.example.test/article",
            },
            "user": { "id": "user-123", "consent": consent },
            "device": { "ifa": "ifa-abc-123" },
            "imp": [{
                "id": "imp-1",
                "banner": { "w": 300, "h": 250 },
                "ext": { "bidder": { "fake_bb": {} } },
            }],
        }))
        .expect("fixture body serialises")
    }

    /// Captures each outbound body so the scrub is directly observable,
    /// then reports a benign per-bidder timeout so the auction completes
    /// without a bid.
    #[derive(Default)]
    struct RecordingFanout {
        bodies: Mutex<Vec<Vec<u8>>>,
    }

    #[async_trait(?Send)]
    impl BidderFanout for RecordingFanout {
        async fn send_all(
            &self,
            calls: Vec<BidderCall>,
            _deadline: AuctionDeadline,
        ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
            let mut recorded = self.bodies.lock().unwrap_or_else(PoisonError::into_inner);
            for call in &calls {
                recorded.push(call.request.body().to_vec());
            }
            Ok(calls
                .iter()
                .map(|_| BidderCallResult {
                    result: Err(FanoutError::Timeout(TimeoutCause::Bidder)),
                    elapsed: Duration::ZERO,
                })
                .collect())
        }
    }

    #[test]
    fn auction_past_the_gcl_expiry_increments_the_stale_counter() {
        let config = format!(
            "[aliases]\nfake_bb={:?}\n{CONFIG}",
            serving_catalog::passthrough_info().name,
        );
        config_funding::install();
        // PAST the baked Global CMP List's own RFC 9111 lifetime.
        // Derived from the baked expiry rather than a hardcoded date so
        // the fixture stays correct across every archive refresh.
        let expiry =
            OffsetDateTime::from_unix_timestamp(auctioneer_config::gcl_expires_at_unix_secs())
                .expect("the baked GCL expiry is a representable instant");
        let now = expiry.saturating_add(time::Duration::hours(1));
        let clock = Arc::new(FakeClock::new(now));
        let handler_clock: Arc<dyn Clock + Send + Sync> = Arc::<FakeClock>::clone(&clock);
        let rng: Arc<dyn Rng + Send + Sync> = Arc::new(SeededRng::new(42));
        let fanout = Arc::new(RecordingFanout::default());
        let handler_fanout: Arc<dyn BidderFanout + Send + Sync> =
            Arc::<RecordingFanout>::clone(&fanout);

        let stale_before = gcl_stale_counter().count_last_hour(now);
        let unavailable_before = counter().count_last_hour(now);

        let mut request = Request::builder()
            .method(Method::POST)
            .uri(Uri::from_static("/openrtb2/auction"))
            .header("content-type", "application/json")
            .body(Body::from_bytes(fixture_body(
                &consent_naming_an_absent_gvl(),
            )))
            .expect("request builds");
        request
            .extensions_mut()
            .insert(TestFanoutHandle(handler_fanout));
        request
            .extensions_mut()
            .insert(TestClockHandle(handler_clock));
        request.extensions_mut().insert(TestRngHandle(rng));

        let ctx = ctx_with_config(&config, request);
        let response: BidResponse = block_on(async {
            let response = auction(ctx).await.expect("the handler answers");
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "an enforced auction with a stale GCL is still a 200",
            );
            let (_parts, body) = response.into_parts();
            let bytes = body
                .into_bytes_bounded(WIDE_CAP)
                .await
                .expect("response body reads");
            serde_json::from_slice(&bytes).expect("the response deserializes as a BidResponse")
        });

        // 1. The bidder SURVIVED fan-out with its identifiers stripped:
        //    a stale GCL makes consent unusable, and purpose 2 is not
        //    enforced, so `AllowBidRequest` holds while `PassID` does
        //    not. This is what proves the auction really ran the
        //    enforcement path rather than short-circuiting.
        let recorded = fanout
            .bodies
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        assert_eq!(
            recorded.len(),
            1,
            "exactly one outbound call for `fake_bb` is expected",
        );
        let outbound: BidRequest =
            serde_json::from_slice(recorded.first().expect("one recorded body"))
                .expect("the selected implementation forwards OpenRTB verbatim");
        assert_eq!(
            outbound.user.as_ref().and_then(|u| u.id.clone()),
            None,
            "`user.id` must be scrubbed under PassID=false",
        );

        // 2. The warning reached the wire.
        let ext = response.ext.clone().unwrap_or(serde_json::Value::Null);
        let codes: Vec<u64> = ext
            .pointer("/warnings/prebid")
            .and_then(serde_json::Value::as_array)
            .map(|rows| {
                rows.iter()
                    .filter_map(|row| row.get("code").and_then(serde_json::Value::as_u64))
                    .collect()
            })
            .unwrap_or_default();
        assert!(
            codes.contains(&WARNING_GCL_STALE),
            "the response must carry the 10019 warning; ext {ext}",
        );

        // 3. The auction handler's response scanner — not the test —
        //    moved it into the counter. Incrementing the singleton
        //    directly would satisfy the assertion below while leaving
        //    the scanner's 10019 arm dead, so the wire assertion above
        //    and this delta are load-bearing together.
        assert_eq!(
            gcl_stale_counter().count_last_hour(now),
            stale_before.wrapping_add(1),
            "the auction handler must record every 10019 into the stale-GCL counter",
        );

        // 4. The two counters stay distinct: a stale GCL fails closed
        //    before vendor-list selection, so no 10018 is produced.
        assert!(
            !codes.contains(&WARNING_GVL_VERSION_UNAVAILABLE),
            "a stale GCL short-circuits before GVL lookup; ext {ext}",
        );
        assert_eq!(
            counter().count_last_hour(now),
            unavailable_before,
            "the unavailable-lookup counter must not move on a stale-GCL auction",
        );
    }
}
