#[cfg(test)]
mod tests {
    use crate::schema_backtrack_bounds::BacktrackDimensions;
    use crate::schema_cache_bounds::reverse_nfa;
    use crate::schema_cache_bounds::{NfaDimensions, emitted_patterns, forward_nfa};
    use crate::schema_hybrid_bounds::{HybridDimensions, HybridDirection, candidate_hybrid};
    use crate::schema_hybrid_capacity::ReportedCapacityProof;
    use crate::schema_hybrid_graph::{QUALIFICATION_BUDGET, certify};
    use crate::schema_hybrid_heap::HybridHeap;
    use crate::schema_hybrid_population::HybridPopulation;
    use crate::schema_hybrid_prefix::{OptionalPrefixEnvelope, PrefixEnvelope, for_meta_pattern};
    use crate::schema_onepass_bounds::{candidate_onepass, explicit_scratch_slots};
    use crate::schema_profile;
    use crate::schema_qualification::Draft;
    use regex_automata::nfa::thompson::NFA;
    use regex_automata::util::primitives::NonMaxUsize;
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fmt::Write as _;
    use std::fs;
    use std::io::{Write as _, stderr};
    use std::mem::size_of;
    use std::path::Path;
    use tempfile::tempdir;

    fn expand_schema(path: &Path, draft: &Draft) -> syn::File {
        let draft_name = match draft {
            Draft::Draft4 => "Draft4",
            Draft::Draft202012 => "Draft202012",
            Draft::Unspecified | Draft::Other(_) => panic!("unqualified source draft"),
        };
        let path_literal = syn::LitStr::new(path.to_str().unwrap(), proc_macro2::Span::call_site())
            .token()
            .to_string();
        let options = format!(
            "path = {path_literal}, draft = {draft_name}, validate_formats = true, ignore_unknown_formats = false, formats = {{ \"uuid\" => crate::reference_uuid_format, \"hostname\" => crate::reference_hostname_format }}, pattern_options = {{ engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 }}, methods = {{ is_valid = true, validate = false, iter_errors = false }}"
        );
        syn::parse2(jsonschema_macros_core::expand(
            options.parse().unwrap(),
            "pub struct BoundControl;".parse().unwrap(),
        ))
        .unwrap()
    }

    fn assert_onepass_membership(built: &BTreeSet<String>, refused: &BTreeSet<String>) {
        let helpers = schema_profile::helpers();
        assert_eq!(
            *built,
            helpers
                .iter()
                .filter(|(_, data)| data["onepass"] == true)
                .map(|(name, _)| name.clone())
                .collect()
        );
        assert_eq!(
            *refused,
            helpers
                .iter()
                .filter(|(_, data)| data["onepass"] == false)
                .map(|(name, _)| name.clone())
                .collect()
        );
    }

    fn expected_helper_prefix_counts() -> BTreeMap<String, usize> {
        schema_profile::helpers()
            .iter()
            .map(|(name, data)| {
                (
                    name.clone(),
                    usize::try_from(data["prefixes"].as_u64().unwrap()).unwrap(),
                )
            })
            .collect()
    }

    #[test]
    fn all_emitted_helpers_compose_fixed_cache_owners_without_partial_totals() {
        use crate::schema_meta_heap::MetaCacheHeap;
        let inputs = schema_profile::inputs();
        let snapshot = &inputs.snapshot;
        let bidders = &snapshot.root;
        assert_eq!(snapshot.schemas.len(), schema_profile::schema_count());
        let mut completed = BTreeMap::new();
        let mut refused = BTreeMap::new();
        let mut prefix_owners = BTreeSet::new();
        for schema in &snapshot.schemas {
            let syntax = expand_schema(&bidders.join(&schema.witness.path), &schema.audit.draft);
            for (name, pattern) in emitted_patterns(&syntax).unwrap() {
                let key = format!("{}:{name}", schema.witness.name);
                match MetaCacheHeap::from_pattern(&pattern, 0x0001_0000) {
                    Ok(bound) => {
                        writeln!(stderr(), "fixed native meta cache {key}: {bound}").unwrap();
                        assert!(bound.eager_bytes > 0);
                        assert!(bound.forward_bytes > 0);
                        assert!(bound.reverse_bytes > 0);
                        assert!(bound.fallback_bytes > 0);
                        if bound.prefix_peak_bytes > 0 {
                            assert!(prefix_owners.insert(key.clone()));
                        }
                        assert!(completed.insert(key, bound).is_none());
                    }
                    Err(reason) => {
                        writeln!(stderr(), "fixed native meta cache refused {key}: {reason}")
                            .unwrap();
                        assert!(refused.insert(key, reason).is_none());
                    }
                }
            }
        }
        assert!(refused.is_empty(), "meta cache refusals: {refused:?}");
        assert_eq!(
            completed.keys().cloned().collect::<BTreeSet<_>>(),
            expected_helper_prefix_counts().into_keys().collect()
        );
        assert_eq!(completed.len(), schema_profile::helper_count());
        assert_eq!(
            prefix_owners,
            schema_profile::helpers()
                .iter()
                .filter(|(_, data)| data["alternative"] == true)
                .map(|(name, _)| name.clone())
                .collect()
        );
    }

