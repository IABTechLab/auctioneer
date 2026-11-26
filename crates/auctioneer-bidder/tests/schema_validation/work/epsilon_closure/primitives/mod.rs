//! Tests-first source contracts for two epsilon helper families, not qualification.
//!
//! Captured bytes, compiler records, receiver bindings and invariant records are
//! DATA. They grant no selected-program, source-completeness or target authority.
//! Strict supported recipes must reject semantic drift even after rehashing.
//! The local call table is not another work-unit taxonomy: `WorkLedger` owns the
//! fourteen units. Integer predicates use `ScalarComparison`; intrinsic index,
//! read and pointer entries use `SourceVisit`. Copies/fills require byte extents,
//! not machine-instruction counts. Symbolic writes below are not total copies.
//! Compiler-inserted bounds checks within intrinsic indexing are represented
//! by that operation's `SourceVisit`, not separate source predicate/control
//! events. Explicit Rust predicates in matched bodies still count normally.
//! Matched-source and valid-ID premises stay mandatory; this grouping assigns
//! no zero CPU cost and establishes no instruction or latency bound.
//! Supported positives close ordinary helper source/control/scalar events. The
//! other eleven units are not qualified here. Full byte extents and target
//! frame values remain separate requirements; inline is not zero frame.
//! `AllocationSuccess` excludes fatal OOM diagnostics, not returned/unwind cleanup.
//!
//! Actual use additionally requires the source owner to establish that the
//! unqualified `assert!` in `SparseSet::insert` resolves to the pinned core
//! builtin across crate roots, module ancestry and the selected prelude. The
//! twelve supplied files cannot establish that binding; receiver/invariant
//! strings and a matched local namespace do not discharge it. The actual
//! source window additionally recognizes the supported vendor ancestry;
//! it does not certify core exports, prelude resolution or effective cfgs.
//! Intrinsic and builtin-derive identities still need the authentic handoff.

use crate::schema_validation::handoff::SourceInputs;
use crate::schema_validation::operations::WorkLedger;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

