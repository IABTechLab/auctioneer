//! Sufficient reported-capacity proof, not physical memory or meta fallback.

use crate::schema_cache_bounds::BoundsError;
use crate::schema_hybrid_bounds::{HybridDimensions, QuitAdmissionError, require_no_quit};
use crate::schema_hybrid_graph::{GraphCertificate, GraphError, QUALIFICATION_BUDGET, certify};
use crate::schema_hybrid_heap::HybridHeap;
use regex_automata::hybrid::dfa::DFA;
use std::fmt;
use std::mem::size_of;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CapacityError {
    Projection(BoundsError),
    RecoveryUnavailable,
    Exploration(GraphError),
    Quit(QuitAdmissionError),
    IdSpace,
    Insufficient { required: usize, configured: usize },
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ReportedCapacityProof {
    pub required: usize,
    pub configured: usize,
}

impl From<BoundsError> for CapacityError {
    fn from(error: BoundsError) -> Self {
        Self::Projection(error)
    }
}

impl From<GraphError> for CapacityError {
    fn from(error: GraphError) -> Self {
        Self::Exploration(error)
    }
}

impl fmt::Display for CapacityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projection(BoundsError::Arithmetic) => f.write_str("arithmetic_overflow"),
            Self::Projection(BoundsError::Compilation) => f.write_str("compilation_refused"),
            Self::Projection(BoundsError::Shape) => f.write_str("unexpected_shape"),
            Self::RecoveryUnavailable => f.write_str("recovery_unavailable"),
            Self::Exploration(reason) => write!(f, "graph_{reason}"),
            Self::Quit(reason) => write!(f, "quit_{reason}"),
            Self::IdSpace => f.write_str("raw_id_space"),
            Self::Insufficient {
                required,
                configured,
            } => write!(
                f,
                "insufficient_capacity required={required} configured={configured}"
            ),
        }
    }
}

impl fmt::Display for ReportedCapacityProof {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "required-reported-bytes={} configured-reported-bytes={}",
            self.required, self.configured
        )
    }
}

impl ReportedCapacityProof {
    pub(crate) fn from_non_quitting_dfa(dfa: &DFA) -> Result<Self, CapacityError> {
        require_no_quit(dfa).map_err(CapacityError::Quit)?;
        Self::from_dfa(dfa)
    }

    pub(crate) fn from_dfa(dfa: &DFA) -> Result<Self, CapacityError> {
        // Establish the independent checked envelope before the engine's
        // unchecked report or graph traversal. No observed report funds itself.
        HybridHeap::from_dfa(dfa)?.ok_or(CapacityError::RecoveryUnavailable)?;
        let storage = HybridDimensions::from_dfa(dfa)?;
        let graph = certify(dfa, QUALIFICATION_BUDGET)?;
        Self::project(
            &storage,
            &graph,
            dfa.get_config().get_cache_capacity(),
            size_of::<usize>(),
        )
    }