    #[test]
    fn all_emitted_helpers_fund_fresh_core_auxiliary_owners_on_both_word_widths() {
        use crate::schema_heap_layout::HeapLayout;
        use crate::schema_meta_eager::CoreEagerHeap;
        let inputs = schema_profile::inputs();
        let snapshot = &inputs.snapshot;
        let bidders = &snapshot.root;
        assert_eq!(snapshot.schemas.len(), schema_profile::schema_count());
        let mut completed = BTreeSet::new();
        let mut refused = BTreeMap::new();
        let mut maximum = [0_usize; 2];
        for schema in &snapshot.schemas {
            let syntax = expand_schema(&bidders.join(&schema.witness.path), &schema.audit.draft);
            for (name, pattern) in emitted_patterns(&syntax).unwrap() {
                let nfa = forward_nfa(&pattern).unwrap();
                for (index, word) in [4_usize, 8].into_iter().enumerate() {
                    let key = (format!("{}:{name}", schema.witness.name), word);
                    match CoreEagerHeap::from_nfa(&nfa, HeapLayout::new(word, 16).unwrap()) {
                        Ok(bound) => {
                            writeln!(
                                stderr(),
                                "fresh Core auxiliary {} word={word}: {bound}",
                                key.0
                            )
                            .unwrap();
                            maximum[index] = maximum[index].max(bound.total);
                            assert!(completed.insert(key));
                        }
                        Err(reason) => assert!(refused.insert(key, reason).is_none()),
                    }
                }
            }
        }
        let expected = expected_helper_prefix_counts()
            .into_keys()
            .flat_map(|key| [4_usize, 8].map(|word| (key.clone(), word)))
            .collect();
        assert!(
            refused.is_empty(),
            "fresh Core auxiliary refusals: {refused:?}"
        );
        assert_eq!(completed, expected);
        assert_eq!(completed.len(), schema_profile::helper_count() * 2);
        assert_eq!(
            maximum,
            if crate::SCHEMA_SOURCE_CONFORMANCE {
                [40, 80]
            } else {
                [8, 16]
            }
        );
    }

    #[test]
    fn all_emitted_helpers_include_retained_lazy_fallbacks_on_both_word_widths() {
        use crate::schema_fallback_heap::FallbackHeap;
        use crate::schema_heap_layout::HeapLayout;
        let inputs = schema_profile::inputs();
        let snapshot = &inputs.snapshot;
        let bidders = &snapshot.root;
        assert_eq!(snapshot.schemas.len(), schema_profile::schema_count());
        let mut completed = BTreeSet::new();
        let mut refused = BTreeMap::new();
        for schema in &snapshot.schemas {
            let syntax = expand_schema(&bidders.join(&schema.witness.path), &schema.audit.draft);
            for (name, pattern) in emitted_patterns(&syntax).unwrap() {
                let nfa = forward_nfa(&pattern).unwrap();
                for word in [4_usize, 8] {
                    let key = (format!("{}:{name}", schema.witness.name), word);
                    match FallbackHeap::from_nfa(
                        &nfa,
                        HeapLayout::new(word, 16).unwrap(),
                        0x0001_0000,
                    ) {
                        Ok(bound) => {
                            assert!(bound.pike_sets > 0 && bound.pike_slots > 0);
                            assert!(bound.backtrack_bitmap > 0 && bound.backtrack_stack > 0);
                            assert_eq!(bound.backtrack_max_span, Some(128));
                            writeln!(stderr(), "retained fallback {} word={word}: {bound}", key.0)
                                .unwrap();
                            assert!(completed.insert(key));
                        }
                        Err(reason) => assert!(refused.insert(key, reason).is_none()),
                    }
                }
            }
        }
        let expected = expected_helper_prefix_counts()
            .into_keys()
            .flat_map(|key| [4_usize, 8].map(|word| (key.clone(), word)))
            .collect();
        assert!(refused.is_empty(), "fallback refusals: {refused:?}");
        assert_eq!(completed, expected);
        assert_eq!(completed.len(), schema_profile::helper_count() * 2);
    }