// Reviewed manual builtin-derive lemma, not a digest whitelist or generated
// implementation fixture. Require this exact captured commit, builtin PartialEq,
// the actual one-field StateID(SmallIndex)/SmallIndex(u32) declarations, no
// packing/manual equality override, and the caller's == (eq, not ne).
// The struct field path has no packed-copy block; the first field is the whole
// equality expression, with no &&/fallback. Each generated body has six local
// source events: entry, equality, two field projections, two receiver places.
// StateID::eq calls SmallIndex::eq once; only the latter adds one scalar test.
// Official pinned source units supporting that case analysis:
// https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/compiler/rustc_builtin_macros/src/lib.rs#L124-L140
// https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/compiler/rustc_builtin_macros/src/deriving/cmp/partial_eq.rs#L41-L70
// https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/compiler/rustc_builtin_macros/src/deriving/cmp/partial_eq.rs#L131-L224
// https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/compiler/rustc_builtin_macros/src/deriving/generic/mod.rs#L1039-L1100
// https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/compiler/rustc_builtin_macros/src/deriving/generic/mod.rs#L1618-L1652
// https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/compiler/rustc_builtin_macros/src/deriving/generic/ty.rs#L200-L205
// https://github.com/rust-lang/rust/blob/48a229ceaefd4985c50990b14116b6d856af0985/compiler/rustc_hir_typeck/src/op.rs#L1190-L1195
const EQUALITY_COMPILER_COMMIT: &str = "48a229ceaefd4985c50990b14116b6d856af0985";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lemma {
    NfaStateAccess,
    SparseSetInsert,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::schema_validation) enum SourceUnit {
    Nfa,
    Sparse,
    Ids,
    Arc,
    Vec,
    RawVec,
    SliceIndex,
    NonNull,
    Unique,
    MutPtr,
    Intrinsics,
    Macros,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompilerData {
    executable: PathBuf,
    verbose: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceData {
    unit: SourceUnit,
    path: PathBuf,
    // Supplied snapshots only. Actual-source consumption must re-read each
    // fixed path around interpretation through the existing bounded reader.
    before: Vec<u8>,
    after: Vec<u8>,
    sha256: String,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::schema_validation) struct HelperKey {
    unit: SourceUnit,
    owner: &'static str,
    method: &'static str,
    receiver: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HelperData {
    key: HelperKey,
    // Actual concrete receiver binding, not just a method-name whitelist.
    receiver: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedData {
    compiler: CompilerData,
    workspace: PathBuf,
    owner: String,
    epoch: u64,
    sources: Vec<SourceData>,
    helpers: Vec<HelperData>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::schema_validation) enum Invariant {
    InitializedBuffers,
    ValidIds,
    DenseDistinctBackMap,
    EmptyEpoch,
    ExclusiveStable,
    IdRepresentable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InvariantData {
    invariant: Invariant,
    owner: String,
    epoch: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Premises {
    states: u64,
    epochs: u64,
    dense_len: u64,
    sparse_len: u64,
    entry_len: u64,
    max_id: u64,
    bindings: Vec<InvariantData>,
}

// Supplied event hypotheses, never observed-max or engine-run proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Calls {
    Nfa {
        roots: u64,
        dispatches: u64,
    },
    Sparse {
        attempts: u64,
        successes: u64,
        rhs: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SymbolicWrites {
    state_id_elements: u64,
    usize_elements: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct LocalContract {
    calls: BTreeMap<HelperKey, u64>,
    writes: SymbolicWrites,
    ledger: WorkLedger,
    required_frames: BTreeSet<HelperKey>,
    unresolved_ordinary_helpers: BTreeSet<HelperKey>,
    // Existing WorkLedger unit names: no new units and no total-copy claim.
    required_buffer_bounds: BTreeSet<&'static str>,
    required_invariants: BTreeSet<Invariant>,
}

#[derive(Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum Refusal {
    MissingSourceProof,
    MissingSource(SourceUnit),
    DuplicateSource(SourceUnit),
    SourceChanged(SourceUnit),
    UnsupportedBody(SourceUnit),
    VendorAncestry(&'static str),
    CompilerMismatch,
    SourcePath(SourceUnit),
    MissingHelper(HelperKey),
    DuplicateHelper(HelperKey),
    ReceiverMismatch(HelperKey),
    UnknownCall { unit: SourceUnit, callee: String },
    MissingInvariant(Invariant),
    InvariantMismatch(Invariant),
    InvalidFacts(&'static str),
    Overflow,
}

mod ancestry;
mod events;
mod source;

#[derive(Debug, Eq, PartialEq)]
pub(in crate::schema_validation) enum HelperSourceJoinRefusal {
    SourceInterpretation(Refusal),
}

// All fields are supplied conditional DATA, never observed caller facts.
#[derive(Clone, Debug)]
pub(in crate::schema_validation) struct ConditionalHelperDemand {
    lemma: Lemma,
    calls: Calls,
    owner: String,
    epoch: u64,
    helpers: Vec<HelperData>,
    premises: Premises,
}

impl ConditionalHelperDemand {
    fn facts(&self) -> events::HelperFacts<'_> {
        events::HelperFacts {
            owner: &self.owner,
            epoch: self.epoch,
            helpers: &self.helpers,
        }
    }
}

// Retain the real partial result, including unvalued frames/buffers/invariants.
// No constructor grants actual-use or source-completeness authority.
#[derive(Debug, Eq, PartialEq)]
pub(in crate::schema_validation) struct ConditionalHelperCostProjection {
    contract: LocalContract,
}

// Positive source setup only: no receiver, epoch, invocation or invariant
// facts are synthesized, and the event-cost interpreter is not invoked.
pub(in crate::schema_validation) fn check_consumed_source_setup(
    inputs: &SourceInputs<'_>,
) -> Result<(), HelperSourceJoinRefusal> {
    source::check_consumed_setup(inputs).map_err(HelperSourceJoinRefusal::SourceInterpretation)
}

// Source checks precede conditional cost interpretation, including at zero
// invocations. Retain the actual partial result and all outstanding duties.
pub(in crate::schema_validation) fn join_consumed_helper_costs(
    inputs: &SourceInputs<'_>,
    demand: &ConditionalHelperDemand,
) -> Result<ConditionalHelperCostProjection, HelperSourceJoinRefusal> {
    check_consumed_source_setup(inputs)?;
    let contract = events::contract(demand.facts(), demand.lemma, demand.calls, &demand.premises)
        .map_err(HelperSourceJoinRefusal::SourceInterpretation)?;
    Ok(ConditionalHelperCostProjection { contract })
}

// Conditional supplied-data interpretation only. Neither this entry nor its
// private children mint a selected-source or completeness capability. Actual
// use still needs captured-seed compiler identity and bounded rust-src reads
// bracketing interpretation through the existing source owner.
fn source_contract(
    data: &CapturedData,
    expected_compiler: &CompilerData,
    lemma: Lemma,
    calls: Calls,
    premises: &Premises,
) -> Result<LocalContract, Refusal> {
    source::check(data, expected_compiler)?;
    events::contract(events::HelperFacts::from(data), lemma, calls, premises)
}

#[cfg(test)]
pub(in crate::schema_validation) mod conditional_fixture {
    use super::*;

    // Fourteen literal components in WorkLedger declaration order. In
    // particular, a constructor mutation cannot change these expected values.
    const EXPECTED_LEDGERS: [[Option<u64>; 14]; 6] = [
        [
            Some(125),
            Some(10),
            None,
            None,
            None,
            Some(9),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        [
            Some(250),
            Some(20),
            None,
            None,
            None,
            Some(18),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        [
            Some(236),
            Some(10),
            None,
            None,
            None,
            Some(9),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        [
            Some(549),
            Some(7),
            None,
            None,
            None,
            Some(5),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        [
            Some(0),
            Some(0),
            None,
            None,
            None,
            Some(0),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
        [
            Some(0),
            Some(0),
            None,
            None,
            None,
            Some(0),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ],
    ];
    const EXPECTED_CALLS: [&[u64]; 6] = [
        &[1, 1, 1],
        &[2, 2, 2],
        &[2, 2, 1],
        &[2, 2, 3, 1, 1, 3, 2, 7],
        &[0, 0, 0],
        &[0, 0, 0, 0, 0, 0, 0, 0],
    ];
    const EXPECTED_WRITES: [[u64; 2]; 6] = [[0, 0], [0, 0], [0, 0], [2, 1], [0, 0], [0, 0]];

    // Fictional invocation/receiver/invariant hypotheses only. No facts are
    // inferred from a live validator, caller, allocation, receiver or epoch.
    fn demand(calls: Calls) -> ConditionalHelperDemand {
        let lemma = match calls {
            Calls::Nfa { .. } => Lemma::NfaStateAccess,
            Calls::Sparse { .. } => Lemma::SparseSetInsert,
        };
        let owner = "conditional_window_owner".to_owned();
        let epoch = 7;
        ConditionalHelperDemand {
            lemma,
            calls,
            owner: owner.clone(),
            epoch,
            helpers: helpers(lemma),
            premises: Premises {
                states: 4,
                epochs: 1,
                dense_len: 4,
                sparse_len: 4,
                entry_len: 0,
                max_id: 3,
                bindings: invariants(lemma)
                    .into_iter()
                    .map(|invariant| InvariantData {
                        invariant,
                        owner: owner.clone(),
                        epoch,
                    })
                    .collect(),
            },
        }
    }

    pub(in crate::schema_validation) fn conditional_demands() -> Vec<ConditionalHelperDemand> {
        [
            Calls::Nfa {
                roots: 1,
                dispatches: 0,
            },
            Calls::Nfa {
                roots: 2,
                dispatches: 0,
            },
            Calls::Nfa {
                roots: 1,
                dispatches: 1,
            },
            Calls::Sparse {
                attempts: 2,
                successes: 1,
                rhs: 1,
            },
            Calls::Nfa {
                roots: 0,
                dispatches: 0,
            },
            Calls::Sparse {
                attempts: 0,
                successes: 0,
                rhs: 0,
            },
        ]
        .into_iter()
        .map(demand)
        .collect()
    }

    fn projected_keys(lemma: Lemma) -> Vec<HelperKey> {
        match lemma {
            Lemma::NfaStateAccess => vec![
                key(SourceUnit::Nfa, "NFA", "state", "&NFA"),
                key(SourceUnit::Nfa, "NFA", "states", "&NFA"),
                key(SourceUnit::Nfa, "State", "is_epsilon", "&State"),
            ],
            Lemma::SparseSetInsert => vec![
                key(SourceUnit::Sparse, "SparseSet", "insert", "&mut SparseSet"),
                key(SourceUnit::Sparse, "SparseSet", "contains", "&SparseSet"),
                key(SourceUnit::Sparse, "SparseSet", "len", "&SparseSet"),
                key(SourceUnit::Sparse, "SparseSet", "capacity", "&SparseSet"),
                key(SourceUnit::Ids, "StateID", "new_unchecked", "usize"),
                key(SourceUnit::Ids, "Vec<T>", "index<StateID>", "&Vec<StateID>"),
                key(
                    SourceUnit::Ids,
                    "Vec<T>",
                    "index_mut<StateID>",
                    "&mut Vec<StateID>",
                ),
                key(SourceUnit::Ids, "StateID", "as_usize", "&StateID"),
            ],
        }
    }

    fn assert_projection(index: usize, demand: &ConditionalHelperDemand, actual: &LocalContract) {
        assert_eq!(actual.ledger.components(), EXPECTED_LEDGERS[index]);
        let keys = projected_keys(demand.lemma);
        assert_eq!(keys.len(), EXPECTED_CALLS[index].len());
        let expected_calls: BTreeMap<_, _> = keys
            .into_iter()
            .zip(EXPECTED_CALLS[index].iter().copied())
            .collect();
        assert_eq!(actual.calls, expected_calls);
        assert_eq!(
            [
                actual.writes.state_id_elements,
                actual.writes.usize_elements
            ],
            EXPECTED_WRITES[index]
        );
        let expected_frames: BTreeSet<_> = helpers(demand.lemma)
            .into_iter()
            .map(|binding| binding.key)
            .collect();
        assert_eq!(actual.required_frames, expected_frames);
        assert!(actual.unresolved_ordinary_helpers.is_empty());
        assert_eq!(
            actual.required_buffer_bounds,
            BTreeSet::from(["scanned_bytes", "copied_bytes"])
        );
        assert_eq!(actual.required_invariants, invariants(demand.lemma));
    }

    // Verify that supplied hypotheses are valid independently of the missing
    // source/cost join, using exactly the already established event evaluator.
    pub(in crate::schema_validation) fn assert_demand_setup(demands: &[ConditionalHelperDemand]) {
        assert_eq!(demands.len(), 6);
        for (index, demand) in demands.iter().enumerate() {
            let local =
                events::contract(demand.facts(), demand.lemma, demand.calls, &demand.premises)
                    .expect("explicit conditional hypotheses are valid DATA");
            assert_projection(index, demand, &local);
        }
    }

    pub(in crate::schema_validation) fn assert_cost_projections(
        demands: &[ConditionalHelperDemand],
        results: &[Result<ConditionalHelperCostProjection, HelperSourceJoinRefusal>],
    ) {
        assert_eq!(demands.len(), 6);
        assert_eq!(results.len(), demands.len());
        for (index, (demand, actual)) in demands.iter().zip(results).enumerate() {
            assert!(
                actual.is_ok(),
                "conditional helper cost join {index}: {actual:?}"
            );
            let projection = actual.as_ref().unwrap();
            assert_projection(index, demand, &projection.contract);
        }
    }

    pub(in crate::schema_validation) fn conditional_controls()
    -> Vec<(ConditionalHelperDemand, HelperSourceJoinRefusal)> {
        let zero = demand(Calls::Nfa {
            roots: 0,
            dispatches: 0,
        });
        let mut missing = zero.clone();
        assert!(
            missing
                .premises
                .bindings
                .iter()
                .any(|binding| binding.invariant == Invariant::ValidIds)
        );
        missing
            .premises
            .bindings
            .retain(|binding| binding.invariant != Invariant::ValidIds);
        let mut receiver = zero.clone();
        let helper = receiver.helpers.first_mut().unwrap();
        assert_eq!(helper.receiver, helper.key.receiver);
        let receiver_key = helper.key.clone();
        helper.receiver = "&unrelated_receiver".to_owned();
        let mut epoch = zero;
        let binding = epoch
            .premises
            .bindings
            .iter_mut()
            .find(|binding| binding.invariant == Invariant::ValidIds)
            .unwrap();
        assert_eq!(binding.epoch, 7);
        binding.epoch = 8;
        let overflow = demand(Calls::Nfa {
            roots: u64::MAX,
            dispatches: 0,
        });
        let controls = vec![
            (missing, Refusal::MissingInvariant(Invariant::ValidIds)),
            (receiver, Refusal::ReceiverMismatch(receiver_key)),
            (epoch, Refusal::InvariantMismatch(Invariant::ValidIds)),
            (overflow, Refusal::Overflow),
        ];
        controls
            .into_iter()
            .map(|(demand, expected)| {
                let observed =
                    events::contract(demand.facts(), demand.lemma, demand.calls, &demand.premises);
                assert_eq!(observed.as_ref().err(), Some(&expected));
                // Retain the independently authored refusal, not the evaluator's.
                (
                    demand,
                    HelperSourceJoinRefusal::SourceInterpretation(expected),
                )
            })
            .collect()
    }

    pub(super) fn key(
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

    // Fixed concrete helper/frame census, not an AST evaluator. Source-event
    // costs must close; these same frame slots remain unvalued target duties.
    pub(super) fn helpers(lemma: Lemma) -> Vec<HelperData> {
        use SourceUnit::*;
        let mut keys = vec![
            key(Ids, "StateID", "as_usize", "&StateID"),
            key(Ids, "SmallIndex", "as_usize", "&SmallIndex"),
            key(RawVec, "RawVec<T>", "ptr", "&RawVec<StateID>"),
            key(RawVec, "RawVecInner", "ptr::<StateID>", "&RawVecInner"),
            key(RawVec, "RawVecInner", "non_null::<StateID>", "&RawVecInner"),
            key(Unique, "Unique<T>", "cast::<StateID>", "Unique<u8>"),
            key(Unique, "Unique<T>", "as_non_null_ptr", "Unique<StateID>"),
            key(NonNull, "NonNull<T>", "cast::<StateID>", "NonNull<u8>"),
            key(NonNull, "NonNull<T>", "as_ptr", "NonNull<u8>"),
            key(NonNull, "NonNull<T>", "as_ptr", "NonNull<StateID>"),
        ];
        match lemma {
            Lemma::NfaStateAccess => {
                // The NFA vector instantiation is State, not StateID.
                keys.retain(|helper| !matches!(helper.unit, RawVec | Unique | NonNull));
                keys.extend([
                    key(Nfa, "NFA", "state", "&NFA"),
                    key(Nfa, "NFA", "states", "&NFA"),
                    key(Nfa, "State", "is_epsilon", "&State"),
                    key(Ids, "[T]", "index<StateID>", "&[State]"),
                    key(Arc, "Arc<T>", "deref", "&Arc<nfa::Inner>"),
                    key(Arc, "Arc<T>", "inner", "&Arc<nfa::Inner>"),
                    key(
                        NonNull,
                        "NonNull<T>",
                        "as_ref",
                        "&NonNull<ArcInner<nfa::Inner>>",
                    ),
                    key(
                        NonNull,
                        "NonNull<T>",
                        "as_ptr",
                        "NonNull<ArcInner<nfa::Inner>>",
                    ),
                    key(MutPtr, "*mut T", "cast_const", "*mut ArcInner<nfa::Inner>"),
                    key(Vec, "Vec<T>", "deref", "&Vec<State>"),
                    key(Vec, "Vec<T>", "as_slice", "&Vec<State>"),
                    key(Vec, "Vec<T>", "as_ptr", "&Vec<State>"),
                    key(RawVec, "RawVec<T>", "ptr", "&RawVec<State>"),
                    key(RawVec, "RawVecInner", "ptr::<State>", "&RawVecInner"),
                    key(RawVec, "RawVecInner", "non_null::<State>", "&RawVecInner"),
                    key(Unique, "Unique<T>", "cast::<State>", "Unique<u8>"),
                    key(Unique, "Unique<T>", "as_non_null_ptr", "Unique<State>"),
                    key(NonNull, "NonNull<T>", "cast::<State>", "NonNull<u8>"),
                    key(NonNull, "NonNull<T>", "as_ptr", "NonNull<u8>"),
                    key(NonNull, "NonNull<T>", "as_ptr", "NonNull<State>"),
                    key(SliceIndex, "[T]", "index<usize>", "&[State]"),
                    key(SliceIndex, "usize", "index", "&[State]"),
                ]);
            }
            Lemma::SparseSetInsert => keys.extend([
                key(Sparse, "SparseSet", "insert", "&mut SparseSet"),
                key(Sparse, "SparseSet", "contains", "&SparseSet"),
                key(Sparse, "SparseSet", "len", "&SparseSet"),
                key(Sparse, "SparseSet", "capacity", "&SparseSet"),
                key(Ids, "StateID", "new_unchecked", "usize"),
                key(Ids, "SmallIndex", "new_unchecked", "usize"),
                key(Ids, "StateID", "eq", "&StateID"),
                key(Ids, "SmallIndex", "eq", "&SmallIndex"),
                key(Ids, "Vec<T>", "index<StateID>", "&Vec<StateID>"),
                key(Ids, "Vec<T>", "index_mut<StateID>", "&mut Vec<StateID>"),
                key(Vec, "Vec<T>", "index<usize>", "&Vec<StateID>"),
                key(Vec, "Vec<T>", "index_mut<usize>", "&mut Vec<StateID>"),
                key(Vec, "Vec<T>", "deref", "&Vec<StateID>"),
                key(Vec, "Vec<T>", "deref_mut", "&mut Vec<StateID>"),
                key(Vec, "Vec<T>", "as_slice", "&Vec<StateID>"),
                key(Vec, "Vec<T>", "as_mut_slice", "&mut Vec<StateID>"),
                key(Vec, "Vec<T>", "as_ptr", "&Vec<StateID>"),
                key(Vec, "Vec<T>", "as_mut_ptr", "&mut Vec<StateID>"),
                key(Vec, "Vec<T>", "len", "&Vec<StateID>"),
                key(SliceIndex, "[T]", "index<usize>", "&[StateID]"),
                key(SliceIndex, "[T]", "index_mut<usize>", "&mut [StateID]"),
                key(SliceIndex, "usize", "index", "&[StateID]"),
                key(SliceIndex, "usize", "index_mut", "&mut [StateID]"),
            ]),
        }
        keys.into_iter()
            .map(|key| HelperData {
                receiver: key.receiver.to_owned(),
                key,
            })
            .collect()
    }

    pub(super) fn invariants(lemma: Lemma) -> BTreeSet<Invariant> {
        let mut required = BTreeSet::from([
            Invariant::ValidIds,
            Invariant::ExclusiveStable,
            Invariant::IdRepresentable,
        ]);
        if lemma == Lemma::SparseSetInsert {
            required.extend([
                Invariant::InitializedBuffers,
                Invariant::DenseDistinctBackMap,
                Invariant::EmptyEpoch,
            ]);
        }
        required
    }
}

#[cfg(test)]
mod tests {
    use super::conditional_fixture::{helpers, invariants, key};
    use super::*;
    use crate::schema_build::selected::probe_compiler;
    use crate::schema_residency::read_regular;
    use proc_macro2::TokenStream;
    use quote::ToTokens as _;
    use sha2::{Digest as _, Sha256};
    use std::mem;
    use std::path::Path;
    use std::str::from_utf8;
    use syn::punctuated::Punctuated;

    fn digest(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    // Exactly one existing compiler capture per test setup, never a global
    // cache. These are fixture DATA, not a compiler/source binding capability.
    // Actual integration must derive expected compiler identity from the
    // CapturedSeed.manifest_bytes build.compiler_verbose and
    // build_environment.RUSTC fields. A cloned probe result is not that proof.
    // Consecutive setup reads below do not prove a stable consumption window;
    // the parent capture does not cover external rust-src. No replacement
    // loader or new capture protocol is introduced here.
    fn capture() -> (CapturedData, CompilerData, Premises) {
        let (executable, verbose) =
            probe_compiler().expect("existing bounded compiler capture succeeds");
        let compiler = CompilerData {
            executable,
            verbose,
        };
        let sysroot = compiler
            .executable
            .parent()
            .and_then(Path::parent)
            .expect("captured compiler has a bin/sysroot parent");
        let library = sysroot.join("lib/rustlib/src/rust/library");
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("crate is below the workspace crates directory")
            .to_owned();
        let mut sources = Vec::new();
        for (unit, relative, standard) in [
            (
                SourceUnit::Nfa,
                "vendor/regex-automata/src/nfa/thompson/nfa.rs",
                false,
            ),
            (
                SourceUnit::Sparse,
                "vendor/regex-automata/src/util/sparse_set.rs",
                false,
            ),
            (
                SourceUnit::Ids,
                "vendor/regex-automata/src/util/primitives.rs",
                false,
            ),
            (SourceUnit::Arc, "alloc/src/sync.rs", true),
            (SourceUnit::Vec, "alloc/src/vec/mod.rs", true),
            (SourceUnit::RawVec, "alloc/src/raw_vec/mod.rs", true),
            (SourceUnit::SliceIndex, "core/src/slice/index.rs", true),
            (SourceUnit::NonNull, "core/src/ptr/non_null.rs", true),
            (SourceUnit::Unique, "core/src/ptr/unique.rs", true),
            (SourceUnit::MutPtr, "core/src/ptr/mut_ptr.rs", true),
            (SourceUnit::Intrinsics, "core/src/intrinsics/mod.rs", true),
            (SourceUnit::Macros, "core/src/macros/mod.rs", true),
        ] {
            let path = if standard {
                library.join(relative)
            } else {
                workspace.join(relative)
            };
            let before = read_regular(&path)
                .expect("required captured source is regular and bounded; no fallback");
            let after = read_regular(&path)
                .expect("required source can be re-read through its existing owner");
            assert_eq!(before, after);
            sources.push(SourceData {
                unit,
                path,
                sha256: digest(&before),
                before,
                after,
            });
        }
        let all = helpers(Lemma::NfaStateAccess)
            .into_iter()
            .chain(helpers(Lemma::SparseSetInsert))
            .map(|binding| (binding.key.clone(), binding))
            .collect::<BTreeMap<_, _>>();
        let owner = "fake_closure_owner".to_owned();
        let epoch = 7;
        let premises = Premises {
            states: 4,
            epochs: 1,
            dense_len: 4,
            sparse_len: 4,
            entry_len: 0,
            max_id: 3,
            bindings: invariants(Lemma::SparseSetInsert)
                .into_iter()
                .map(|invariant| InvariantData {
                    invariant,
                    owner: owner.clone(),
                    epoch,
                })
                .collect(),
        };
        let expected = compiler.clone();
        (
            CapturedData {
                compiler,
                workspace,
                owner,
                epoch,
                sources,
                helpers: all.into_values().collect(),
            },
            expected,
            premises,
        )
    }

    fn nfa() -> Calls {
        Calls::Nfa {
            roots: 1,
            dispatches: 1,
        }
    }
    fn sparse() -> Calls {
        Calls::Sparse {
            attempts: 2,
            successes: 1,
            rhs: 1,
        }
    }

    fn local_calls(lemma: Lemma, values: &[u64]) -> BTreeMap<HelperKey, u64> {
        let keys = match lemma {
            Lemma::NfaStateAccess => vec![
                key(SourceUnit::Nfa, "NFA", "state", "&NFA"),
                key(SourceUnit::Nfa, "NFA", "states", "&NFA"),
                key(SourceUnit::Nfa, "State", "is_epsilon", "&State"),
            ],
            Lemma::SparseSetInsert => vec![
                key(SourceUnit::Sparse, "SparseSet", "insert", "&mut SparseSet"),
                key(SourceUnit::Sparse, "SparseSet", "contains", "&SparseSet"),
                key(SourceUnit::Sparse, "SparseSet", "len", "&SparseSet"),
                key(SourceUnit::Sparse, "SparseSet", "capacity", "&SparseSet"),
                key(SourceUnit::Ids, "StateID", "new_unchecked", "usize"),
                key(SourceUnit::Ids, "Vec<T>", "index<StateID>", "&Vec<StateID>"),
                key(
                    SourceUnit::Ids,
                    "Vec<T>",
                    "index_mut<StateID>",
                    "&mut Vec<StateID>",
                ),
                key(SourceUnit::Ids, "StateID", "as_usize", "&StateID"),
            ],
        };
        assert_eq!(keys.len(), values.len());
        keys.into_iter().zip(values.iter().copied()).collect()
    }

    fn syntax(data: &CapturedData, unit: SourceUnit) -> syn::File {
        let source = data
            .sources
            .iter()
            .find(|source| source.unit == unit)
            .unwrap();
        syn::parse_file(from_utf8(&source.before).unwrap()).unwrap()
    }

    fn normalize_attributes(attributes: &mut Vec<syn::Attribute>) {
        attributes.retain(|attribute| {
            !matches!(&attribute.meta, syn::Meta::NameValue(meta)
                if attribute.path().is_ident("doc")
                && matches!(attribute.style, syn::AttrStyle::Outer)
                && matches!(&meta.value, syn::Expr::Lit(value) if matches!(&value.lit, syn::Lit::Str(_))))
        });
        for attribute in attributes {
            if attribute.path().is_ident("derive") {
                let parsed_paths = attribute
                    .parse_args_with(Punctuated::<syn::Path, syn::Token![,]>::parse_terminated)
                    .unwrap();
                let paths: Punctuated<syn::Path, syn::Token![,]> =
                    parsed_paths.into_iter().collect();
                let syn::Meta::List(list) = &mut attribute.meta else {
                    panic!("derive list");
                };
                list.tokens = paths.to_token_stream();
            }
        }
    }

    // Independent source projections, not caller-provided edge labels. Named
    // projections omit only fields not traversed by these helper families.
    fn assert_declaration(data: &CapturedData, unit: SourceUnit, mut expected: syn::ItemStruct) {
        let source = syntax(data, unit);
        let mut actual = source
            .items
            .iter()
            .find_map(|entry| {
                let syn::Item::Struct(item) = entry else {
                    return None;
                };
                (item.ident == expected.ident).then(|| item.clone())
            })
            .expect("actual declaration for the independently quoted recipe");
        match (&mut actual.fields, &expected.fields) {
            (syn::Fields::Named(actual_fields), syn::Fields::Named(expected_fields)) => {
                actual_fields.named = mem::take(&mut actual_fields.named)
                    .into_iter()
                    .filter(|field| {
                        expected_fields
                            .named
                            .iter()
                            .any(|wanted| wanted.ident == field.ident)
                    })
                    .collect();
            }
            (syn::Fields::Unnamed(actual_fields), syn::Fields::Unnamed(expected_fields)) => {
                assert_eq!(actual_fields.unnamed.len(), expected_fields.unnamed.len());
            }
            _ => panic!("declaration field shape changed"),
        }
        // Ignore only literal docs and semantically optional trailing commas.
        for declaration in [&mut actual, &mut expected] {
            normalize_attributes(&mut declaration.attrs);
            for field in &mut declaration.fields {
                normalize_attributes(&mut field.attrs);
            }
            match &mut declaration.fields {
                syn::Fields::Named(fields) => {
                    fields.named = mem::take(&mut fields.named).into_iter().collect();
                }
                syn::Fields::Unnamed(fields) => {
                    fields.unnamed = mem::take(&mut fields.unnamed).into_iter().collect();
                }
                syn::Fields::Unit => {}
            }
        }
        assert_eq!(
            actual.to_token_stream().to_string(),
            expected.to_token_stream().to_string()
        );
    }

    fn assert_field_type(
        data: &CapturedData,
        unit: SourceUnit,
        owner: &str,
        member: &str,
        expected: &syn::Type,
    ) {
        let source = syntax(data, unit);
        let declaration = source
            .items
            .iter()
            .find_map(|entry| {
                let syn::Item::Struct(item) = entry else {
                    return None;
                };
                (item.ident == owner).then_some(item)
            })
            .expect("actual owner declaration");
        let field = declaration
            .fields
            .iter()
            .find(|field| field.ident.as_ref().is_some_and(|name| name == member))
            .expect("actual traversed field");
        assert_eq!(
            field.ty.to_token_stream().to_string(),
            expected.to_token_stream().to_string()
        );
    }

    fn assert_import(data: &CapturedData, unit: SourceUnit, name: &str, expected: &syn::Path) {
        fn leaves(tree: &syn::UseTree, prefix: &TokenStream, result: &mut Vec<(String, String)>) {
            match tree {
                syn::UseTree::Path(path) => {
                    let ident = &path.ident;
                    leaves(&path.tree, &quote::quote!(#prefix #ident ::), result);
                }
                syn::UseTree::Group(group) => {
                    for item in &group.items {
                        leaves(item, prefix, result);
                    }
                }
                syn::UseTree::Name(item) => {
                    let ident = &item.ident;
                    result.push((ident.to_string(), quote::quote!(#prefix #ident).to_string()));
                }
                syn::UseTree::Rename(item) => {
                    let ident = &item.ident;
                    result.push((
                        item.rename.to_string(),
                        quote::quote!(#prefix #ident).to_string(),
                    ));
                }
                syn::UseTree::Glob(_) => {}
            }
        }
        let mut imports = Vec::new();
        for item in syntax(data, unit).items {
            if let syn::Item::Use(import) = item {
                let mut bindings = Vec::new();
                leaves(&import.tree, &TokenStream::new(), &mut bindings);
                if bindings.iter().any(|(local, _)| local == name) {
                    let expected_attributes = if unit == SourceUnit::Ids {
                        quote::quote!(#[cfg(feature = "alloc")])
                    } else {
                        proc_macro2::TokenStream::new()
                    };
                    let attributes = &import.attrs;
                    assert_eq!(
                        quote::quote!(#(#attributes)*).to_string(),
                        expected_attributes.to_string()
                    );
                    assert!(import.leading_colon.is_none());
                    assert!(matches!(import.vis, syn::Visibility::Inherited));
                }
                imports.extend(bindings);
            }
        }
        let bindings: Vec<_> = imports
            .into_iter()
            .filter(|(local, _)| local == name)
            .map(|(_, path)| path)
            .collect();
        assert_eq!(bindings, vec![expected.to_token_stream().to_string()]);
    }

    fn assert_owner_edges(data: &CapturedData) {
        assert_owner_declarations(data);
        assert_owner_imports(data);
        assert_owner_fields(data);
    }

    fn assert_owner_declarations(data: &CapturedData) {
        assert_declaration(
            data,
            SourceUnit::Nfa,
            syn::parse_quote!(
                #[derive(Clone)]
                pub struct NFA(Arc<Inner>);
            ),
        );
        assert_declaration(
            data,
            SourceUnit::Nfa,
            syn::parse_quote!(
                #[derive(Default)]
                pub(super) struct Inner {
                    states: Vec<State>,
                }
            ),
        );
        assert_declaration(
            data,
            SourceUnit::Sparse,
            syn::parse_quote!(
                #[derive(Clone)]
                pub(crate) struct SparseSet {
                    len: usize,
                    dense: Vec<StateID>,
                    sparse: Vec<StateID>,
                }
            ),
        );
        assert_declaration(
            data,
            SourceUnit::Ids,
            syn::parse_quote!(
                #[derive(Clone, Copy, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
                #[repr(transparent)]
                pub struct StateID(SmallIndex);
            ),
        );
        assert_declaration(
            data,
            SourceUnit::Ids,
            syn::parse_quote!(
                #[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, PartialOrd, Ord)]
                #[repr(transparent)]
                pub struct SmallIndex(u32);
            ),
        );
    }

    fn assert_owner_imports(data: &CapturedData) {
        for (unit, name, expected) in [
            (SourceUnit::Nfa, "Arc", syn::parse_quote!(alloc::sync::Arc)),
            (SourceUnit::Nfa, "Vec", syn::parse_quote!(alloc::vec::Vec)),
            (
                SourceUnit::Nfa,
                "StateID",
                syn::parse_quote!(crate::util::primitives::StateID),
            ),
            (
                SourceUnit::Sparse,
                "Vec",
                syn::parse_quote!(alloc::vec::Vec),
            ),
            (
                SourceUnit::Sparse,
                "StateID",
                syn::parse_quote!(crate::util::primitives::StateID),
            ),
            (SourceUnit::Ids, "Vec", syn::parse_quote!(alloc::vec::Vec)),
            (
                SourceUnit::Arc,
                "NonNull",
                syn::parse_quote!(core::ptr::NonNull),
            ),
            (
                SourceUnit::Vec,
                "RawVec",
                syn::parse_quote!(crate::raw_vec::RawVec),
            ),
            (
                SourceUnit::Vec,
                "Index",
                syn::parse_quote!(core::ops::Index),
            ),
            (
                SourceUnit::Vec,
                "IndexMut",
                syn::parse_quote!(core::ops::IndexMut),
            ),
            (
                SourceUnit::Vec,
                "SliceIndex",
                syn::parse_quote!(core::slice::SliceIndex),
            ),
            (
                SourceUnit::RawVec,
                "Unique",
                syn::parse_quote!(core::ptr::Unique),
            ),
            (
                SourceUnit::RawVec,
                "NonNull",
                syn::parse_quote!(core::ptr::NonNull),
            ),
            (
                SourceUnit::Unique,
                "NonNull",
                syn::parse_quote!(crate::ptr::NonNull),
            ),
        ] {
            assert_import(data, unit, name, &expected);
        }
    }

    fn assert_owner_fields(data: &CapturedData) {
        for (unit, owner, member, expected) in [
            (
                SourceUnit::Arc,
                "Arc",
                "ptr",
                syn::parse_quote!(NonNull<ArcInner<T>>),
            ),
            (SourceUnit::Arc, "ArcInner", "data", syn::parse_quote!(T)),
            (
                SourceUnit::Vec,
                "Vec",
                "buf",
                syn::parse_quote!(RawVec<T, A>),
            ),
            (SourceUnit::Vec, "Vec", "len", syn::parse_quote!(usize)),
            (
                SourceUnit::RawVec,
                "RawVec",
                "inner",
                syn::parse_quote!(RawVecInner<A>),
            ),
            (
                SourceUnit::RawVec,
                "RawVecInner",
                "ptr",
                syn::parse_quote!(Unique<u8>),
            ),
            (
                SourceUnit::Unique,
                "Unique",
                "pointer",
                syn::parse_quote!(NonNull<T>),
            ),
            (
                SourceUnit::NonNull,
                "NonNull",
                "pointer",
                syn::parse_quote!(crate::pattern_type!(*const T is !null)),
            ),
        ] {
            assert_field_type(data, unit, owner, member, &expected);
        }
    }

    // Select the single literal macro arm and whole top-level template items.
    // No macro expansion, runtime matcher or generated impl is executed.
    fn index_template(data: &CapturedData) -> Vec<proc_macro2::TokenStream> {
        use proc_macro2::{Delimiter, TokenStream, TokenTree};
        let source = syntax(data, SourceUnit::Ids);
        let definition = source
            .items
            .iter()
            .find_map(|entry| {
                let syn::Item::Macro(item) = entry else {
                    return None;
                };
                (item
                    .ident
                    .as_ref()
                    .is_some_and(|name| name == "index_type_impls"))
                .then_some(item)
            })
            .expect("actual ID macro definition");
        assert!(definition.mac.path.is_ident("macro_rules"));
        assert!(definition.attrs.is_empty());
        let tokens: Vec<_> = definition.mac.tokens.clone().into_iter().collect();
        let [
            TokenTree::Group(parameters),
            TokenTree::Punct(equal),
            TokenTree::Punct(arrow),
            TokenTree::Group(body),
            TokenTree::Punct(semi),
        ] = tokens.as_slice()
        else {
            panic!("single supported macro arm");
        };
        assert_eq!(
            (
                parameters.delimiter(),
                equal.as_char(),
                arrow.as_char(),
                body.delimiter(),
                semi.as_char()
            ),
            (Delimiter::Parenthesis, '=', '>', Delimiter::Brace, ';')
        );
        assert_eq!(
            parameters.stream().to_string(),
            quote::quote!($name:ident, $err:ident, $iter:ident, $withiter:ident).to_string()
        );
        let mut items = Vec::new();
        let mut current = TokenStream::new();
        for token in body.stream() {
            let ends_item = matches!(&token, TokenTree::Group(group) if group.delimiter() == Delimiter::Brace)
                || matches!(&token, TokenTree::Punct(punctuation) if punctuation.as_char() == ';');
            current.extend([token]);
            if ends_item {
                items.push(mem::take(&mut current));
            }
        }
        assert!(current.is_empty());
        let invocations: Vec<_> = source
            .items
            .iter()
            .filter_map(|entry| {
                let syn::Item::Macro(invocation) = entry else {
                    return None;
                };
                invocation
                    .mac
                    .path
                    .is_ident("index_type_impls")
                    .then(|| invocation.mac.tokens.to_string())
            })
            .collect();
        assert_eq!(
            invocations,
            vec![
                quote::quote!(PatternID, PatternIDError, PatternIDIter, WithPatternIDIter)
                    .to_string(),
                quote::quote!(StateID, StateIDError, StateIDIter, WithStateIDIter).to_string(),
            ]
        );
        items
    }

    fn assert_index_edges(data: &CapturedData) {
        use proc_macro2::{Punct, Spacing};

        let items = index_template(data);
        // The source lexer joins '<' to the following '$' in <$name>;
        // quote! emits a literal '<' alone. Pin that one boundary explicitly
        // instead of normalizing punctuation or weakening whole-item equality.
        let parameter_open = Punct::new('<', Spacing::Joint);
        for expected in [
            quote::quote! { impl<T> core::ops::Index #parameter_open $name> for [T] {
                type Output = T;
                #[inline] fn index(&self, index: $name) -> &T { &self[index.as_usize()] }
            } },
            quote::quote! { impl<T> core::ops::IndexMut #parameter_open $name> for [T] {
                #[inline] fn index_mut(&mut self, index: $name) -> &mut T { &mut self[index.as_usize()] }
            } },
            quote::quote! { #[cfg(feature = "alloc")] impl<T> core::ops::Index #parameter_open $name> for Vec<T> {
                type Output = T;
                #[inline] fn index(&self, index: $name) -> &T { &self[index.as_usize()] }
            } },
            quote::quote! { #[cfg(feature = "alloc")] impl<T> core::ops::IndexMut #parameter_open $name> for Vec<T> {
                #[inline] fn index_mut(&mut self, index: $name) -> &mut T { &mut self[index.as_usize()] }
            } },
        ] {
            assert_eq!(
                items
                    .iter()
                    .filter(|item| item.to_string() == expected.to_string())
                    .count(),
                1,
                "expected literal index item: {}; actual top-level macro items: {:?}",
                expected,
                items.iter().map(ToString::to_string).collect::<Vec<_>>()
            );
        }
    }

    fn assert_body(data: &CapturedData, unit: SourceUnit, owner: &str, expected: &syn::ImplItemFn) {
        let source = data
            .sources
            .iter()
            .find(|source| source.unit == unit)
            .unwrap();
        let syntax = syn::parse_file(from_utf8(&source.before).unwrap()).unwrap();
        let actual = syntax
            .items
            .iter()
            .find_map(|entry| {
                let syn::Item::Impl(item) = entry else {
                    return None;
                };
                if item.self_ty.to_token_stream().to_string() != owner {
                    return None;
                }
                let mut header = item.clone();
                header.items.clear();
                let owner_type: syn::Type = syn::parse_str(owner).unwrap();
                let expected_header: syn::ItemImpl = syn::parse_quote!(impl #owner_type {});
                assert_eq!(
                    header.to_token_stream().to_string(),
                    expected_header.to_token_stream().to_string()
                );
                item.items.iter().find_map(|member| {
                    let syn::ImplItem::Fn(method) = member else {
                        return None;
                    };
                    (method.sig.ident == expected.sig.ident).then_some(method)
                })
            })
            .expect("independent source recipe has its actual impl/method");
        assert_eq!(
            actual.sig.to_token_stream().to_string(),
            expected.sig.to_token_stream().to_string()
        );
        assert_eq!(
            actual.block.to_token_stream().to_string(),
            expected.block.to_token_stream().to_string()
        );
    }

    fn assert_local(
        data: &CapturedData,
        expected_compiler: &CompilerData,
        premises: &Premises,
        lemma: Lemma,
        calls: Calls,
        expected_counts: (&[u64], [u64; 3]),
        writes: SymbolicWrites,
    ) {
        let required_frames = helpers(lemma)
            .into_iter()
            .map(|binding| binding.key)
            .collect::<BTreeSet<_>>();
        // Literal independent oracles, not the evaluator's family formulas.
        let (values, [source, control, scalar]) = expected_counts;
        let ledger = WorkLedger::source_events(source, control, scalar);
        let expected = LocalContract {
            calls: local_calls(lemma, values),
            writes,
            ledger,
            unresolved_ordinary_helpers: BTreeSet::new(),
            required_buffer_bounds: BTreeSet::from(["scanned_bytes", "copied_bytes"]),
            required_frames,
            required_invariants: invariants(lemma),
        };
        let view = |local: LocalContract| {
            (
                local.calls,
                (local.writes.state_id_elements, local.writes.usize_elements),
                local.ledger,
                local.required_frames,
                local.unresolved_ordinary_helpers,
                local.required_buffer_bounds,
                local.required_invariants,
            )
        };
        assert_eq!(
            source_contract(data, expected_compiler, lemma, calls, premises).map(view),
            Ok(view(expected)),
            "primitive source stage: {:?}",
            super::source::failure_stage(data, expected_compiler)
        );
    }

    // Fixture editing only: no recognition, traversal execution or invariant
    // implementation. Rehash both copies to test that hashes grant no approval.
    fn edit_source(data: &mut CapturedData, unit: SourceUnit, edit: impl FnOnce(&mut syn::File)) {
        let source = data
            .sources
            .iter_mut()
            .find(|source| source.unit == unit)
            .unwrap();
        let mut syntax = syn::parse_file(from_utf8(&source.before).unwrap()).unwrap();
        let original = syntax.to_token_stream().to_string();
        edit(&mut syntax);
        let changed = syntax.to_token_stream().to_string();
        assert_ne!(
            changed, original,
            "fixture must change an actual source edge"
        );
        source.before = changed.into_bytes();
        source.after.clone_from(&source.before);
        source.sha256 = digest(&source.before);
    }

    fn edit_field(
        data: &mut CapturedData,
        unit: SourceUnit,
        owner: &str,
        member: &str,
        ty: syn::Type,
    ) {
        edit_source(data, unit, |source| {
            let declaration = source
                .items
                .iter_mut()
                .find_map(|entry| {
                    let syn::Item::Struct(item) = entry else {
                        return None;
                    };
                    (item.ident == owner).then_some(item)
                })
                .expect("actual source owner exists");
            let field = declaration
                .fields
                .iter_mut()
                .find(|field| {
                    field
                        .ident
                        .as_ref()
                        .map_or(member == "0", |name| name == member)
                })
                .expect("actual source field exists");
            field.ty = ty;
        });
    }

    fn edit_method(
        data: &mut CapturedData,
        unit: SourceUnit,
        owner: &str,
        name: &str,
        edit: impl FnOnce(&mut syn::ImplItemFn),
    ) {
        let source = data
            .sources
            .iter_mut()
            .find(|source| source.unit == unit)
            .unwrap();
        let mut syntax = syn::parse_file(from_utf8(&source.before).unwrap()).unwrap();
        let method = syntax
            .items
            .iter_mut()
            .find_map(|entry| {
                let syn::Item::Impl(item) = entry else {
                    return None;
                };
                if item.self_ty.to_token_stream().to_string() != owner {
                    return None;
                }
                item.items.iter_mut().find_map(|member| {
                    let syn::ImplItem::Fn(method) = member else {
                        return None;
                    };
                    (method.sig.ident == name).then_some(method)
                })
            })
            .expect("exact fixture impl/method exists");
        edit(method);
        source.before = syntax.to_token_stream().to_string().into_bytes();
        source.after.clone_from(&source.before);
        source.sha256 = digest(&source.before);
    }

    fn edit_id_type(data: &mut CapturedData, name: &str, edit: impl FnOnce(&mut syn::ItemStruct)) {
        let source = data
            .sources
            .iter_mut()
            .find(|source| source.unit == SourceUnit::Ids)
            .unwrap();
        let mut syntax = syn::parse_file(from_utf8(&source.before).unwrap()).unwrap();
        let declaration = syntax
            .items
            .iter_mut()
            .find_map(|entry| {
                let syn::Item::Struct(item) = entry else {
                    return None;
                };
                (item.ident == name).then_some(item)
            })
            .expect("actual vendor ID declaration exists");
        edit(declaration);
        source.before = syntax.to_token_stream().to_string().into_bytes();
        source.after.clone_from(&source.before);
        source.sha256 = digest(&source.before);
    }

    #[test]
    fn nfa_access_matches_pristine_recipes_and_literal_counts() {
        let (data, expected, premises) = capture();
        assert_owner_edges(&data);
        assert_index_edges(&data);
        assert_body(
            &data,
            SourceUnit::Nfa,
            "NFA",
            &syn::parse_quote! {
                pub fn state(&self, id: StateID) -> &State { &self.states()[id] }
            },
        );
        assert_body(
            &data,
            SourceUnit::Nfa,
            "NFA",
            &syn::parse_quote! {
                pub fn states(&self) -> &[State] { &self.0.states }
            },
        );
        assert_body(
            &data,
            SourceUnit::Nfa,
            "State",
            &syn::parse_quote! {
                pub fn is_epsilon(&self) -> bool {
                    match *self {
                        State::ByteRange { .. } | State::Sparse { .. } | State::Dense { .. }
                        | State::Fail | State::Match { .. } => false,
                        State::Look { .. } | State::Union { .. } | State::BinaryUnion { .. }
                        | State::Capture { .. } => true,
                    }
                }
            },
        );
        let calls = nfa();
        let Calls::Nfa { roots, dispatches } = calls else {
            panic!("NFA fixture call population")
        };
        assert_eq!((roots, dispatches), (1, 1));
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::NfaStateAccess,
            calls,
            (&[2, 2, 1], [236, 10, 9]),
            SymbolicWrites {
                state_id_elements: 0,
                usize_elements: 0,
            },
        );
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::NfaStateAccess,
            Calls::Nfa {
                roots: 1,
                dispatches: 0,
            },
            (&[1, 1, 1], [125, 10, 9]),
            SymbolicWrites {
                state_id_elements: 0,
                usize_elements: 0,
            },
        );
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::NfaStateAccess,
            Calls::Nfa {
                roots: 2,
                dispatches: 2,
            },
            (&[4, 4, 2], [472, 20, 18]),
            SymbolicWrites {
                state_id_elements: 0,
                usize_elements: 0,
            },
        );
    }

    fn assert_sparse_recipes(data: &CapturedData) {
        assert_body(
            data,
            SourceUnit::Sparse,
            "SparseSet",
            &syn::parse_quote! {
                pub(crate) fn insert(&mut self, id: StateID) -> bool {
                    if self.contains(id) { return false; }
                    let i = self.len();
                    assert!(i < self.capacity(), "{:?} exceeds capacity of {:?} when inserting {:?}", i, self.capacity(), id,);
                    let index = StateID::new_unchecked(i);
                    self.dense[index] = id;
                    self.sparse[id] = index;
                    self.len += 1;
                    true
                }
            },
        );
        assert_body(
            data,
            SourceUnit::Sparse,
            "SparseSet",
            &syn::parse_quote! {
                pub(crate) fn contains(&self, id: StateID) -> bool {
                    let index = self.sparse[id];
                    index.as_usize() < self.len() && self.dense[index] == id
                }
            },
        );
        assert_body(
            data,
            SourceUnit::Sparse,
            "SparseSet",
            &syn::parse_quote! {
                pub(crate) fn len(&self) -> usize { self.len }
            },
        );
        assert_body(
            data,
            SourceUnit::Sparse,
            "SparseSet",
            &syn::parse_quote! {
                pub(crate) fn capacity(&self) -> usize { self.dense.len() }
            },
        );
    }

    #[test]
    fn sparse_insert_matches_pristine_recipes_and_branch_counts() {
        let (data, expected, mut premises) = capture();
        assert_owner_edges(&data);
        assert_index_edges(&data);
        assert_sparse_recipes(&data);
        let Calls::Sparse {
            attempts,
            successes,
            rhs,
        } = sparse()
        else {
            panic!("sparse fixture call population")
        };
        assert_eq!((attempts, successes, rhs), (2, 1, 1));
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::SparseSetInsert,
            Calls::Sparse {
                attempts: 1,
                successes: 1,
                rhs: 0,
            },
            (&[1, 1, 2, 1, 1, 1, 2, 4], [334, 4, 3]),
            SymbolicWrites {
                state_id_elements: 2,
                usize_elements: 1,
            },
        );
        premises.entry_len = 1;
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::SparseSetInsert,
            Calls::Sparse {
                attempts: 1,
                successes: 1,
                rhs: 1,
            },
            (&[1, 1, 2, 1, 1, 2, 2, 5], [435, 4, 4]),
            SymbolicWrites {
                state_id_elements: 2,
                usize_elements: 1,
            },
        );
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::SparseSetInsert,
            Calls::Sparse {
                attempts: 1,
                successes: 0,
                rhs: 1,
            },
            (&[1, 1, 1, 0, 0, 2, 0, 3], [215, 3, 2]),
            SymbolicWrites {
                state_id_elements: 0,
                usize_elements: 0,
            },
        );
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::SparseSetInsert,
            sparse(),
            (&[2, 2, 3, 1, 1, 3, 2, 7], [549, 7, 5]),
            SymbolicWrites {
                state_id_elements: 2,
                usize_elements: 1,
            },
        );
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::SparseSetInsert,
            Calls::Sparse {
                attempts: 4,
                successes: 2,
                rhs: 3,
            },
            (&[4, 4, 6, 2, 2, 7, 4, 15], [1199, 14, 11]),
            SymbolicWrites {
                state_id_elements: 4,
                usize_elements: 2,
            },
        );
    }

    #[test]
    fn zero_invocations_keep_required_helper_slots_and_premises() {
        let (data, expected, mut premises) = capture();
        premises.epochs = 0;
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::NfaStateAccess,
            Calls::Nfa {
                roots: 0,
                dispatches: 0,
            },
            (&[0, 0, 0], [0, 0, 0]),
            SymbolicWrites {
                state_id_elements: 0,
                usize_elements: 0,
            },
        );
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::SparseSetInsert,
            Calls::Sparse {
                attempts: 0,
                successes: 0,
                rhs: 0,
            },
            (&[0; 8], [0, 0, 0]),
            SymbolicWrites {
                state_id_elements: 0,
                usize_elements: 0,
            },
        );
    }

    #[test]
    fn helper_receipts_require_exact_concrete_receivers() {
        let (data, expected, premises) = capture();
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::NfaStateAccess,
            nfa(),
            (&[2, 2, 1], [236, 10, 9]),
            SymbolicWrites {
                state_id_elements: 0,
                usize_elements: 0,
            },
        );
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::SparseSetInsert,
            sparse(),
            (&[2, 2, 3, 1, 1, 3, 2, 7], [549, 7, 5]),
            SymbolicWrites {
                state_id_elements: 2,
                usize_elements: 1,
            },
        );
        for (lemma, calls) in [
            (Lemma::NfaStateAccess, nfa()),
            (Lemma::SparseSetInsert, sparse()),
        ] {
            for required in helpers(lemma) {
                assert!(!required.key.owner.is_empty());
                assert!(!required.key.method.is_empty());
                assert_eq!(required.receiver, required.key.receiver);
                let index = data
                    .helpers
                    .iter()
                    .position(|binding| binding.key == required.key)
                    .unwrap();
                let mut changed_capture_1 = data.clone();
                changed_capture_1.helpers.remove(index);
                assert_eq!(
                    source_contract(&changed_capture_1, &expected, lemma, calls, &premises),
                    Err(Refusal::MissingHelper(required.key.clone()))
                );
                let mut changed_capture_2 = data.clone();
                changed_capture_2.helpers.push(required.clone());
                assert_eq!(
                    source_contract(&changed_capture_2, &expected, lemma, calls, &premises),
                    Err(Refusal::DuplicateHelper(required.key.clone()))
                );
                let mut changed_capture_3 = data.clone();
                changed_capture_3.helpers[index].receiver = "&Vec<u8>".to_owned();
                assert_eq!(
                    source_contract(&changed_capture_3, &expected, lemma, calls, &premises),
                    Err(Refusal::ReceiverMismatch(required.key))
                );
            }
        }
    }

    #[test]
    fn missing_duplicate_or_changed_source_receipts_are_refused() {
        let (data, expected, premises) = capture();
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::SparseSetInsert,
            sparse(),
            (&[2, 2, 3, 1, 1, 3, 2, 7], [549, 7, 5]),
            SymbolicWrites {
                state_id_elements: 2,
                usize_elements: 1,
            },
        );
        for (index, source) in data.sources.iter().enumerate() {
            assert_eq!(source.sha256, digest(&source.before));
            assert_eq!(source.before, source.after);
            let mut changed_capture_4 = data.clone();
            changed_capture_4.sources.remove(index);
            assert_eq!(
                source_contract(
                    &changed_capture_4,
                    &expected,
                    Lemma::SparseSetInsert,
                    sparse(),
                    &premises
                ),
                Err(Refusal::MissingSource(source.unit))
            );
            let mut changed_capture_5 = data.clone();
            changed_capture_5.sources.push(source.clone());
            assert_eq!(
                source_contract(
                    &changed_capture_5,
                    &expected,
                    Lemma::SparseSetInsert,
                    sparse(),
                    &premises
                ),
                Err(Refusal::DuplicateSource(source.unit))
            );
            let mut changed_capture_6 = data.clone();
            changed_capture_6.sources[index].after.push(b' ');
            assert_eq!(
                source_contract(
                    &changed_capture_6,
                    &expected,
                    Lemma::SparseSetInsert,
                    sparse(),
                    &premises
                ),
                Err(Refusal::SourceChanged(source.unit))
            );
        }
    }

    #[test]
    fn compiler_source_identity_mismatch_is_refused() {
        let (data, expected, premises) = capture();
        assert!(data.workspace.is_absolute());
        assert!(!expected.verbose.is_empty());
        let supported_commit = format!("commit-hash: {EQUALITY_COMPILER_COMMIT}");
        assert!(
            expected
                .verbose
                .lines()
                .any(|line| line == supported_commit)
        );
        assert_eq!(
            expected
                .executable
                .file_name()
                .and_then(|name| name.to_str()),
            Some("rustc")
        );
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::NfaStateAccess,
            nfa(),
            (&[2, 2, 1], [236, 10, 9]),
            SymbolicWrites {
                state_id_elements: 0,
                usize_elements: 0,
            },
        );
        let mut changed_compiler_7 = expected.clone();
        changed_compiler_7
            .verbose
            .push_str("\nunmatched compiler\n");
        assert_eq!(
            source_contract(
                &data,
                &changed_compiler_7,
                Lemma::NfaStateAccess,
                nfa(),
                &premises
            ),
            Err(Refusal::CompilerMismatch)
        );
        // Matching caller/capture strings must not approve an unsupported
        // compiler revision for the builtin-derive manual lemma.
        let mut changed_capture_8 = data.clone();
        changed_capture_8.compiler.verbose = changed_capture_8.compiler.verbose.replace(
            &supported_commit,
            "commit-hash: 0000000000000000000000000000000000000000",
        );
        let matching_but_unsupported = changed_capture_8.compiler.clone();
        assert_eq!(
            source_contract(
                &changed_capture_8,
                &matching_but_unsupported,
                Lemma::SparseSetInsert,
                sparse(),
                &premises
            ),
            Err(Refusal::CompilerMismatch)
        );
        let mut changed_compiler_9 = expected.clone();
        changed_compiler_9.executable = changed_compiler_9.executable.with_file_name("other-rustc");
        assert_eq!(
            source_contract(
                &data,
                &changed_compiler_9,
                Lemma::NfaStateAccess,
                nfa(),
                &premises
            ),
            Err(Refusal::CompilerMismatch)
        );
        let mut changed_capture_10 = data.clone();
        let source = changed_capture_10
            .sources
            .iter_mut()
            .find(|source| source.unit == SourceUnit::Vec)
            .unwrap();
        assert!(source.path.is_absolute());
        source.path = data
            .workspace
            .join("unmatched-toolchain/alloc/src/vec/mod.rs");
        assert_eq!(
            source_contract(
                &changed_capture_10,
                &expected,
                Lemma::NfaStateAccess,
                nfa(),
                &premises
            ),
            Err(Refusal::SourcePath(SourceUnit::Vec))
        );
    }

    fn assert_changed_declarations(
        data: &CapturedData,
        expected: &CompilerData,
        premises: &Premises,
    ) {
        let mut changed_capture_11 = data.clone();
        edit_method(
            &mut changed_capture_11,
            SourceUnit::Nfa,
            "NFA",
            "state",
            |method| {
                let body = method.block.to_token_stream().to_string();
                method.sig.inputs[1] = syn::parse_quote!(id: SmallIndex);
                assert_eq!(method.block.to_token_stream().to_string(), body);
            },
        );
        assert_eq!(
            source_contract(
                &changed_capture_11,
                expected,
                Lemma::NfaStateAccess,
                nfa(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Nfa))
        );
        // These mutate actual declarations, not HelperData.receiver labels.
        for (unit, owner, member, ty, lemma, calls) in [
            (
                SourceUnit::Nfa,
                "NFA",
                "0",
                syn::parse_quote!(Box<Inner>),
                Lemma::NfaStateAccess,
                nfa(),
            ),
            (
                SourceUnit::Nfa,
                "Inner",
                "states",
                syn::parse_quote!(Box<[State]>),
                Lemma::NfaStateAccess,
                nfa(),
            ),
            (
                SourceUnit::Sparse,
                "SparseSet",
                "len",
                syn::parse_quote!(u32),
                Lemma::SparseSetInsert,
                sparse(),
            ),
            (
                SourceUnit::Sparse,
                "SparseSet",
                "dense",
                syn::parse_quote!(Vec<u32>),
                Lemma::SparseSetInsert,
                sparse(),
            ),
            (
                SourceUnit::Sparse,
                "SparseSet",
                "sparse",
                syn::parse_quote!(Vec<u32>),
                Lemma::SparseSetInsert,
                sparse(),
            ),
            (
                SourceUnit::Vec,
                "Vec",
                "buf",
                syn::parse_quote!(UnreviewedRawVec<T, A>),
                Lemma::NfaStateAccess,
                nfa(),
            ),
        ] {
            let mut changed_capture_12 = data.clone();
            edit_field(&mut changed_capture_12, unit, owner, member, ty);
            assert_eq!(
                source_contract(&changed_capture_12, expected, lemma, calls, premises),
                Err(Refusal::UnsupportedBody(unit))
            );
        }
    }

    fn assert_changed_owner_bindings(
        data: &CapturedData,
        expected: &CompilerData,
        premises: &Premises,
    ) {
        let mut changed_capture_13 = data.clone();
        edit_source(&mut changed_capture_13, SourceUnit::Nfa, |source| {
            let import = source
                .items
                .iter_mut()
                .find_map(|entry| {
                    let syn::Item::Use(item) = entry else {
                        return None;
                    };
                    matches!(&item.tree, syn::UseTree::Path(path) if path.ident == "alloc")
                        .then_some(item)
                })
                .expect("actual allocation import exists");
            import.tree = syn::parse_quote!(alloc::{boxed::Box, format, string::String, rc::Rc as Arc, vec, vec::Vec});
        });
        assert_eq!(
            source_contract(
                &changed_capture_13,
                expected,
                Lemma::NfaStateAccess,
                nfa(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Nfa))
        );
        let mut changed_capture_14 = data.clone();
        edit_source(&mut changed_capture_14, SourceUnit::Nfa, |source| {
            let owner = source
                .items
                .iter_mut()
                .find_map(|entry| {
                    let syn::Item::Impl(item) = entry else {
                        return None;
                    };
                    (item.self_ty.to_token_stream().to_string() == "NFA").then_some(item)
                })
                .expect("actual NFA impl exists");
            owner.trait_ = Some((syn::parse_quote!(UnreviewedTrait), syn::parse_quote!(for)));
        });
        assert_eq!(
            source_contract(
                &changed_capture_14,
                expected,
                Lemma::NfaStateAccess,
                nfa(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Nfa))
        );
    }

    fn assert_changed_index_invocation(
        data: &CapturedData,
        expected: &CompilerData,
        premises: &Premises,
    ) {
        let mut changed_capture_15 = data.clone();
        edit_source(&mut changed_capture_15, SourceUnit::Ids, |source| {
            let invocation = source
                .items
                .iter_mut()
                .find_map(|entry| {
                    let syn::Item::Macro(item) = entry else {
                        return None;
                    };
                    (item.mac.path.is_ident("index_type_impls")
                        && item.mac.tokens.to_string()
                            == quote::quote!(StateID, StateIDError, StateIDIter, WithStateIDIter)
                                .to_string())
                    .then_some(item)
                })
                .expect("actual StateID macro invocation exists");
            invocation.mac.tokens =
                quote::quote!(OtherStateID, StateIDError, StateIDIter, WithStateIDIter);
        });
        assert_eq!(
            source_contract(
                &changed_capture_15,
                expected,
                Lemma::SparseSetInsert,
                sparse(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Ids))
        );
    }

    fn assert_changed_index_receiver(
        data: &CapturedData,
        expected: &CompilerData,
        premises: &Premises,
    ) {
        let mut changed_capture_16 = data.clone();
        edit_source(&mut changed_capture_16, SourceUnit::Ids, |source| {
            use proc_macro2::{Delimiter, Group, Ident, TokenTree};
            let definition = source
                .items
                .iter_mut()
                .find_map(|entry| {
                    let syn::Item::Macro(item) = entry else {
                        return None;
                    };
                    (item
                        .ident
                        .as_ref()
                        .is_some_and(|name| name == "index_type_impls"))
                    .then_some(item)
                })
                .expect("actual index macro definition exists");
            let mut arm: Vec<_> = definition.mac.tokens.clone().into_iter().collect();
            let body = arm
                .iter_mut()
                .find_map(|token| {
                    let TokenTree::Group(group) = token else {
                        return None;
                    };
                    (group.delimiter() == Delimiter::Brace).then_some(group)
                })
                .unwrap();
            let mut template: Vec<_> = body.stream().into_iter().collect();
            let receiver = template
                .iter_mut()
                .find(|token| {
                    matches!(token,
                TokenTree::Ident(name) if name == "Vec")
                })
                .expect("actual Vec index impl receiver exists");
            *receiver = TokenTree::Ident(Ident::new("UnreviewedVec", receiver.span()));
            let mut replacement = Group::new(Delimiter::Brace, template.into_iter().collect());
            replacement.set_span(body.span());
            *body = replacement;
            definition.mac.tokens = arm.into_iter().collect();
        });
        assert_eq!(
            source_contract(
                &changed_capture_16,
                expected,
                Lemma::SparseSetInsert,
                sparse(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Ids))
        );
    }

    fn assert_changed_helper_bodies(
        data: &CapturedData,
        expected: &CompilerData,
        premises: &Premises,
    ) {
        let mut changed_capture_17 = data.clone();
        edit_method(
            &mut changed_capture_17,
            SourceUnit::Nfa,
            "NFA",
            "state",
            |method| {
                method.block = syn::parse_quote!({ &self.states()[StateID::ZERO] });
            },
        );
        assert_eq!(
            source_contract(
                &changed_capture_17,
                expected,
                Lemma::NfaStateAccess,
                nfa(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Nfa))
        );
        let mut changed_capture_18 = data.clone();
        edit_method(
            &mut changed_capture_18,
            SourceUnit::Nfa,
            "State",
            "is_epsilon",
            |method| method.block = syn::parse_quote!({ true }),
        );
        assert_eq!(
            source_contract(
                &changed_capture_18,
                expected,
                Lemma::NfaStateAccess,
                nfa(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Nfa))
        );
        let mut changed_capture_19 = data.clone();
        edit_method(
            &mut changed_capture_19,
            SourceUnit::Sparse,
            "SparseSet",
            "contains",
            |method| {
                method.block = syn::parse_quote!({
                    let index = self.sparse[id];
                    index.as_usize() < self.len() || self.dense[index] == id
                });
            },
        );
        assert_eq!(
            source_contract(
                &changed_capture_19,
                expected,
                Lemma::SparseSetInsert,
                sparse(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Sparse))
        );
        let mut changed_capture_20 = data.clone();
        edit_method(
            &mut changed_capture_20,
            SourceUnit::Sparse,
            "SparseSet",
            "contains",
            |method| {
                method.block = syn::parse_quote!({
                    let index = self.sparse[id];
                    index.as_usize() < self.len() && self.dense[index] != id
                });
            },
        );
        assert_eq!(
            source_contract(
                &changed_capture_20,
                expected,
                Lemma::SparseSetInsert,
                sparse(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Sparse))
        );
    }

    fn assert_changed_id_declarations(
        data: &CapturedData,
        expected: &CompilerData,
        premises: &Premises,
    ) {
        let mut changed_capture_21 = data.clone();
        edit_id_type(&mut changed_capture_21, "SmallIndex", |item| {
            item.attrs.push(syn::parse_quote!(#[repr(packed)]));
        });
        assert_eq!(
            source_contract(
                &changed_capture_21,
                expected,
                Lemma::SparseSetInsert,
                sparse(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Ids))
        );
        let mut changed_capture_22 = data.clone();
        edit_id_type(&mut changed_capture_22, "StateID", |item| {
            let syn::Fields::Unnamed(fields) = &mut item.fields else {
                panic!("tuple ID fixture");
            };
            fields.unnamed[0].ty = syn::parse_quote!(u64);
        });
        assert_eq!(
            source_contract(
                &changed_capture_22,
                expected,
                Lemma::SparseSetInsert,
                sparse(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Ids))
        );
        let mut changed_capture_23 = data.clone();
        edit_id_type(&mut changed_capture_23, "StateID", |item| {
            let syn::Fields::Unnamed(fields) = &mut item.fields else {
                panic!("tuple ID fixture");
            };
            let extra_field = fields.unnamed[0].clone();
            fields.unnamed.push(extra_field);
        });
        assert_eq!(
            source_contract(
                &changed_capture_23,
                expected,
                Lemma::SparseSetInsert,
                sparse(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Ids))
        );
        let mut changed_capture_24 = data.clone();
        edit_id_type(&mut changed_capture_24, "SmallIndex", |item| {
            item.attrs
                .retain(|attribute| !attribute.path().is_ident("derive"));
        });
        assert_eq!(
            source_contract(
                &changed_capture_24,
                expected,
                Lemma::SparseSetInsert,
                sparse(),
                premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Ids))
        );
    }

    #[test]
    fn known_helper_semantic_changes_are_refused() {
        let (data, expected, premises) = capture();
        assert_owner_edges(&data);
        assert_index_edges(&data);
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::NfaStateAccess,
            nfa(),
            (&[2, 2, 1], [236, 10, 9]),
            SymbolicWrites {
                state_id_elements: 0,
                usize_elements: 0,
            },
        );
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::SparseSetInsert,
            sparse(),
            (&[2, 2, 3, 1, 1, 3, 2, 7], [549, 7, 5]),
            SymbolicWrites {
                state_id_elements: 2,
                usize_elements: 1,
            },
        );
        assert_changed_declarations(&data, &expected, &premises);
        assert_changed_owner_bindings(&data, &expected, &premises);
        assert_changed_index_invocation(&data, &expected, &premises);
        assert_changed_index_receiver(&data, &expected, &premises);
        assert_changed_helper_bodies(&data, &expected, &premises);
        assert_changed_id_declarations(&data, &expected, &premises);
        assert_changed_assert_resolution(&data, &expected, &premises);
        assert_changed_const_qualifiers(&data, &expected, &premises);
    }

    fn assert_changed_assert_resolution(
        data: &CapturedData,
        expected: &CompilerData,
        premises: &Premises,
    ) {
        assert_sparse_recipes(data);
        for injected in [
            syn::parse_quote!(
                macro_rules! assert {
                ($($argument:tt)*) => {{
                    extra_work();
                    core::assert!($($argument)*);
                }};
            }
            ),
            syn::parse_quote!(
                use crate::workful_assert as assert;
            ),
            syn::parse_quote!(
                use crate::workful_macros::*;
            ),
            syn::parse_quote!(
                #[macro_use]
                mod workful_macros;
            ),
        ] {
            let mut namespace_capture = data.clone();
            edit_source(&mut namespace_capture, SourceUnit::Sparse, |file| {
                file.items.insert(0, injected);
            });
            // All four method bodies remain pristine, including the literal
            // assert! call. The new scope must still invalidate the recipe.
            assert_sparse_recipes(&namespace_capture);
            assert_eq!(
                source_contract(
                    &namespace_capture,
                    expected,
                    Lemma::SparseSetInsert,
                    sparse(),
                    premises
                ),
                Err(Refusal::UnsupportedBody(SourceUnit::Sparse))
            );
        }
    }

    fn assert_changed_const_qualifiers(
        data: &CapturedData,
        expected: &CompilerData,
        premises: &Premises,
    ) {
        use proc_macro2::TokenTree;

        for (trait_name, replacement) in [
            (
                "Deref",
                syn::parse_quote!(
                    #[stable(feature = "rust1", since = "1.0.0")]
                    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
                    impl<T, A: Allocator> ops::Deref for Vec<T, A> {
                        type Target = [T];
                        #[inline]
                        fn deref(&self) -> &[T] {
                            self.as_slice()
                        }
                    }
                ),
            ),
            (
                "Index",
                syn::Item::Verbatim(quote::quote!(
                    #[stable(feature = "rust1", since = "1.0.0")]
                    #[rustc_const_unstable(feature = "const_index", issue = "143775")]
                    const impl<T, I: SliceIndex<[T]>, A: Allocator> Index<I> for Vec<T, A> {
                        type Output = I::Output;
                        #[inline]
                        fn index(&self, index: I) -> &Self::Output {
                            Index::index(&**self, index)
                        }
                    }
                )),
            ),
        ] {
            let mut qualifier_capture = data.clone();
            edit_source(&mut qualifier_capture, SourceUnit::Vec, |file| {
                let mut matched = 0_usize;
                for item in &mut file.items {
                    let syn::Item::Verbatim(tokens) = item else {
                        continue;
                    };
                    if tokens
                        .clone()
                        .into_iter()
                        .any(|token| matches!(token, TokenTree::Ident(name) if name == trait_name))
                    {
                        *item = replacement.clone();
                        matched = matched.checked_add(1).unwrap();
                    }
                }
                assert_eq!(matched, 1, "one actual const trait impl before mutation");
            });
            assert_eq!(
                source_contract(
                    &qualifier_capture,
                    expected,
                    Lemma::SparseSetInsert,
                    sparse(),
                    premises
                ),
                Err(Refusal::UnsupportedBody(SourceUnit::Vec))
            );
        }
    }

    #[test]
    fn unknown_helper_calls_are_not_zero_work() {
        let (data, expected, premises) = capture();
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::SparseSetInsert,
            sparse(),
            (&[2, 2, 3, 1, 1, 3, 2, 7], [549, 7, 5]),
            SymbolicWrites {
                state_id_elements: 2,
                usize_elements: 1,
            },
        );
        let mut changed_capture_25 = data.clone();
        edit_method(
            &mut changed_capture_25,
            SourceUnit::Sparse,
            "SparseSet",
            "insert",
            |method| {
                method
                    .block
                    .stmts
                    .insert(0, syn::parse_quote!(unreviewed(id);));
            },
        );
        assert_eq!(
            source_contract(
                &changed_capture_25,
                &expected,
                Lemma::SparseSetInsert,
                sparse(),
                &premises
            ),
            Err(Refusal::UnknownCall {
                unit: SourceUnit::Sparse,
                callee: "unreviewed".to_owned()
            })
        );
    }

    #[test]
    fn unapproved_helper_attributes_are_refused() {
        let (data, expected, premises) = capture();
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::NfaStateAccess,
            nfa(),
            (&[2, 2, 1], [236, 10, 9]),
            SymbolicWrites {
                state_id_elements: 0,
                usize_elements: 0,
            },
        );
        let mut changed_capture_26 = data.clone();
        edit_method(
            &mut changed_capture_26,
            SourceUnit::Nfa,
            "NFA",
            "state",
            |method| {
                method
                    .attrs
                    .push(syn::parse_quote!(#[unreviewed_runtime_attribute]));
            },
        );
        assert_eq!(
            source_contract(
                &changed_capture_26,
                &expected,
                Lemma::NfaStateAccess,
                nfa(),
                &premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Nfa))
        );
        let mut changed_capture_27 = data.clone();
        edit_method(
            &mut changed_capture_27,
            SourceUnit::Nfa,
            "NFA",
            "state",
            |method| {
                method
                    .attrs
                    .push(syn::parse_quote!(#[doc = stringify!(unreviewed)]));
            },
        );
        assert_eq!(
            source_contract(
                &changed_capture_27,
                &expected,
                Lemma::NfaStateAccess,
                nfa(),
                &premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Nfa))
        );
        let mut changed_capture_28 = data.clone();
        edit_method(
            &mut changed_capture_28,
            SourceUnit::Sparse,
            "SparseSet",
            "insert",
            |method| {
                method.block.stmts[0] = syn::parse_quote!(
                    #[unreviewed_runtime_attribute]
                    if self.contains(id) {
                        return false;
                    }
                );
            },
        );
        assert_eq!(
            source_contract(
                &changed_capture_28,
                &expected,
                Lemma::SparseSetInsert,
                sparse(),
                &premises
            ),
            Err(Refusal::UnsupportedBody(SourceUnit::Sparse))
        );
    }

    #[test]
    fn sparse_invariants_require_matching_owner_and_epoch() {
        let (data, expected, premises) = capture();
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::SparseSetInsert,
            sparse(),
            (&[2, 2, 3, 1, 1, 3, 2, 7], [549, 7, 5]),
            SymbolicWrites {
                state_id_elements: 2,
                usize_elements: 1,
            },
        );
        for (index, binding) in premises.bindings.iter().enumerate() {
            assert_eq!(binding.owner, data.owner);
            assert_eq!(binding.epoch, data.epoch);
            let mut changed_premises_29 = premises.clone();
            changed_premises_29.bindings.remove(index);
            assert_eq!(
                source_contract(
                    &data,
                    &expected,
                    Lemma::SparseSetInsert,
                    sparse(),
                    &changed_premises_29
                ),
                Err(Refusal::MissingInvariant(binding.invariant))
            );
            let mut changed_premises_30 = premises.clone();
            changed_premises_30.bindings[index]
                .owner
                .push_str("::other");
            assert_eq!(
                source_contract(
                    &data,
                    &expected,
                    Lemma::SparseSetInsert,
                    sparse(),
                    &changed_premises_30
                ),
                Err(Refusal::InvariantMismatch(binding.invariant))
            );
            let mut changed_premises_31 = premises.clone();
            changed_premises_31.bindings[index].epoch = 8;
            assert_eq!(
                source_contract(
                    &data,
                    &expected,
                    Lemma::SparseSetInsert,
                    sparse(),
                    &changed_premises_31
                ),
                Err(Refusal::InvariantMismatch(binding.invariant))
            );
        }
    }

    fn assert_bad_sparse_premises(
        data: &CapturedData,
        expected: &CompilerData,
        premises: &Premises,
    ) {
        for (changed_premises_31, error) in [
            (
                Premises {
                    states: 0,
                    ..premises.clone()
                },
                Refusal::InvalidFacts("states"),
            ),
            (
                Premises {
                    epochs: 0,
                    ..premises.clone()
                },
                Refusal::InvalidFacts("epochs"),
            ),
            (
                Premises {
                    dense_len: 3,
                    ..premises.clone()
                },
                Refusal::InvalidFacts("dense_len"),
            ),
            (
                Premises {
                    sparse_len: 3,
                    ..premises.clone()
                },
                Refusal::InvalidFacts("sparse_len"),
            ),
            (
                Premises {
                    entry_len: 5,
                    ..premises.clone()
                },
                Refusal::InvalidFacts("entry_len"),
            ),
            (
                Premises {
                    max_id: 4,
                    ..premises.clone()
                },
                Refusal::InvalidFacts("max_id"),
            ),
        ] {
            assert_eq!(
                source_contract(
                    data,
                    expected,
                    Lemma::SparseSetInsert,
                    sparse(),
                    &changed_premises_31
                ),
                Err(error)
            );
        }
    }

    #[test]
    fn invalid_sparse_dimensions_and_ids_are_refused() {
        let (data, expected, premises) = capture();
        assert_eq!(
            (
                premises.states,
                premises.epochs,
                premises.dense_len,
                premises.sparse_len,
                premises.entry_len,
                premises.max_id
            ),
            (4, 1, 4, 4, 0, 3)
        );
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::SparseSetInsert,
            sparse(),
            (&[2, 2, 3, 1, 1, 3, 2, 7], [549, 7, 5]),
            SymbolicWrites {
                state_id_elements: 2,
                usize_elements: 1,
            },
        );
        assert_bad_sparse_premises(&data, &expected, &premises);
        assert_eq!(
            source_contract(
                &data,
                &expected,
                Lemma::SparseSetInsert,
                Calls::Sparse {
                    attempts: 1,
                    successes: 2,
                    rhs: 0
                },
                &premises
            ),
            Err(Refusal::InvalidFacts("successes"))
        );
        assert_eq!(
            source_contract(
                &data,
                &expected,
                Lemma::SparseSetInsert,
                Calls::Sparse {
                    attempts: 1,
                    successes: 1,
                    rhs: 2
                },
                &premises
            ),
            Err(Refusal::InvalidFacts("rhs"))
        );
    }

    #[test]
    fn primitive_count_overflow_is_refused() {
        let (data, expected, premises) = capture();
        assert_local(
            &data,
            &expected,
            &premises,
            Lemma::NfaStateAccess,
            nfa(),
            (&[2, 2, 1], [236, 10, 9]),
            SymbolicWrites {
                state_id_elements: 0,
                usize_elements: 0,
            },
        );
        assert_eq!(
            source_contract(
                &data,
                &expected,
                Lemma::NfaStateAccess,
                Calls::Nfa {
                    roots: u64::MAX,
                    dispatches: 1
                },
                &premises
            ),
            Err(Refusal::Overflow)
        );
        // Source-event totals overflow although the local helper call
        // multiplicities remain representable: root path 125, duplicate 215.
        assert_eq!(
            source_contract(
                &data,
                &expected,
                Lemma::NfaStateAccess,
                Calls::Nfa {
                    roots: u64::MAX.checked_div(125).unwrap().checked_add(1).unwrap(),
                    dispatches: 0
                },
                &premises
            ),
            Err(Refusal::Overflow)
        );
        let duplicate_premises = Premises {
            entry_len: 1,
            ..premises.clone()
        };
        assert_eq!(
            source_contract(
                &data,
                &expected,
                Lemma::SparseSetInsert,
                Calls::Sparse {
                    attempts: u64::MAX.checked_div(215).unwrap().checked_add(1).unwrap(),
                    successes: 0,
                    rhs: u64::MAX.checked_div(215).unwrap().checked_add(1).unwrap()
                },
                &duplicate_premises
            ),
            Err(Refusal::Overflow)
        );
        assert_eq!(
            source_contract(
                &data,
                &expected,
                Lemma::SparseSetInsert,
                Calls::Sparse {
                    attempts: u64::MAX.checked_div(2).unwrap().checked_add(1).unwrap(),
                    successes: 1,
                    rhs: u64::MAX.checked_div(2).unwrap()
                },
                &premises
            ),
            Err(Refusal::Overflow)
        );
    }
}