    fn project(
        storage: &HybridDimensions,
        graph: &GraphCertificate,
        configured: usize,
        word: usize,
    ) -> Result<Self, CapacityError> {
        let arithmetic = || CapacityError::Projection(BoundsError::Arithmetic);
        let stride = storage
            .sentinel_transitions
            .checked_div(3)
            .ok_or_else(arithmetic)?;
        let ids = graph
            .ordinary_states
            .checked_add(3)
            .and_then(|rows| rows.checked_mul(stride))
            .ok_or_else(arithmetic)?;
        if ids >= 0x07ff_ffff {
            return Err(CapacityError::IdSpace);
        }
        let stack = storage
            .epsilon_stack
            .checked_mul(2)
            .and_then(|entries| entries.max(4).checked_mul(4))
            .ok_or_else(arithmetic)?;
        let scratch = storage
            .state_payload_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(stack))
            .ok_or_else(arithmetic)?;
        let prospective_row = stride
            .checked_add(word)
            .and_then(|entries| entries.checked_mul(4))
            .and_then(|bytes| bytes.checked_add(4))
            .and_then(|bytes| bytes.checked_add(storage.state_payload_bytes))
            .ok_or_else(arithmetic)?;
        // The complete report already includes observed scratch. Adding its
        // independent maximum again is deliberately sufficient, not necessary.
        let required = graph
            .reported_heap_bytes
            .checked_add(scratch)
            .and_then(|bytes| bytes.checked_add(prospective_row))
            .ok_or_else(arithmetic)?;
        if required > configured {
            return Err(CapacityError::Insufficient {
                required,
                configured,
            });
        }
        Ok(Self {
            required,
            configured,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BoundsError, CapacityError, GraphCertificate, GraphError, HybridDimensions,
        QuitAdmissionError, ReportedCapacityProof,
    };
    use crate::schema_cache_bounds::{NfaDimensions, forward_nfa};
    use crate::schema_hybrid_bounds::{HybridDirection, candidate_hybrid};
    use crate::schema_hybrid_population::{HybridPopulation, recovery_ceiling};
    use regex_automata::hybrid::dfa::DFA;
    use regex_automata::nfa::thompson::{Builder, NFA, Transition};
    use regex_automata::util::look::Look;
    use std::mem::size_of;

    fn storage() -> HybridDimensions {
        HybridDimensions {
            sparse_entries: 12,
            start_entries: 18,
            sentinel_transitions: 12,
            epsilon_stack: 1,
            state_payload_bytes: 32,
        }
    }

    #[test]
    fn non_quitting_capacity_constructor_rejects_each_quit_source_before_graphs() {
        let unicode =
            candidate_hybrid(&forward_nfa(r"\bword").unwrap(), HybridDirection::Forward).unwrap();
        let explicit = DFA::builder()
            .configure(DFA::config().quit(b'x', true))
            .build("a+")
            .unwrap();
        for (dfa, reason) in [
            (unicode, QuitAdmissionError::UnicodeWordAssertion),
            (explicit, QuitAdmissionError::ExplicitQuitByte),
        ] {
            assert_eq!(
                ReportedCapacityProof::from_non_quitting_dfa(&dfa),
                Err(CapacityError::Quit(reason))
            );
        }
        let overflow = DFA::builder()
            .configure(
                DFA::config()
                    .quit(b'x', true)
                    .cache_capacity(usize::MAX.checked_div(2).unwrap()),
            )
            .build_from_nfa(NFA::never_match())
            .unwrap();
        assert_eq!(
            ReportedCapacityProof::from_dfa(&overflow),
            Err(CapacityError::Projection(BoundsError::Arithmetic))
        );
        assert_eq!(
            ReportedCapacityProof::from_non_quitting_dfa(&overflow),
            Err(CapacityError::Quit(QuitAdmissionError::ExplicitQuitByte))
        );
    }

    fn graph(reported: usize, ordinary: usize) -> GraphCertificate {
        GraphCertificate {
            ordinary_states: ordinary,
            reported_heap_bytes: reported,
            ..GraphCertificate::default()
        }
    }

    #[test]
    fn reported_requirement_varies_each_independent_storage_owner() {
        // Arithmetic inputs, not standalone completed graph certificates.
        for (reported, word, stride, branches, payload, required) in [
            (100_usize, 8, 4_usize, 1, 32, 264),
            (101, 8, 4, 1, 32, 265),
            (100, 4, 4, 1, 32, 248),
            (100, 8, 8, 1, 32, 280),
            (100, 8, 4, 10, 32, 328),
            (100, 8, 4, 1, 64, 360),
        ] {
            let mut dimensions = storage();
            dimensions.sentinel_transitions = stride.checked_mul(3).unwrap();
            dimensions.epsilon_stack = branches;
            dimensions.state_payload_bytes = payload;
            assert_eq!(
                ReportedCapacityProof::project(&dimensions, &graph(reported, 4), 1024, word),
                Ok(ReportedCapacityProof {
                    required,
                    configured: 1024
                })
            );
        }
    }

    #[test]
    fn reported_requirement_accepts_exact_capacity_and_refuses_one_byte_below() {
        for configured in [263, 264, 265] {
            let expected = if configured < 264 {
                Err(CapacityError::Insufficient {
                    required: 264,
                    configured,
                })
            } else {
                Ok(ReportedCapacityProof {
                    required: 264,
                    configured,
                })
            };
            assert_eq!(
                ReportedCapacityProof::project(&storage(), &graph(100, 4), configured, 8),
                expected
            );
        }
    }

    #[test]
    fn raw_id_space_is_independent_of_otherwise_fitting_reported_capacity() {
        let mut dimensions = storage();
        dimensions.sentinel_transitions = 24;
        assert_eq!(
            ReportedCapacityProof::project(&dimensions, &graph(100, 0x00FF_FFFC), 1024, 8),
            Ok(ReportedCapacityProof {
                required: 280,
                configured: 1024
            })
        );
        assert_eq!(
            ReportedCapacityProof::project(&dimensions, &graph(100, 0x00FF_FFFD), 1024, 8),
            Err(CapacityError::IdSpace)
        );
    }

    #[test]
    fn reported_requirement_checks_all_sums_products_and_id_arithmetic() {
        let arithmetic = Err(CapacityError::Projection(BoundsError::Arithmetic));
        assert_eq!(
            ReportedCapacityProof::project(&storage(), &graph(usize::MAX, 4), 1024, 8),
            arithmetic
        );
        assert_eq!(
            ReportedCapacityProof::project(&storage(), &graph(100, usize::MAX), 1024, 8),
            arithmetic
        );
        let mut dimensions = storage();
        dimensions.epsilon_stack = usize::MAX;
        assert_eq!(
            ReportedCapacityProof::project(&dimensions, &graph(100, 4), 1024, 8),
            arithmetic
        );
        dimensions = storage();
        dimensions.state_payload_bytes = usize::MAX;
        assert_eq!(
            ReportedCapacityProof::project(&dimensions, &graph(100, 4), 1024, 8),
            arithmetic
        );
    }

    #[test]
    fn never_match_capacity_proof_uses_its_actual_report_and_configured_limit() {
        let required = if size_of::<usize>() == 8 {
            369_usize
        } else {
            305
        };
        for configured in [required.checked_sub(1).unwrap(), required] {
            let dfa = DFA::builder()
                .configure(DFA::config().cache_capacity(configured))
                .build_from_nfa(NFA::never_match())
                .unwrap();
            let expected = if configured < required {
                Err(CapacityError::Insufficient {
                    required,
                    configured,
                })
            } else {
                Ok(ReportedCapacityProof {
                    required,
                    configured,
                })
            };
            assert_eq!(ReportedCapacityProof::from_dfa(&dfa), expected);
        }
    }

    #[test]
    fn terminal_match_capacity_proof_keeps_a_distinct_real_report_and_nfa() {
        let mut builder = Builder::new();
        builder.start_pattern().unwrap();
        let matched = builder.add_match().unwrap();
        let end = builder.add_look(matched, Look::End).unwrap();
        let byte = builder
            .add_range(Transition {
                start: b'a',
                end: b'a',
                next: end,
            })
            .unwrap();
        builder.finish_pattern(byte).unwrap();
        let dfa = DFA::builder()
            .build_from_nfa(builder.build(byte, byte).unwrap())
            .unwrap();
        assert_eq!(
            ReportedCapacityProof::from_dfa(&dfa),
            Ok(ReportedCapacityProof {
                required: if size_of::<usize>() == 8 { 622 } else { 526 },
                configured: dfa.get_config().get_cache_capacity(),
            })
        );
    }

    #[test]
    fn capacity_proof_refuses_unavailable_recovery_before_exploring() {
        let nfa = forward_nfa("[01]*1[01]{16}$").unwrap();
        let base = candidate_hybrid(&nfa, HybridDirection::Forward).unwrap();
        let required = recovery_ceiling(
            NfaDimensions::from_nfa(&nfa).unwrap(),
            &HybridDimensions::from_dfa(&base).unwrap(),
            size_of::<usize>(),
        )
        .unwrap();
        let dfa = DFA::builder()
            .configure(
                base.get_config()
                    .clone()
                    .cache_capacity(required.checked_sub(1).unwrap()),
            )
            .build_from_nfa(nfa)
            .unwrap();
        assert_eq!(
            ReportedCapacityProof::from_dfa(&dfa),
            Err(CapacityError::RecoveryUnavailable)
        );
    }

    #[test]
    fn capacity_proof_checks_coarse_heap_arithmetic_before_observing_a_small_graph() {
        let dfa = DFA::builder()
            .configure(DFA::config().cache_capacity(usize::MAX.checked_div(2).unwrap()))
            .build_from_nfa(NFA::never_match())
            .unwrap();
        assert!(HybridPopulation::from_dfa(&dfa).unwrap().is_some());
        assert_eq!(
            ReportedCapacityProof::from_dfa(&dfa),
            Err(CapacityError::Projection(BoundsError::Arithmetic))
        );
    }

    #[test]
    fn capacity_proof_never_promotes_a_failed_graph_to_a_small_success() {
        let dfa = candidate_hybrid(
            &forward_nfa("[01]*1[01]{16}$").unwrap(),
            HybridDirection::Forward,
        )
        .unwrap();
        assert_eq!(
            ReportedCapacityProof::from_dfa(&dfa),
            Err(CapacityError::Exploration(GraphError::Cleared))
        );
    }
}
