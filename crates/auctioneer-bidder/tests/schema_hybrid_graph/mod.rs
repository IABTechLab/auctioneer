//! Complete replay certificates for one fixed DFA, not whole-meta qualification.

use regex_automata::{
    Anchored, PatternID,
    hybrid::{
        LazyStateID,
        dfa::{Cache, DFA},
    },
    util::start,
};
use std::collections::{HashSet, VecDeque};
use std::fmt;
use std::iter;

#[derive(Clone, Copy)]
pub(crate) struct GraphBudget {
    pub states: usize,
    pub steps: usize,
}

pub(crate) const QUALIFICATION_BUDGET: GraphBudget = GraphBudget {
    states: 2048,
    steps: 0x0080_0000,
};

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum GraphError {
    StateLimit,
    StepLimit,
    Cleared,
    Engine,
    Shape,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct GraphCertificate {
    pub ordinary_states: usize,
    pub expanded_states: usize,
    pub starts: usize,
    pub byte_edges: usize,
    pub eoi_edges: usize,
    pub eoi_matches: usize,
    pub match_expansions: usize,
    pub steps: usize,
    pub reported_heap_bytes: usize,
}

impl fmt::Display for GraphCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ordinary={} expanded={} starts={} byte-edges={} eoi-edges={} eoi-matches={} expanded-matches={} steps={} final-reported-bytes={}",
            self.ordinary_states,
            self.expanded_states,
            self.starts,
            self.byte_edges,
            self.eoi_edges,
            self.eoi_matches,
            self.match_expansions,
            self.steps,
            self.reported_heap_bytes
        )
    }
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::StateLimit => "state_limit",
            Self::StepLimit => "step_limit",
            Self::Cleared => "cache_cleared",
            Self::Engine => "engine_refused",
            Self::Shape => "unexpected_shape",
        })
    }
}

struct Witness {
    start: start::Config,
    bytes: Vec<u8>,
    // Comparison key only: transitions always reacquire the current handle.
    id: LazyStateID,
}

struct Explorer<'a> {
    dfa: &'a DFA,
    cache: Cache,
    budget: GraphBudget,
    report: GraphCertificate,
    discovered: HashSet<LazyStateID>,
    enqueued: HashSet<LazyStateID>,
    pending: VecDeque<Witness>,
}