    fn audit_hybrid_direction(nfa: &NFA, direction: HybridDirection) -> HybridDimensions {
        let dfa = candidate_hybrid(nfa, direction).unwrap();
        let projected = HybridDimensions::from_dfa(&dfa).unwrap();
        assert_eq!(
            dfa.create_cache().memory_usage(),
            projected.fresh_reported_bytes(size_of::<usize>()).unwrap()
        );
        let population = HybridPopulation::from_dfa(&dfa).unwrap().unwrap();
        let heap = HybridHeap::from_dfa(&dfa).unwrap().unwrap();
        let closed = HybridHeap::from_complete_dfa(&dfa).unwrap().unwrap();
        assert!(closed.live_envelope <= heap.live_envelope);
        assert!(closed.growth_overlap <= heap.growth_overlap);
        assert!(closed.peak > 0 && closed.peak <= heap.peak);
        writeln!(
            stderr(),
            "hybrid population projection: reverse={} {population} requested-heap=({heap}) complete-graph-heap=({closed})",
            nfa.is_reverse()
        )
        .unwrap();
        projected
    }

    fn onepass_raw_candidate_builds(nfa: &NFA, scratch: usize) -> bool {
        match candidate_onepass(nfa) {
            Ok(engine) => {
                assert_eq!(
                    engine.create_cache().memory_usage(),
                    scratch
                        .checked_mul(size_of::<Option<NonMaxUsize>>())
                        .unwrap()
                );
                true
            }
            Err(build_error) => {
                assert!(build_error.to_string().contains("pattern is not one-pass:"));
                false
            }
        }
    }

    fn audit_hybrid_pair(nfa: &NFA, pattern: &str) -> (HybridDimensions, HybridDimensions, usize) {
        let reversed = reverse_nfa(pattern).unwrap();
        assert!(reversed.is_reverse());
        let reverse_shape = NfaDimensions::from_nfa(&reversed).unwrap();
        assert_eq!((reverse_shape.slots, reverse_shape.capture_states), (0, 0));
        let forward = audit_hybrid_direction(nfa, HybridDirection::Forward);
        let reverse = audit_hybrid_direction(&reversed, HybridDirection::Reverse);
        let reported = forward
            .fresh_reported_bytes(size_of::<usize>())
            .unwrap()
            .checked_add(reverse.fresh_reported_bytes(size_of::<usize>()).unwrap())
            .unwrap();
        (forward, reverse, reported)
    }

