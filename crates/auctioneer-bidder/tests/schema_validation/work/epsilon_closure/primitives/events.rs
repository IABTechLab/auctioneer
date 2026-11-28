//! Finite matched-callee event table, separate from the literal test oracles.
//!
//! A local source event is an entry, evaluated expression/place, local binding
//! or explicit branch block. Parentheses and direct callee path spelling add
//! no event. Intrinsic calls/indexes are single source events, not zero CPU.
//! Edges below add every ordinary callee; implicit dereferences are explicit.
//! The two branch fragments have no invented frame. All entries stay present
//! at zero invocations so zero workload cannot discharge frame/byte duties.

use super::{
    Calls, CapturedData, HelperKey, Invariant, Lemma, LocalContract, Premises, Refusal, SourceUnit,
    SymbolicWrites, WorkLedger,
};
use std::collections::{BTreeMap, BTreeSet};

// Borrowed hypotheses shared by the original DATA contract and window join.
// This view carries no compiler/source identity or actual caller authority.
#[derive(Clone, Copy)]
pub(super) struct HelperFacts<'a> {
    pub owner: &'a str,
    pub epoch: u64,
    pub helpers: &'a [super::HelperData],
}

impl<'a> From<&'a CapturedData> for HelperFacts<'a> {
    fn from(data: &'a CapturedData) -> Self {
        Self {
            owner: &data.owner,
            epoch: data.epoch,
            helpers: &data.helpers,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Site {
    State,
    States,
    Epsilon,
    IdSliceIndex,
    ArcDeref,
    ArcInner,
    NonNullRef,
    ArcPointer,
    CastConst,
    VecDeref,
    VecDerefMut,
    VecSlice,
    VecMutSlice,
    VecPointer,
    VecMutPointer,
    RawPointer,
    InnerPointer,
    InnerNonNull,
    UniqueCast,
    UniqueNonNull,
    NonNullCast,
    BytePointer,
    ElementPointer,
    SliceIndex,
    SliceIndexMut,
    UsizeIndex,
    UsizeIndexMut,
    IdUsize,
    SmallUsize,
    Insert,
    InsertSuccess,
    Contains,
    ContainsRhs,
    Len,
    Capacity,
    IdNew,
    SmallNew,
    IdEq,
    SmallEq,
    IdVecIndex,
    IdVecIndexMut,
    VecIndex,
    VecIndexMut,
    VecLen,
}

struct Row {
    frame: Option<HelperKey>,
    local: [u64; 3],
    callees: &'static [Site],
}

fn key(
    unit: SourceUnit,
    owner: &'static str,
    method: &'static str,
    receiver: &'static str,
) -> HelperKey {
    HelperKey {
        unit,
        owner,
        method,
        receiver,
    }
}

fn shared_frame(site: Site, lemma: Lemma) -> Option<HelperKey> {
    use Site::*;
    use SourceUnit::{Ids, NonNull, RawVec, SliceIndex as SliceUnit, Unique, Vec as VecUnit};
    let nfa = lemma == Lemma::NfaStateAccess;
    let read_vec = if nfa { "&Vec<State>" } else { "&Vec<StateID>" };
    let read_slice = if nfa { "&[State]" } else { "&[StateID]" };
    Some(match site {
        IdUsize => key(Ids, "StateID", "as_usize", "&StateID"),
        SmallUsize => key(Ids, "SmallIndex", "as_usize", "&SmallIndex"),
        VecDeref => key(VecUnit, "Vec<T>", "deref", read_vec),
        VecSlice => key(VecUnit, "Vec<T>", "as_slice", read_vec),
        VecPointer => key(VecUnit, "Vec<T>", "as_ptr", read_vec),
        RawPointer => key(
            RawVec,
            "RawVec<T>",
            "ptr",
            if nfa {
                "&RawVec<State>"
            } else {
                "&RawVec<StateID>"
            },
        ),
        InnerPointer => key(
            RawVec,
            "RawVecInner",
            if nfa {
                "ptr::<State>"
            } else {
                "ptr::<StateID>"
            },
            "&RawVecInner",
        ),
        InnerNonNull => key(
            RawVec,
            "RawVecInner",
            if nfa {
                "non_null::<State>"
            } else {
                "non_null::<StateID>"
            },
            "&RawVecInner",
        ),
        UniqueCast => key(
            Unique,
            "Unique<T>",
            if nfa {
                "cast::<State>"
            } else {
                "cast::<StateID>"
            },
            "Unique<u8>",
        ),
        UniqueNonNull => key(
            Unique,
            "Unique<T>",
            "as_non_null_ptr",
            if nfa {
                "Unique<State>"
            } else {
                "Unique<StateID>"
            },
        ),
        NonNullCast => key(
            NonNull,
            "NonNull<T>",
            if nfa {
                "cast::<State>"
            } else {
                "cast::<StateID>"
            },
            "NonNull<u8>",
        ),
        BytePointer => key(NonNull, "NonNull<T>", "as_ptr", "NonNull<u8>"),
        ElementPointer => key(
            NonNull,
            "NonNull<T>",
            "as_ptr",
            if nfa {
                "NonNull<State>"
            } else {
                "NonNull<StateID>"
            },
        ),
        SliceIndex => key(SliceUnit, "[T]", "index<usize>", read_slice),
        UsizeIndex => key(SliceUnit, "usize", "index", read_slice),
        State | States | Epsilon | IdSliceIndex | ArcDeref | ArcInner | NonNullRef | ArcPointer
        | CastConst | VecDerefMut | VecMutSlice | VecMutPointer | SliceIndexMut | UsizeIndexMut
        | Insert | InsertSuccess | Contains | ContainsRhs | Len | Capacity | IdNew | SmallNew
        | IdEq | SmallEq | IdVecIndex | IdVecIndexMut | VecIndex | VecIndexMut | VecLen => {
            return None;
        }
    })
}

fn specific_frame(site: Site) -> Option<HelperKey> {
    use Site::*;
    use SourceUnit::{
        Arc, Ids, MutPtr, Nfa, NonNull, SliceIndex as SliceUnit, Sparse, Vec as VecUnit,
    };
    Some(match site {
        State => key(Nfa, "NFA", "state", "&NFA"),
        States => key(Nfa, "NFA", "states", "&NFA"),
        Epsilon => key(Nfa, "State", "is_epsilon", "&State"),
        IdSliceIndex => key(Ids, "[T]", "index<StateID>", "&[State]"),
        ArcDeref => key(Arc, "Arc<T>", "deref", "&Arc<nfa::Inner>"),
        ArcInner => key(Arc, "Arc<T>", "inner", "&Arc<nfa::Inner>"),
        NonNullRef => key(
            NonNull,
            "NonNull<T>",
            "as_ref",
            "&NonNull<ArcInner<nfa::Inner>>",
        ),
        ArcPointer => key(
            NonNull,
            "NonNull<T>",
            "as_ptr",
            "NonNull<ArcInner<nfa::Inner>>",
        ),
        CastConst => key(MutPtr, "*mut T", "cast_const", "*mut ArcInner<nfa::Inner>"),
        Insert => key(Sparse, "SparseSet", "insert", "&mut SparseSet"),
        Contains => key(Sparse, "SparseSet", "contains", "&SparseSet"),
        Len => key(Sparse, "SparseSet", "len", "&SparseSet"),
        Capacity => key(Sparse, "SparseSet", "capacity", "&SparseSet"),
        IdNew => key(Ids, "StateID", "new_unchecked", "usize"),
        SmallNew => key(Ids, "SmallIndex", "new_unchecked", "usize"),
        IdEq => key(Ids, "StateID", "eq", "&StateID"),
        SmallEq => key(Ids, "SmallIndex", "eq", "&SmallIndex"),
        IdVecIndex => key(Ids, "Vec<T>", "index<StateID>", "&Vec<StateID>"),
        IdVecIndexMut => key(Ids, "Vec<T>", "index_mut<StateID>", "&mut Vec<StateID>"),
        VecIndex => key(VecUnit, "Vec<T>", "index<usize>", "&Vec<StateID>"),
        VecIndexMut => key(VecUnit, "Vec<T>", "index_mut<usize>", "&mut Vec<StateID>"),
        VecLen => key(VecUnit, "Vec<T>", "len", "&Vec<StateID>"),
        VecDerefMut => key(VecUnit, "Vec<T>", "deref_mut", "&mut Vec<StateID>"),
        VecMutSlice => key(VecUnit, "Vec<T>", "as_mut_slice", "&mut Vec<StateID>"),
        VecMutPointer => key(VecUnit, "Vec<T>", "as_mut_ptr", "&mut Vec<StateID>"),
        SliceIndexMut => key(SliceUnit, "[T]", "index_mut<usize>", "&mut [StateID]"),
        UsizeIndexMut => key(SliceUnit, "usize", "index_mut", "&mut [StateID]"),
        InsertSuccess | ContainsRhs | VecDeref | VecSlice | VecPointer | RawPointer
        | InnerPointer | InnerNonNull | UniqueCast | UniqueNonNull | NonNullCast | BytePointer
        | ElementPointer | SliceIndex | UsizeIndex | IdUsize | SmallUsize => return None,
    })
}

// The numbers are local source-site counts, not transitive coefficients.
// E.g. as_slice: entry, unsafe, borrow, deref, intrinsic call, as_ptr call,
// receiver, len projection, receiver. Its ordinary pointer chain is separate.
fn shared_rows() -> Vec<(Site, [u64; 3], &'static [Site])> {
    use Site::*;
    vec![
        // entry + call/cast + field projection + receiver
        (IdUsize, [4, 0, 0], &[SmallUsize]),
        (SmallUsize, [4, 0, 0], &[]),
        // entry + method call + receiver
        (VecDeref, [3, 0, 0], &[VecSlice]),
        (VecSlice, [9, 0, 0], &[VecPointer]),
        // entry + method call + field projection + receiver
        (VecPointer, [4, 0, 0], &[RawPointer]),
        (RawPointer, [4, 0, 0], &[InnerPointer]),
        // entry + two method calls + receiver
        (InnerPointer, [4, 0, 0], &[InnerNonNull, ElementPointer]),
        // entry + two calls + field projection + receiver
        (InnerNonNull, [5, 0, 0], &[UniqueCast, UniqueNonNull]),
        // entry + struct construction + call + field + receiver + marker place
        (UniqueCast, [6, 0, 0], &[NonNullCast]),
        // entry + field projection + receiver
        (UniqueNonNull, [3, 0, 0], &[]),
        // entry + unsafe + intrinsic call + cast + method call + receiver
        (NonNullCast, [6, 0, 0], &[BytePointer]),
        // entry + unsafe + intrinsic call + receiver; concrete instances differ
        (BytePointer, [4, 0, 0], &[]),
        (ElementPointer, [4, 0, 0], &[]),
        // entry + method call + index place + receiver
        (SliceIndex, [4, 0, 0], &[UsizeIndex]),
        // entry + borrow + intrinsic index + dereference + slice + index place
        (UsizeIndex, [6, 0, 0], &[]),
    ]
}

fn rows(lemma: Lemma) -> Result<BTreeMap<Site, Row>, Refusal> {
    use Site::*;
    let mut entries = shared_rows();
    match lemma {
        Lemma::NfaStateAccess => entries.extend([
            // entry + borrow + index + states call + receiver + ID place
            (State, [6, 0, 0], &[States, IdSliceIndex][..]),
            // entry + borrow + two projections + receiver
            (States, [5, 0, 0], &[ArcDeref, VecDeref]),
            // Entry, match, deref, receiver, result, nine variant probes.
            (Epsilon, [14, 10, 9], &[]),
            // entry + borrow + index + receiver + as_usize call + index place
            (IdSliceIndex, [6, 0, 0], &[IdUsize, SliceIndex]),
            // entry + borrow + data projection + inner call + receiver
            (ArcDeref, [5, 0, 0], &[ArcInner]),
            // entry + unsafe + as_ref call + pointer projection + receiver
            (ArcInner, [5, 0, 0], &[NonNullRef]),
            // entry + unsafe + borrow + dereference + two calls + receiver
            (NonNullRef, [7, 0, 0], &[ArcPointer, CastConst]),
            (ArcPointer, [4, 0, 0], &[]),
            // entry + cast + receiver
            (CastConst, [3, 0, 0], &[]),
        ]),
        Lemma::SparseSetInsert => entries.extend(sparse_rows()),
    }
    entries
        .into_iter()
        .map(|(site, local, callees)| {
            let frame = shared_frame(site, lemma).or_else(|| specific_frame(site));
            if frame.is_none() && !matches!(site, InsertSuccess | ContainsRhs) {
                return Err(Refusal::MissingSourceProof);
            }
            Ok((
                site,
                Row {
                    frame,
                    local,
                    callees,
                },
            ))
        })
        .collect()
}

fn sparse_rows() -> Vec<(Site, [u64; 3], &'static [Site])> {
    use Site::*;
    vec![
        // Entry + if + block + contains/receiver/id + return/false. The
        // returning prefix is conservatively charged even on successful calls.
        (Insert, [8, 2, 0], &[Contains]),
        // len binding 3; builtin assert guard/predicate 6; new binding 3;
        // two indexed assignments 12; len increment 4; final true 1.
        // Valid dimensions/back-map exclude formatting/panic and its second
        // diagnostic-only capacity call, not by declaring diagnostics free.
        (
            InsertSuccess,
            [29, 1, 1],
            &[Len, Capacity, IdNew, IdVecIndexMut, IdVecIndexMut],
        ),
        // entry + let/index/field/receiver/ID + &&/</two calls/index/receiver
        (Contains, [12, 1, 1], &[IdVecIndex, IdUsize, Len]),
        // equality expression + index + dense field + receiver + index + ID
        (ContainsRhs, [6, 0, 0], &[IdVecIndex, IdEq]),
        (Len, [3, 0, 0], &[]),
        (Capacity, [4, 0, 0], &[VecLen]),
        // entry + let + len field/receiver + unsafe + intrinsic assume call
        // + comparison + len/MAX_SLICE_LEN places + tail len place
        (VecLen, [10, 0, 1], &[]),
        // entry + constructor + nested call/cast + argument place
        (IdNew, [4, 0, 0], &[SmallNew]),
        (SmallNew, [4, 0, 0], &[]),
        // Pinned builtin derive lemma in the parent, not invented source.
        (IdEq, [6, 0, 0], &[SmallEq]),
        (SmallEq, [6, 0, 1], &[]),
        (IdVecIndex, [6, 0, 0], &[IdUsize, VecIndex]),
        (IdVecIndexMut, [6, 0, 0], &[IdUsize, VecIndexMut]),
        // entry + call + borrow + two dereferences + receiver + index place
        (VecIndex, [7, 0, 0], &[VecDeref, SliceIndex]),
        (VecIndexMut, [7, 0, 0], &[VecDerefMut, SliceIndexMut]),
        (VecDerefMut, [3, 0, 0], &[VecMutSlice]),
        (VecMutSlice, [9, 0, 0], &[VecMutPointer]),
        (VecMutPointer, [4, 0, 0], &[RawPointer]),
        (SliceIndexMut, [4, 0, 0], &[UsizeIndexMut]),
        (UsizeIndexMut, [6, 0, 0], &[]),
    ]
}

fn required_invariants(lemma: Lemma) -> BTreeSet<Invariant> {
    let mut result = BTreeSet::from([
        Invariant::ValidIds,
        Invariant::ExclusiveStable,
        Invariant::IdRepresentable,
    ]);
    if lemma == Lemma::SparseSetInsert {
        result.extend([
            Invariant::InitializedBuffers,
            Invariant::DenseDistinctBackMap,
            Invariant::EmptyEpoch,
        ]);
    }
    result
}

fn bindings(
    data: HelperFacts<'_>,
    rows: &BTreeMap<Site, Row>,
    required: &BTreeSet<Invariant>,
    premises: &Premises,
) -> Result<BTreeSet<HelperKey>, Refusal> {
    let frames: BTreeSet<HelperKey> = rows.values().filter_map(|row| row.frame.clone()).collect();
    for frame in &frames {
        let mut matches = data.helpers.iter().filter(|binding| binding.key == *frame);
        let binding = matches
            .next()
            .ok_or_else(|| Refusal::MissingHelper(frame.clone()))?;
        if matches.next().is_some() {
            return Err(Refusal::DuplicateHelper(frame.clone()));
        }
        if binding.receiver != frame.receiver {
            return Err(Refusal::ReceiverMismatch(frame.clone()));
        }
    }
    for invariant in required {
        let mut matches = premises
            .bindings
            .iter()
            .filter(|binding| binding.invariant == *invariant);
        let binding = matches
            .next()
            .ok_or(Refusal::MissingInvariant(*invariant))?;
        if matches.next().is_some()
            || data.owner.is_empty()
            || binding.owner != data.owner
            || binding.epoch != data.epoch
        {
            return Err(Refusal::InvariantMismatch(*invariant));
        }
    }
    Ok(frames)
}

fn dimensions(premises: &Premises, calls: Calls) -> Result<(), Refusal> {
    let active = match calls {
        Calls::Nfa { roots, dispatches } => roots != 0 || dispatches != 0,
        Calls::Sparse {
            attempts,
            successes,
            rhs,
        } => attempts != 0 || successes != 0 || rhs != 0,
    };
    if premises.states == 0 || premises.states > u64::from(i32::MAX.unsigned_abs()) {
        return Err(Refusal::InvalidFacts("states"));
    }
    if active && premises.epochs == 0 {
        return Err(Refusal::InvalidFacts("epochs"));
    }
    if premises.max_id >= premises.states {
        return Err(Refusal::InvalidFacts("max_id"));
    }
    if let Calls::Sparse {
        attempts,
        successes,
        rhs,
    } = calls
    {
        if premises.dense_len != premises.states {
            return Err(Refusal::InvalidFacts("dense_len"));
        }
        if premises.sparse_len != premises.states {
            return Err(Refusal::InvalidFacts("sparse_len"));
        }
        if premises.entry_len > premises.states {
            return Err(Refusal::InvalidFacts("entry_len"));
        }
        if successes > attempts {
            return Err(Refusal::InvalidFacts("successes"));
        }
        if rhs > attempts {
            return Err(Refusal::InvalidFacts("rhs"));
        }
        let population = premises
            .states
            .checked_mul(premises.epochs)
            .ok_or(Refusal::Overflow)?;
        let available = population
            .checked_sub(premises.entry_len)
            .ok_or(Refusal::InvalidFacts("entry_len"))?;
        if successes > available {
            return Err(Refusal::InvalidFacts("successes"));
        }
    }
    Ok(())
}

fn accumulate(
    site: Site,
    multiplicity: u64,
    table: &BTreeMap<Site, Row>,
    active: &mut BTreeSet<Site>,
    calls: &mut BTreeMap<Site, u64>,
    totals: &mut [u64; 3],
) -> Result<(), Refusal> {
    let row = table.get(&site).ok_or(Refusal::MissingSourceProof)?;
    if !active.insert(site) {
        return Err(Refusal::MissingSourceProof);
    }
    let count = calls.entry(site).or_default();
    *count = count.checked_add(multiplicity).ok_or(Refusal::Overflow)?;
    for (total, local) in totals.iter_mut().zip(row.local) {
        *total = total
            .checked_add(local.checked_mul(multiplicity).ok_or(Refusal::Overflow)?)
            .ok_or(Refusal::Overflow)?;
    }
    for callee in row.callees {
        accumulate(*callee, multiplicity, table, active, calls, totals)?;
    }
    active.remove(&site);
    Ok(())
}

fn roots(lemma: Lemma, calls: Calls) -> Result<Vec<(Site, u64)>, Refusal> {
    match (lemma, calls) {
        (Lemma::NfaStateAccess, Calls::Nfa { roots, dispatches }) => Ok(vec![
            (
                Site::State,
                roots.checked_add(dispatches).ok_or(Refusal::Overflow)?,
            ),
            (Site::Epsilon, roots),
        ]),
        (
            Lemma::SparseSetInsert,
            Calls::Sparse {
                attempts,
                successes,
                rhs,
            },
        ) => Ok(vec![
            (Site::Insert, attempts),
            (Site::InsertSuccess, successes),
            (Site::ContainsRhs, rhs),
        ]),
        (Lemma::NfaStateAccess, Calls::Sparse { .. })
        | (Lemma::SparseSetInsert, Calls::Nfa { .. }) => Err(Refusal::MissingSourceProof),
    }
}

// Use the same matched rows as work accumulation, but not its counted roots:
// fragments execute beneath their enclosing helper, never as stack roots.
// Invocation counts cannot remove any local structural obligation.
fn simultaneous_frame_paths(
    lemma: Lemma,
    table: &BTreeMap<Site, Row>,
) -> Result<BTreeSet<Vec<HelperKey>>, Refusal> {
    for (site, row) in table {
        let expected_frame = shared_frame(*site, lemma).or_else(|| specific_frame(*site));
        if row.frame != expected_frame
            || (row.frame.is_none() && !matches!(site, Site::InsertSuccess | Site::ContainsRhs))
            || row
                .callees
                .iter()
                .any(|callee| matches!(callee, Site::InsertSuccess | Site::ContainsRhs))
        {
            return Err(Refusal::MissingSourceProof);
        }
    }
    let roots: &[Site] = match lemma {
        Lemma::NfaStateAccess => &[Site::State, Site::Epsilon],
        Lemma::SparseSetInsert => &[Site::Insert],
    };
    let mut active = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut prefix = Vec::new();
    let mut paths = BTreeSet::new();
    for root in roots {
        collect_frame_paths(
            *root,
            table,
            &mut active,
            &mut visited,
            &mut prefix,
            &mut paths,
        )?;
    }
    // Shared callees are visited for every prefix. The census is an audit,
    // not memoization that would discard another caller's complete path.
    if visited.len() != table.len() {
        return Err(Refusal::MissingSourceProof);
    }
    let required_frames: BTreeSet<_> = table.values().filter_map(|row| row.frame.clone()).collect();
    let reached_frames = paths.iter().flatten().cloned().collect::<BTreeSet<_>>();
    if reached_frames != required_frames {
        return Err(Refusal::MissingSourceProof);
    }
    Ok(paths)
}

fn collect_frame_paths(
    site: Site,
    table: &BTreeMap<Site, Row>,
    active: &mut BTreeSet<Site>,
    visited: &mut BTreeSet<Site>,
    prefix: &mut Vec<HelperKey>,
    paths: &mut BTreeSet<Vec<HelperKey>>,
) -> Result<(), Refusal> {
    let row = table.get(&site).ok_or(Refusal::MissingSourceProof)?;
    if !active.insert(site) {
        return Err(Refusal::MissingSourceProof);
    }
    visited.insert(site);
    let caller_depth = prefix.len();
    if let Some(frame) = &row.frame {
        prefix.push(frame.clone());
    }
    // These are lexical portions of the already matched insert/contains
    // bodies, not ordinary callees. Their enclosing caller stays active.
    let fragments: &[Site] = match site {
        Site::Insert => &[Site::InsertSuccess],
        Site::Contains => &[Site::ContainsRhs],
        _ => &[],
    };
    if row.callees.is_empty() && fragments.is_empty() {
        paths.insert(prefix.clone());
    }
    for callee in row.callees.iter().chain(fragments) {
        collect_frame_paths(*callee, table, active, visited, prefix, paths)?;
    }
    // Each child returns to precisely this caller prefix before its sibling
    // starts. Only identical full paths deduplicate in the output set.
    prefix.truncate(caller_depth);
    active.remove(&site);
    Ok(())
}

#[cfg(test)]
pub(super) fn assert_frame_path_recipe_refusals() {
    for lemma in [Lemma::NfaStateAccess, Lemma::SparseSetInsert] {
        let table = rows(lemma).unwrap();
        assert!(!simultaneous_frame_paths(lemma, &table).unwrap().is_empty());
        // This is private recipe DATA corruption, not source authority. Every
        // present node (including frameless fragments) must stay reachable.
        for omitted in table.keys() {
            let mut changed = rows(lemma).unwrap();
            assert!(changed.remove(omitted).is_some());
            assert_eq!(
                simultaneous_frame_paths(lemma, &changed),
                Err(Refusal::MissingSourceProof),
                "missing {lemma:?} row {omitted:?}",
            );
        }
    }
    let cycles: [(Lemma, Site, &'static [Site]); 3] = [
        (Lemma::NfaStateAccess, Site::BytePointer, &[Site::States]),
        (Lemma::SparseSetInsert, Site::InsertSuccess, &[Site::Insert]),
        (Lemma::SparseSetInsert, Site::ContainsRhs, &[Site::Contains]),
    ];
    for (lemma, site, callees) in cycles {
        let mut changed = rows(lemma).unwrap();
        let row = changed.get_mut(&site).unwrap();
        assert_ne!(row.callees, callees);
        row.callees = callees;
        assert_eq!(
            simultaneous_frame_paths(lemma, &changed),
            Err(Refusal::MissingSourceProof),
            "cycle through {site:?}",
        );
    }
    let mut frameless = rows(Lemma::SparseSetInsert).unwrap();
    let len = frameless.get_mut(&Site::Len).unwrap();
    assert!(len.frame.take().is_some());
    assert_eq!(
        simultaneous_frame_paths(Lemma::SparseSetInsert, &frameless),
        Err(Refusal::MissingSourceProof),
        "ordinary helper cannot silently become frameless",
    );
    for site in [Site::InsertSuccess, Site::ContainsRhs] {
        let mut invented = rows(Lemma::SparseSetInsert).unwrap();
        let row = invented.get_mut(&site).unwrap();
        assert!(row.frame.is_none());
        row.frame = Some(key(SourceUnit::Sparse, "fragment", "invented", "&fragment"));
        assert_eq!(
            simultaneous_frame_paths(Lemma::SparseSetInsert, &invented),
            Err(Refusal::MissingSourceProof),
            "fragment {site:?} must not acquire a frame",
        );
    }
    let mut unused = rows(Lemma::SparseSetInsert).unwrap();
    assert!(
        unused
            .insert(
                Site::Epsilon,
                Row {
                    frame: Some(key(SourceUnit::Nfa, "State", "is_epsilon", "&State")),
                    local: [7, 3, 1],
                    callees: &[],
                }
            )
            .is_none()
    );
    assert_eq!(
        simultaneous_frame_paths(Lemma::SparseSetInsert, &unused),
        Err(Refusal::MissingSourceProof),
        "unused required frame cannot masquerade as reachable",
    );
}

pub(super) fn contract(
    data: HelperFacts<'_>,
    lemma: Lemma,
    calls: Calls,
    premises: &Premises,
) -> Result<LocalContract, Refusal> {
    let table = rows(lemma)?;
    let simultaneous_frame_paths = simultaneous_frame_paths(lemma, &table)?;
    let required = required_invariants(lemma);
    let required_frames = bindings(data, &table, &required, premises)?;
    dimensions(premises, calls)?;
    let mut counts = BTreeMap::new();
    let mut totals = [0_u64; 3];
    for (site, multiplicity) in roots(lemma, calls)? {
        accumulate(
            site,
            multiplicity,
            &table,
            &mut BTreeSet::new(),
            &mut counts,
            &mut totals,
        )?;
    }
    let projected: &[Site] = match lemma {
        Lemma::NfaStateAccess => &[Site::State, Site::States, Site::Epsilon],
        Lemma::SparseSetInsert => &[
            Site::Insert,
            Site::Contains,
            Site::Len,
            Site::Capacity,
            Site::IdNew,
            Site::IdVecIndex,
            Site::IdVecIndexMut,
            Site::IdUsize,
        ],
    };
    let mut local_calls = BTreeMap::new();
    for site in projected {
        let frame = table
            .get(site)
            .and_then(|row| row.frame.clone())
            .ok_or(Refusal::MissingSourceProof)?;
        let count = counts
            .get(site)
            .copied()
            .ok_or(Refusal::MissingSourceProof)?;
        local_calls.insert(frame, count);
    }
    let writes = match calls {
        Calls::Nfa { .. } => SymbolicWrites {
            state_id_elements: 0,
            usize_elements: 0,
        },
        Calls::Sparse { successes, .. } => SymbolicWrites {
            state_id_elements: successes.checked_mul(2).ok_or(Refusal::Overflow)?,
            usize_elements: successes,
        },
    };
    let [source, control, scalar] = totals;
    Ok(LocalContract {
        calls: local_calls,
        writes,
        ledger: WorkLedger::source_events(source, control, scalar),
        required_frames,
        simultaneous_frame_paths,
        unresolved_ordinary_helpers: BTreeSet::new(),
        required_buffer_bounds: BTreeSet::from(["scanned_bytes", "copied_bytes"]),
        required_invariants: required,
    })
}