impl Explorer<'_> {
    fn step(&mut self) -> Result<(), GraphError> {
        if self.report.steps >= self.budget.steps {
            return Err(GraphError::StepLimit);
        }
        self.report.steps = self.report.steps.checked_add(1).ok_or(GraphError::Shape)?;
        Ok(())
    }

    fn checked<E>(&self, result: Result<LazyStateID, E>) -> Result<LazyStateID, GraphError> {
        if self.cache.clear_count() != 0 {
            return Err(GraphError::Cleared);
        }
        let id = result.map_err(|_error| GraphError::Engine)?;
        if id.is_unknown() {
            return Err(GraphError::Shape);
        }
        Ok(id)
    }

    fn start(&mut self, config: &start::Config) -> Result<LazyStateID, GraphError> {
        self.step()?;
        let result = self.dfa.start_state(&mut self.cache, config);
        self.checked(result)
    }

    fn byte(&mut self, current: LazyStateID, byte: u8) -> Result<LazyStateID, GraphError> {
        self.step()?;
        let result = self.dfa.next_state(&mut self.cache, current, byte);
        self.checked(result)
    }

    fn eoi(&mut self, current: LazyStateID) -> Result<LazyStateID, GraphError> {
        self.step()?;
        let result = self.dfa.next_eoi_state(&mut self.cache, current);
        self.checked(result)
    }

    fn remember(&mut self, id: LazyStateID) -> Result<(), GraphError> {
        self.discovered.insert(id);
        if self.discovered.len() > self.budget.states {
            return Err(GraphError::StateLimit);
        }
        Ok(())
    }

    fn enqueue(
        &mut self,
        id: LazyStateID,
        config: &start::Config,
        prefix: &[u8],
        tail: Option<u8>,
    ) -> Result<(), GraphError> {
        self.remember(id)?;
        if self.enqueued.insert(id) {
            let mut bytes = prefix.to_vec();
            bytes.extend(tail);
            self.pending.push_back(Witness {
                start: config.clone(),
                bytes,
                id,
            });
        }
        Ok(())
    }

    fn seed(&mut self) -> Result<(), GraphError> {
        let modes = if self.dfa.get_config().get_starts_for_each_pattern() {
            self.dfa
                .pattern_len()
                .checked_add(2)
                .ok_or(GraphError::Shape)?
        } else {
            2
        };
        for mode in 0..modes {
            let anchored = match mode {
                0 => Anchored::No,
                1 => Anchored::Yes,
                _ => Anchored::Pattern(
                    PatternID::new(mode.checked_sub(2).ok_or(GraphError::Shape)?)
                        .map_err(|_error| GraphError::Shape)?,
                ),
            };
            for behind in iter::once(None).chain((0..=u8::MAX).map(Some)) {
                let config = start::Config::new().anchored(anchored).look_behind(behind);
                let id = self.start(&config)?;
                self.report.starts = self.report.starts.checked_add(1).ok_or(GraphError::Shape)?;
                self.enqueue(id, &config, &[], None)?;
            }
        }
        Ok(())
    }

    fn replay(&mut self, witness: &Witness) -> Result<LazyStateID, GraphError> {
        let mut current = self.start(&witness.start)?;
        for &byte in &witness.bytes {
            current = self.byte(current, byte)?;
        }
        if current != witness.id {
            return Err(GraphError::Shape);
        }
        Ok(current)
    }

    fn explore(mut self) -> Result<GraphCertificate, GraphError> {
        self.seed()?;
        while let Some(witness) = self.pending.pop_front() {
            self.report.expanded_states = self
                .report
                .expanded_states
                .checked_add(1)
                .ok_or(GraphError::Shape)?;
            if witness.id.is_match() {
                self.report.match_expansions = self
                    .report
                    .match_expansions
                    .checked_add(1)
                    .ok_or(GraphError::Shape)?;
            }
            for byte in 0..=u8::MAX {
                let current = self.replay(&witness)?;
                let next = self.byte(current, byte)?;
                self.report.byte_edges = self
                    .report
                    .byte_edges
                    .checked_add(1)
                    .ok_or(GraphError::Shape)?;
                self.enqueue(next, &witness.start, &witness.bytes, Some(byte))?;
            }
            let current = self.replay(&witness)?;
            let terminal = self.eoi(current)?;
            self.report.eoi_edges = self
                .report
                .eoi_edges
                .checked_add(1)
                .ok_or(GraphError::Shape)?;
            if terminal.is_match() {
                self.report.eoi_matches = self
                    .report
                    .eoi_matches
                    .checked_add(1)
                    .ok_or(GraphError::Shape)?;
            }
            // EOI is terminal. An ordinary byte witness can enqueue it later.
            self.remember(terminal)?;
        }
        self.report.ordinary_states = self
            .discovered
            .iter()
            .filter(|id| !id.is_dead() && !id.is_quit())
            .count();
        self.report.reported_heap_bytes = self.cache.memory_usage();
        Ok(self.report)
    }
}

pub(crate) fn certify(dfa: &DFA, budget: GraphBudget) -> Result<GraphCertificate, GraphError> {
    if budget.states == 0 || budget.steps == 0 {
        return Err(GraphError::Shape);
    }
    let cache = dfa.create_cache();
    if cache.clear_count() != 0 {
        return Err(GraphError::Cleared);
    }
    Explorer {
        dfa,
        cache,
        budget,
        report: GraphCertificate::default(),
        discovered: HashSet::new(),
        enqueued: HashSet::new(),
        pending: VecDeque::new(),
    }
    .explore()
}