    #[test]
    fn nfa_inputs_are_the_emitted_translation_not_the_raw_schema_pattern() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("translated.json");
        fs::write(
            &path,
            serde_json::to_vec(&json!({"type": "string", "pattern": "\\d+"})).unwrap(),
        )
        .unwrap();
        let syntax = expand_schema(&path, &Draft::Draft4);
        let patterns = emitted_patterns(&syntax).unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].1, "[0-9]+");
    }

    #[test]
    fn every_emitted_primary_hybrid_graph_is_completed_or_explicitly_refused() {
        let inputs = schema_profile::inputs();
        let snapshot = &inputs.snapshot;
        let bidders = &snapshot.root;
        assert_eq!(snapshot.schemas.len(), schema_profile::schema_count());
        let mut attempted = BTreeSet::new();
        let mut completed = BTreeSet::new();
        let mut capacity_proved = BTreeSet::new();
        let mut capacity_refused = BTreeMap::new();
        let mut refused = BTreeMap::new();
        for schema in &snapshot.schemas {
            let syntax = expand_schema(&bidders.join(&schema.witness.path), &schema.audit.draft);
            for (name, pattern) in emitted_patterns(&syntax).unwrap() {
                for (nfa, direction, label) in [
                    (
                        forward_nfa(&pattern).unwrap(),
                        HybridDirection::Forward,
                        "forward",
                    ),
                    (
                        reverse_nfa(&pattern).unwrap(),
                        HybridDirection::Reverse,
                        "reverse",
                    ),
                ] {
                    let key = format!("{}:{name}:{label}", schema.witness.name);
                    assert!(attempted.insert(key.clone()));
                    let dfa = candidate_hybrid(&nfa, direction).unwrap();
                    match ReportedCapacityProof::from_non_quitting_dfa(&dfa) {
                        Ok(capacity) => {
                            writeln!(
                                stderr(),
                                "hybrid no-quit/no-capacity-clear {key}: {capacity}"
                            )
                            .unwrap();
                            assert!(capacity_proved.insert(key.clone()));
                        }
                        Err(reason) => {
                            writeln!(stderr(), "hybrid unproved capacity {key}: {reason}").unwrap();
                            assert!(capacity_refused.insert(key, reason).is_none());
                            continue;
                        }
                    }
                    match certify(&dfa, QUALIFICATION_BUDGET) {
                        Ok(certificate) => {
                            writeln!(stderr(), "hybrid complete graph {key}: {certificate}")
                                .unwrap();
                            assert!(completed.insert(key));
                        }
                        Err(reason) => {
                            writeln!(stderr(), "hybrid unproved graph {key}: {reason}").unwrap();
                            assert!(refused.insert(key, reason.to_string()).is_none());
                        }
                    }
                }
            }
        }
        assert_eq!(attempted.len(), schema_profile::helper_count() * 2);
        let mut expected = BTreeSet::new();
        for key in expected_helper_prefix_counts().keys() {
            for direction in ["forward", "reverse"] {
                assert!(expected.insert(format!("{key}:{direction}")));
            }
        }
        assert_eq!(attempted, expected);
        assert_eq!(
            completed
                .len()
                .checked_add(refused.len())
                .and_then(|count| count.checked_add(capacity_refused.len()))
                .unwrap(),
            schema_profile::helper_count() * 2
        );
        assert!(!completed.is_empty());
        assert!(
            refused.is_empty(),
            "unproved primary graphs: {}",
            refused.keys().cloned().collect::<Vec<_>>().join(",")
        );
        assert_eq!(completed, expected);
        assert!(
            capacity_refused.is_empty(),
            "unproved primary capacities: {capacity_refused:?}"
        );
        assert_eq!(capacity_proved, expected);
    }

    #[test]
    fn every_emitted_helper_has_a_complete_optional_prefix_envelope_or_typed_refusal() {
        let inputs = schema_profile::inputs();
        let snapshot = &inputs.snapshot;
        let bidders = &snapshot.root;
        assert_eq!(snapshot.schemas.len(), schema_profile::schema_count());
        let mut attempted = BTreeSet::new();
        let mut completed = BTreeMap::new();
        let mut dispositions = BTreeMap::new();
        let mut refused = BTreeMap::new();
        for schema in &snapshot.schemas {
            let syntax = expand_schema(&bidders.join(&schema.witness.path), &schema.audit.draft);
            for (name, pattern) in emitted_patterns(&syntax).unwrap() {
                let key = format!("{}:{name}", schema.witness.name);
                assert!(attempted.insert(key.clone()));
                let outcome = PrefixEnvelope::from_pattern(&pattern).and_then(|envelope| {
                    for_meta_pattern(&pattern).map(|admitted| (envelope, admitted))
                });
                match outcome {
                    Ok((envelope, admitted)) => {
                        writeln!(stderr(), "optional-prefix envelope {key}: candidates={} requested-peak={} required-reported={}", envelope.candidates, envelope.peak_bytes, envelope.required_reported_bytes).unwrap();
                        if envelope.candidates > 0 {
                            assert!(envelope.peak_bytes > 0);
                            assert!(envelope.required_reported_bytes > 0);
                        }
                        let kind = match admitted {
                            OptionalPrefixEnvelope::ExcludedAnchored {
                                structural_candidates,
                            } => {
                                assert_eq!(structural_candidates, envelope.candidates);
                                "excluded_anchored"
                            }
                            OptionalPrefixEnvelope::NoConcat => {
                                assert_eq!(envelope.candidates, 0);
                                "no_concat"
                            }
                            OptionalPrefixEnvelope::Alternatives(qualified) => {
                                assert_eq!(qualified, envelope);
                                "alternatives"
                            }
                        };
                        assert!(dispositions.insert(key.clone(), kind).is_none());
                        assert!(completed.insert(key, envelope.candidates).is_none());
                    }
                    Err(reason) => {
                        writeln!(
                            stderr(),
                            "unproved optional-prefix envelope {key}: {reason}"
                        )
                        .unwrap();
                        assert!(refused.insert(key, reason).is_none());
                    }
                }
            }
        }
        let expected = expected_helper_prefix_counts();
        assert_eq!(attempted, expected.keys().cloned().collect());
        assert!(refused.is_empty(), "unproved optional owners: {refused:?}");
        assert_eq!(completed, expected);
        for (key, prefixes) in expected {
            let kind = if schema_profile::helpers()[&key]["alternative"] == true {
                "alternatives"
            } else if prefixes == 0 {
                "no_concat"
            } else {
                "excluded_anchored"
            };
            assert_eq!(dispositions.get(&key), Some(&kind));
        }
    }

    fn assert_forward_storage_maxima(
        (helpers, schemas, max_slots, max_tables): (usize, usize, usize, usize),
        (max_words32, max_words64, max_backtrack_frames): (usize, usize, usize),
    ) {
        if crate::SCHEMA_SOURCE_CONFORMANCE {
            assert_eq!((helpers, schemas, max_slots, max_tables), (23, 14, 6, 384));
            assert_eq!(
                (max_words32, max_words64, max_backtrack_frames),
                (190, 95, 1161)
            );
        } else {
            assert_eq!((helpers, schemas), (2, 1));
            assert!(max_slots <= 6 && max_tables <= 384);
            assert!(max_words32 <= 190 && max_words64 <= 95 && max_backtrack_frames <= 1161);
        }
    }

    fn assert_raw_candidate_maxima(
        (max_onepass_slots, max_built_slots): (usize, usize),
        onepass_built: &BTreeSet<String>,
        onepass_refusals: &BTreeSet<String>,
        (hybrid_pairs, max_hybrid_pair_reported): (usize, usize),
        (max_reverse_sparse, max_reverse_stack): (usize, usize),
    ) {
        if crate::SCHEMA_SOURCE_CONFORMANCE {
            assert_eq!(
                onepass_built
                    .len()
                    .checked_add(onepass_refusals.len())
                    .unwrap(),
                23
            );
            assert_eq!(max_onepass_slots, 4);
            assert_eq!((onepass_built.len(), max_built_slots), (19, 2));
            assert_eq!(hybrid_pairs, 23);
            assert_eq!((max_reverse_sparse, max_reverse_stack), (292, 31));
            assert_eq!(
                max_hybrid_pair_reported,
                if size_of::<usize>() == 4 { 3486 } else { 3550 }
            );
        } else {
            assert_eq!((onepass_built.len(), onepass_refusals.len()), (2, 0));
            assert_eq!(hybrid_pairs, 2);
            assert!(max_onepass_slots <= 4 && max_built_slots <= 2);
            assert!(max_reverse_sparse <= 292 && max_reverse_stack <= 31);
            assert!(max_hybrid_pair_reported <= if size_of::<usize>() == 4 { 3486 } else { 3550 });
        }
        assert_onepass_membership(onepass_built, onepass_refusals);
    }

    #[test]
    fn every_generated_forward_nfa_has_checked_fallback_storage_dimensions() {
        let inputs = schema_profile::inputs();
        let snapshot = &inputs.snapshot;
        let bidders = &snapshot.root;
        assert_eq!(snapshot.schemas.len(), schema_profile::schema_count());
        let (mut helpers, mut schemas, mut max_slots, mut max_tables) =
            (0_usize, 0_usize, 0_usize, 0_usize);
        let (mut max_words32, mut max_words64, mut max_backtrack_frames) =
            (0_usize, 0_usize, 0_usize);
        let mut rows = String::new();
        let (mut max_onepass_slots, mut max_built_slots) = (0_usize, 0_usize);
        let mut onepass_built = BTreeSet::new();
        let mut onepass_refusals = BTreeSet::new();
        let (mut hybrid_pairs, mut max_hybrid_pair_reported) = (0_usize, 0_usize);
        let (mut max_reverse_sparse, mut max_reverse_stack) = (0_usize, 0_usize);
        for schema in &snapshot.schemas {
            let syntax = expand_schema(&bidders.join(&schema.witness.path), &schema.audit.draft);
            let patterns = emitted_patterns(&syntax).unwrap();
            let names: BTreeSet<_> = patterns.iter().map(|(name, _)| name).collect();
            assert_eq!(names.len(), patterns.len());
            helpers = helpers.checked_add(patterns.len()).unwrap();
            schemas = schemas
                .checked_add(usize::from(!patterns.is_empty()))
                .unwrap();
            for (name, pattern) in patterns {
                let nfa = forward_nfa(&pattern).unwrap();
                let shape = NfaDimensions::from_nfa(&nfa).unwrap();
                assert_eq!(shape.patterns, 1);
                let storage = shape.pike_storage().unwrap();
                max_slots = max_slots.max(shape.slots);
                max_tables = max_tables.max(storage.table_entries);
                let backtrack32 = BacktrackDimensions::for_earliest(shape, 128, 128, 32)
                    .unwrap()
                    .unwrap();
                let backtrack64 = BacktrackDimensions::for_earliest(shape, 128, 128, 64)
                    .unwrap()
                    .unwrap();
                max_words32 = max_words32.max(backtrack32.visited_words);
                max_words64 = max_words64.max(backtrack64.visited_words);
                max_backtrack_frames = max_backtrack_frames.max(backtrack64.stack_frames);
                assert_eq!(backtrack32.stack_frames, backtrack64.stack_frames);
                assert_eq!(
                    BacktrackDimensions::for_earliest(shape, 129, 0, 64),
                    Ok(None)
                );
                let scratch = explicit_scratch_slots(shape.slots, shape.patterns)
                    .unwrap()
                    .unwrap();
                assert_eq!(scratch, nfa.group_info().explicit_slot_len());
                max_onepass_slots = max_onepass_slots.max(scratch);
                if onepass_raw_candidate_builds(&nfa, scratch) {
                    assert!(onepass_built.insert(format!("{}:{name}", schema.witness.name)));
                    max_built_slots = max_built_slots.max(scratch);
                } else {
                    assert!(onepass_refusals.insert(format!("{}:{name}", schema.witness.name)));
                }
                let (hybrid_forward, hybrid_reverse, pair_reported) =
                    audit_hybrid_pair(&nfa, &pattern);
                hybrid_pairs += 1;
                max_hybrid_pair_reported = max_hybrid_pair_reported.max(pair_reported);
                max_reverse_sparse = max_reverse_sparse.max(hybrid_reverse.sparse_entries);
                max_reverse_stack = max_reverse_stack.max(hybrid_reverse.epsilon_stack);
                writeln!(
                    rows,
                    "{} {name} nfa=({shape}) pike=({storage}) backtrack32=({backtrack32}) backtrack64=({backtrack64}) hybrid-forward=({hybrid_forward}) hybrid-reverse=({hybrid_reverse}) fresh-pair-reported={pair_reported}",
                    schema.witness.name
                )
                .unwrap();
            }
        }
        assert_forward_storage_maxima(
            (helpers, schemas, max_slots, max_tables),
            (max_words32, max_words64, max_backtrack_frames),
        );
        assert_raw_candidate_maxima(
            (max_onepass_slots, max_built_slots),
            &onepass_built,
            &onepass_refusals,
            (hybrid_pairs, max_hybrid_pair_reported),
            (max_reverse_sparse, max_reverse_stack),
        );
        writeln!(stderr(), "hybrid raw candidate audit: pairs={hybrid_pairs} max-fresh-reported-pair={max_hybrid_pair_reported}").unwrap();
        writeln!(stderr(), "one-pass raw candidate audit: built={} refused={} max-explicit-slots={max_onepass_slots}", onepass_built.len(), onepass_refusals.len()).unwrap();
        for name in &onepass_refusals {
            writeln!(stderr(), "one-pass raw candidate refusal: {name}").unwrap();
        }
        writeln!(stderr(), "forward-nfa logical audit: schemas=261 helper-schemas={schemas} helpers={helpers} max-slots={max_slots} max-table-entries={max_tables}\n{rows}").unwrap();
    }
}