#[cfg(test)]
mod tests {
    use super::{
        Explorer, GraphBudget, GraphCertificate, GraphError, QUALIFICATION_BUDGET, certify,
    };
    use regex_automata::hybrid::dfa::DFA;
    use regex_automata::nfa::thompson::{self, Builder, Transition};
    use regex_automata::util::look::{Look, LookMatcher};
    use regex_automata::util::start;
    use regex_automata::{Anchored, PatternID};
    use std::collections::{HashSet, VecDeque};
    use std::mem::size_of;

    fn new_explorer(dfa: &DFA) -> Explorer<'_> {
        Explorer {
            dfa,
            cache: dfa.create_cache(),
            budget: QUALIFICATION_BUDGET,
            report: GraphCertificate::default(),
            discovered: HashSet::new(),
            enqueued: HashSet::new(),
            pending: VecDeque::new(),
        }
    }

    #[test]
    fn seeded_roots_cover_distinct_word_line_and_custom_terminator_contexts() {
        let mut look = LookMatcher::new();
        look.set_line_terminator(b'~');
        let dfa = DFA::builder()
            .configure(DFA::config().starts_for_each_pattern(true))
            .thompson(thompson::Config::new().look_matcher(look))
            .build(r"(?m:^)|(?mR:^)|(?-u:\b)")
            .unwrap();
        let mut explorer = new_explorer(&dfa);
        explorer.seed().unwrap();
        let seeded = explorer.enqueued.clone();
        for anchored in [
            Anchored::No,
            Anchored::Yes,
            Anchored::Pattern(PatternID::ZERO),
        ] {
            for behind in [
                None,
                Some(b'a'),
                Some(b'!'),
                Some(b'\n'),
                Some(b'\r'),
                Some(b'~'),
            ] {
                let config = start::Config::new().anchored(anchored).look_behind(behind);
                let id = explorer.start(&config).unwrap();
                assert!(seeded.contains(&id));
            }
        }
        assert_eq!(explorer.report.starts, 771);
    }

    #[test]
    fn terminal_only_encodings_are_included_in_the_population_bound() {
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
        let nfa = builder.build(byte, byte).unwrap();
        let dfa = DFA::builder().build_from_nfa(nfa).unwrap();
        let certificate = certify(&dfa, QUALIFICATION_BUDGET).unwrap();
        // Literal byte, pending End, and EOI-only match; dead is not ordinary.
        assert_eq!(certificate.ordinary_states, 3);
        assert_eq!(certificate.match_expansions, 0);
        assert!(certificate.eoi_matches > 0);
    }

    #[test]
    fn terminal_first_discovery_does_not_suppress_later_byte_expansion() {
        let dfa = DFA::new("^a").unwrap();
        let mut explorer = new_explorer(&dfa);
        let config = start::Config::new();
        let initial = explorer.start(&config).unwrap();
        let after_a = explorer.byte(initial, b'a').unwrap();
        let terminal = explorer.eoi(after_a).unwrap();
        assert!(terminal.is_match());
        explorer.remember(terminal).unwrap();
        assert!(!explorer.enqueued.contains(&terminal));

        let restarted = explorer.start(&config).unwrap();
        let repeated_a = explorer.byte(restarted, b'a').unwrap();
        let after_b = explorer.byte(repeated_a, b'b').unwrap();
        assert_eq!(after_b, terminal);
        explorer
            .enqueue(after_b, &config, b"a", Some(b'b'))
            .unwrap();
        let witness = explorer.pending.pop_front().unwrap();
        assert_eq!(witness.id, terminal);
        assert_eq!(witness.bytes, b"ab");
        assert_eq!(explorer.replay(&witness), Ok(terminal));
        assert!(explorer.pending.is_empty());
    }

    #[test]
    fn never_match_certificate_covers_every_start_context_byte_and_terminal_edge() {
        // add_nfa_states retains Fail: starts share one ordinary state; every
        // edge reaches dead. Empty/one-byte witnesses cost two/three API calls.
        assert_eq!(
            certify(&DFA::never_match().unwrap(), QUALIFICATION_BUDGET),
            Ok(GraphCertificate {
                ordinary_states: 1,
                expanded_states: 2,
                starts: 514,
                byte_edges: 512,
                eoi_edges: 2,
                eoi_matches: 0,
                match_expansions: 0,
                steps: 1799,
                reported_heap_bytes: if size_of::<usize>() == 8 { 255 } else { 207 },
            })
        );
    }

    #[test]
    fn certificate_seeds_only_configured_per_pattern_modes_without_losing_shared_modes() {
        let mut builder = Builder::new();
        let mut roots = Vec::new();
        for byte in *b"ab" {
            builder.start_pattern().unwrap();
            let matched = builder.add_match().unwrap();
            let root = builder
                .add_range(Transition {
                    start: byte,
                    end: byte,
                    next: matched,
                })
                .unwrap();
            builder.finish_pattern(root).unwrap();
            roots.push(root);
        }
        let shared = builder.add_union(roots).unwrap();
        let nfa = builder.build(shared, shared).unwrap();
        for (per_pattern, expected_starts, expected_population) in
            [(false, 514, 5), (true, 1028, 7)]
        {
            let dfa = DFA::builder()
                .configure(DFA::config().starts_for_each_pattern(per_pattern))
                .build_from_nfa(nfa.clone())
                .unwrap();
            let certificate = certify(&dfa, QUALIFICATION_BUDGET).unwrap();
            assert_eq!(certificate.starts, expected_starts);
            // Shared root, two pending/delayed matches; per-pattern adds two roots.
            assert_eq!(certificate.ordinary_states, expected_population);
        }
    }

    #[test]
    fn certificate_counts_terminal_only_matches_and_expands_byte_reached_matches() {
        let terminal = certify(&DFA::new("a$").unwrap(), QUALIFICATION_BUDGET).unwrap();
        assert!(terminal.eoi_matches > 0);
        let continuing = certify(&DFA::new("a").unwrap(), QUALIFICATION_BUDGET).unwrap();
        assert!(continuing.match_expansions > 0);
    }

    #[test]
    fn replay_steps_and_discovered_state_budgets_refuse_independently() {
        let never = DFA::never_match().unwrap();
        assert_eq!(
            certify(
                &never,
                GraphBudget {
                    states: 2,
                    steps: 1798
                }
            ),
            Err(GraphError::StepLimit)
        );
        assert_eq!(
            certify(
                &never,
                GraphBudget {
                    states: 2,
                    steps: 1799
                }
            )
            .unwrap()
            .steps,
            1799
        );
        assert_eq!(
            certify(
                &DFA::new("a").unwrap(),
                GraphBudget {
                    states: 1,
                    ..QUALIFICATION_BUDGET
                }
            ),
            Err(GraphError::StateLimit)
        );
    }

    #[test]
    fn incomplete_cache_epochs_never_produce_partial_population_certificates() {
        let dfa = DFA::builder()
            .configure(DFA::config().cache_capacity(0x0001_0000))
            .build("[01]*1[01]{16}$")
            .unwrap();
        assert_eq!(
            certify(&dfa, QUALIFICATION_BUDGET),
            Err(GraphError::Cleared)
        );
    }

    #[test]
    fn rejected_start_contexts_are_not_silently_skipped() {
        let dfa = DFA::builder()
            .configure(DFA::config().quit(0, true))
            .build("a")
            .unwrap();
        assert_eq!(certify(&dfa, QUALIFICATION_BUDGET), Err(GraphError::Engine));
    }

    #[test]
    fn zero_exploration_budgets_refuse_instead_of_claiming_an_empty_graph() {
        for budget in [
            GraphBudget {
                states: 0,
                ..QUALIFICATION_BUDGET
            },
            GraphBudget {
                steps: 0,
                ..QUALIFICATION_BUDGET
            },
        ] {
            assert_eq!(
                certify(&DFA::never_match().unwrap(), budget),
                Err(GraphError::Shape)
            );
        }
    }
}
