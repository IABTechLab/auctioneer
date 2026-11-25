//! Tests-first canonical CURRENT report, not a completeness or serving receipt.
//!
//! Only the sealed view and same-capture checked bodies supply source authority.
//! Coverage candidates, caller-duty labels and rendered JSON remain DATA. Caller
//! roots are separate from descriptor children; their duties are not discharged
//! by listing them. Common derivations and matched target facts remain missing.
//! No historical reader, component assembler, source loader or build is added.

use super::{handoff, operations};
use crate::schema_build::application::CapturedSeed;
use crate::schema_residency::descriptor::{ValidationAssignment, ValidationView};
use crate::schema_selection_source::public_names::PublicNameCensus;
use serde_json::Value;

type Bodies<'a> = handoff::CheckedBodies<
    'a,
    (
        String,
        Result<operations::BodyOperations, operations::Refusal>,
    ),
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContextRole {
    InputMeasurement,
    ValidationCoordination,
}

// Report obligations, not additional work units or priced source events.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Duty {
    Work,
    RequestedStorage,
    StructuralStack,
    OrdinaryCleanup,
    Attempts,
    ExternalServices,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ContextDuty {
    role: ContextRole,
    source_owner: String,
    duties: Vec<Duty>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CoverageData {
    binding: Value,
    children: Vec<ValidationAssignment>,
    contexts: Vec<ContextDuty>,
}

// Private diagnostic output only. There is deliberately no Deserialize, cost
// conversion, common-completion constructor or qualified receipt in this API.
#[derive(Debug, Eq, PartialEq)]
struct CurrentReport {
    document: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    MissingSourceProof,
    Binding,
    ChildCoverage,
    ProgramCoverage,
    ContextCoverage,
    ContextDuties(ContextRole),
    RetiredCandidate,
    NonCanonicalField,
    IncompleteSource,
}

// Refusal-only until all four cases reach hosted assertion RED. Later source
// integration must reuse handoff/public_names and all real closing checks.
fn describe_validation(
    _seed: &CapturedSeed,
    _view: ValidationView<'_>,
    _bodies: &Bodies<'_>,
    _census: &PublicNameCensus,
    _coverage: &CoverageData,
) -> Result<CurrentReport, Refusal> {
    Err(Refusal::MissingSourceProof)
}

// An in-memory output-format check, never deserialization of source authority.
fn check_current_report(_document: &Value) -> Result<(), Refusal> {
    Err(Refusal::MissingSourceProof)
}

// Only a refusal is representable; this cannot grant an installed cost/route.
fn qualification_refusal(_report: &CurrentReport) -> Refusal {
    Refusal::MissingSourceProof
}

#[cfg(test)]
mod tests {
    use super::{
        Bodies, ContextDuty, ContextRole, CoverageData, Duty, Refusal, check_current_report,
        describe_validation, qualification_refusal,
    };
    use crate::schema_build::application::CapturedSeed;
    use crate::schema_residency::{
        descriptor::ValidationView, read_regular, with_current_descriptor,
    };
    use crate::schema_selection_source::{
        Lane,
        public_names::{self, Disposition, PublicNameCensus},
    };
    use crate::schema_validation::{handoff, operations, public_names as dispositions};
    use proc_macro2::{TokenStream, TokenTree};
    use quote::ToTokens as _;
    use serde_json::{Value, json};
    use std::{
        collections::BTreeSet,
        fs, io, mem,
        path::{Path, PathBuf},
        str::from_utf8,
    };
    use syn::ext::IdentExt as _;
    use syn::visit_mut::{self, VisitMut};

    fn contexts() -> Vec<ContextDuty> {
        vec![
            ContextDuty {
                role: ContextRole::InputMeasurement,
                source_owner: "auctioneer_bidder::schema::measure_schema_input".to_owned(),
                duties: vec![
                    Duty::Work,
                    Duty::RequestedStorage,
                    Duty::StructuralStack,
                    Duty::OrdinaryCleanup,
                    Duty::Attempts,
                ],
            },
            ContextDuty {
                role: ContextRole::ValidationCoordination,
                source_owner: "auctioneer_bidder::schema::validate_native_parameters".to_owned(),
                duties: vec![
                    Duty::Work,
                    Duty::RequestedStorage,
                    Duty::StructuralStack,
                    Duty::OrdinaryCleanup,
                    Duty::Attempts,
                    Duty::ExternalServices,
                ],
            },
        ]
    }

    fn programs(view: ValidationView<'_>) -> BTreeSet<String> {
        let mut result = BTreeSet::from(["schema::staging::formats::HOSTNAME_PATTERN".to_owned()]);
        for declaration in view.graph()["declarations"].as_array().unwrap() {
            for program in declaration["programs"].as_array().unwrap() {
                assert!(result.insert(program["identity"].as_str().unwrap().to_owned()));
            }
        }
        result
    }

    fn assert_child_inventory(view: ValidationView<'_>, coverage: &CoverageData) {
        assert_eq!(coverage.binding, *view.binding());
        assert_eq!(coverage.children.as_slice(), view.assignments());
        assert_eq!(view.check_handoff(&coverage.children), Ok(()));
        assert!(!coverage.children.is_empty());
        let roles = |owner: &str| {
            coverage
                .children
                .iter()
                .filter(|row| row.source_owner == owner)
                .map(|row| row.role.as_str())
                .collect::<BTreeSet<_>>()
        };
        let program_ids = programs(view);
        assert!(!program_ids.is_empty());
        for owner in &program_ids {
            assert_eq!(
                roles(owner),
                BTreeSet::from(["cache", "mutable-scratch", "search-fallback"])
            );
        }
        let declarations = view.graph()["declarations"].as_array().unwrap();
        assert!(!declarations.is_empty());
        let mut callbacks = BTreeSet::new();
        for declaration in declarations {
            let owner = format!(
                "{}::{}",
                declaration["owner"].as_str().unwrap(),
                declaration["marker"].as_str().unwrap()
            );
            assert_eq!(
                roles(&owner),
                BTreeSet::from(["declaration-stack", "declaration-work"])
            );
            for callback in declaration["callbacks"].as_array().unwrap() {
                callbacks.insert(callback.as_str().unwrap());
            }
        }
        assert!(
            declarations
                .iter()
                .any(|row| row["programs"].as_array().unwrap().is_empty())
        );
        for callback in &callbacks {
            assert_eq!(
                roles(&format!("schema::staging::formats::{callback}")),
                BTreeSet::from(["callback-stack", "callback-work"])
            );
        }
        assert_eq!(
            coverage.children.len(),
            program_ids
                .len()
                .checked_mul(3)
                .unwrap()
                .checked_add(declarations.len().checked_mul(2).unwrap())
                .unwrap()
                .checked_add(callbacks.len().checked_mul(2).unwrap())
                .unwrap()
        );
    }

    fn with_case(
        inspect: impl FnOnce(
            &CapturedSeed,
            ValidationView<'_>,
            &Bodies<'_>,
            &PublicNameCensus,
            &CoverageData,
        ),
    ) {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            handoff::check_seed(seed, view).expect("accepted current capture before report setup");
            let bodies = handoff::checked_bodies(seed, view, |body| {
                Ok((
                    body.owner.to_owned(),
                    operations::classify_source_body(body.syntax, &[], &[]),
                ))
            })
            .expect("existing checked body owner, not another parser/build");
            assert_eq!(bodies.recheck(view), Ok(()));
            assert_eq!(
                bodies.analyses().len(),
                view.graph()["declarations"].as_array().unwrap().len()
            );
            assert!(bodies.analyses().iter().all(|(_, body)| body.is_ok()));
            let lane = if cfg!(feature = "insecure-bidders") {
                Lane::Development
            } else {
                Lane::Secure
            };
            let census = public_names::discover(Path::new(env!("CARGO_MANIFEST_DIR")), lane)?;
            assert!(!census.names.is_empty());
            assert_eq!(
                dispositions::check_public_dispositions(seed, view, &census),
                Ok(())
            );
            let coverage = CoverageData {
                binding: view.binding().clone(),
                children: view.assignments().to_vec(),
                contexts: contexts(),
            };
            assert_child_inventory(view, &coverage);
            inspect(seed, view, &bodies, &census, &coverage);
            assert_eq!(bodies.recheck(view), Ok(()));
            Ok(())
        })
        .unwrap();
    }

    fn expected_coverage(view: ValidationView<'_>, census: &PublicNameCensus) -> Value {
        let children: Vec<_> = view
            .assignments()
            .iter()
            .map(|row| {
                json!({
                    "child": row.child, "source_owner": row.source_owner, "role": row.role
                })
            })
            .collect();
        let names: Vec<_> = census
            .names
            .iter()
            .map(|name| {
                let (disposition, owner) = match &name.disposition {
                    Disposition::SchemaOwner(owner) => ("schema_owner", Some(owner)),
                    Disposition::LocalSchemaFree => ("local_schema_free", None),
                };
                json!({"name":name.name, "disposition":disposition, "source_owner":owner})
            })
            .collect();
        json!({
            "programs": programs(view), "children": children, "public_names": names,
            "contexts": [
                {"role":"input_measurement", "source_owner":"auctioneer_bidder::schema::measure_schema_input",
                 "duties":["work","requested_storage","structural_stack","ordinary_cleanup","attempts"],
                 "source_proof":"not_established"},
                {"role":"validation_coordination", "source_owner":"auctioneer_bidder::schema::validate_native_parameters",
                 "duties":["work","requested_storage","structural_stack","ordinary_cleanup","attempts","external_services"],
                 "source_proof":"not_established"}
            ]
        })
    }

    // Expected output DATA only. This never constructs CurrentReport or a view.
    fn expected_document(view: ValidationView<'_>, census: &PublicNameCensus) -> Value {
        json!({
            "schema_version":1_u8, "kind":"schema_validation_resources", "binding":view.binding(),
            "status":"Incomplete", "coverage":expected_coverage(view, census),
            "premises":{
                "input_caps":"caller_source_join_required", "fresh_serial_lifetime":"source_proof_required",
                "allocation_success":"matched_allocator_and_artifact_fact_required"
            },
            "source_exclusions":[{"kind":"terminal_allocation_failure_diagnostics",
                "conditional_on":"allocation_success", "ordinary_error_unwind_cleanup_excluded":false}],
            "derivations":{
                "storage":{"status":"Incomplete","reason":"storage_lifecycle_not_closed"},
                "work":{"status":"Incomplete","reason":"ordinary_primitive_work_not_closed"},
                "structural_stack":{"status":"Incomplete","reason":"primitive_stack_not_closed"},
                "shape_correspondence":{"status":"Incomplete","reason":"source_affine_coefficients_not_closed"},
                "attempted_work":{"status":"Incomplete","reason":"caller_attempt_closure_not_closed"}
            },
            "required_target_facts":["matched_artifact_layout_and_frames","allocator_allocation_success",
                "clock_service_facts","entropy_service_facts","target_policy_and_funding_acceptance"],
            "observations":{"status":"not_collected","scope":"non_qualifying","rows":[]},
            "numeric_policy_adequacy":"not_evaluated", "attempted_work_funding":"not_established",
            "target_status":"TargetIncomplete", "resource_qualification":false,
            "physical_fit":false, "serving_permission":false
        })
    }

    fn assert_children_refuse(
        seed: &CapturedSeed,
        view: ValidationView<'_>,
        bodies: &Bodies<'_>,
        census: &PublicNameCensus,
        coverage: &CoverageData,
    ) {
        let original = coverage.children.first().unwrap();
        let mut missing = coverage.clone();
        assert_eq!(missing.children.remove(0), *original);
        let mut duplicate = coverage.clone();
        duplicate.children.push(original.clone());
        let mut misowned = coverage.clone();
        misowned.children.first_mut().unwrap().source_owner = "fake_misowned_child".to_owned();
        let mut wrong_role = coverage.clone();
        wrong_role.children.first_mut().unwrap().role = "fake_unrecognized_role".to_owned();
        for changed in [&missing, &duplicate, &misowned, &wrong_role] {
            assert_ne!(changed.children, coverage.children);
            assert_eq!(
                describe_validation(seed, view, bodies, census, changed),
                Err(Refusal::ChildCoverage)
            );
        }
        let mut stale = coverage.clone();
        stale.binding["identity"] = json!("fake_stale_report_binding");
        assert_ne!(stale.binding, *view.binding());
        assert_eq!(
            describe_validation(seed, view, bodies, census, &stale),
            Err(Refusal::Binding)
        );
    }

    fn assert_contexts_refuse(
        seed: &CapturedSeed,
        view: ValidationView<'_>,
        bodies: &Bodies<'_>,
        census: &PublicNameCensus,
        coverage: &CoverageData,
    ) {
        assert_eq!(coverage.contexts.len(), 2);
        for (index, original) in coverage.contexts.iter().enumerate() {
            assert!(!original.source_owner.is_empty());
            assert!(!original.duties.is_empty());
            let mut missing = coverage.clone();
            assert_eq!(missing.contexts.remove(index), *original);
            let mut duplicate = coverage.clone();
            duplicate.contexts.push(original.clone());
            let mut misowned = coverage.clone();
            misowned.contexts.get_mut(index).unwrap().source_owner =
                "fake_misowned_context".to_owned();
            for changed in [&missing, &duplicate, &misowned] {
                assert_ne!(changed.contexts, coverage.contexts);
                assert_eq!(
                    describe_validation(seed, view, bodies, census, changed),
                    Err(Refusal::ContextCoverage)
                );
            }
            for (duty_index, duty) in original.duties.iter().enumerate() {
                let mut absent = coverage.clone();
                assert_eq!(
                    absent
                        .contexts
                        .get_mut(index)
                        .unwrap()
                        .duties
                        .remove(duty_index),
                    *duty
                );
                let mut repeated = coverage.clone();
                repeated.contexts.get_mut(index).unwrap().duties.push(*duty);
                for changed in [&absent, &repeated] {
                    assert_eq!(
                        describe_validation(seed, view, bodies, census, changed),
                        Err(Refusal::ContextDuties(original.role))
                    );
                }
            }
        }
    }

    #[test]
    fn current_source_covers_every_discovered_owner() {
        with_case(|seed, view, bodies, census, coverage| {
            assert_eq!(
                describe_validation(seed, view, bodies, census, coverage)
                    .map(|report| report.document),
                Ok(expected_document(view, census))
            );
            assert_children_refuse(seed, view, bodies, census, coverage);
            assert_contexts_refuse(seed, view, bodies, census, coverage);
        });
    }

    fn assert_fieldset(current: &Value) {
        let fields: BTreeSet<_> = current
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            fields,
            BTreeSet::from([
                "schema_version",
                "kind",
                "binding",
                "status",
                "coverage",
                "premises",
                "source_exclusions",
                "derivations",
                "required_target_facts",
                "observations",
                "numeric_policy_adequacy",
                "attempted_work_funding",
                "target_status",
                "resource_qualification",
                "physical_fit",
                "serving_permission"
            ])
        );
        assert_eq!(fields.len(), 16);
    }

    fn assert_report_mutants(current: &Value) {
        for field in current.as_object().unwrap().keys() {
            let mut missing = current.clone();
            assert!(missing.as_object_mut().unwrap().remove(field).is_some());
            assert_eq!(
                check_current_report(&missing),
                Err(Refusal::NonCanonicalField)
            );
        }
        for (field, value) in [
            ("schema_version", json!(0_u8)),
            ("kind", json!("conditional_source_shape")),
            ("status", json!("CommonSourceComplete")),
            ("target_status", json!("TargetBound")),
            ("resource_qualification", json!(true)),
            ("physical_fit", json!(true)),
            ("serving_permission", json!(true)),
            ("numeric_policy_adequacy", json!("qualified")),
            ("attempted_work_funding", json!("funded")),
            ("heuristic_fallback", json!({"work_units":1_024_u64})),
        ] {
            let mut changed = current.clone();
            assert_ne!(changed.get(field), Some(&value));
            changed[field] = value;
            assert_eq!(
                check_current_report(&changed),
                Err(Refusal::NonCanonicalField)
            );
        }
        let old = json!({"schema_version":1_u8, "kind":"conditional_native_schema_cache_heap_candidate",
            "native_cache_heap_candidate":{"requested_bytes":7_u64}});
        assert_eq!(check_current_report(&old), Err(Refusal::RetiredCandidate));
        for (field, value) in [
            (
                "native_cache_heap_candidate",
                json!({"requested_bytes":7_u64}),
            ),
            (
                "kind",
                json!("conditional_native_schema_cache_heap_candidate"),
            ),
        ] {
            let mut hybrid = current.clone();
            assert_ne!(hybrid.get(field), Some(&value));
            hybrid[field] = value;
            assert_eq!(
                check_current_report(&hybrid),
                Err(Refusal::RetiredCandidate)
            );
        }
        let mut missing_role = current.clone();
        let contexts = missing_role["coverage"]["contexts"].as_array_mut().unwrap();
        assert_eq!(contexts.len(), 2);
        contexts.pop().unwrap();
        assert_eq!(
            check_current_report(&missing_role),
            Err(Refusal::ContextCoverage)
        );
    }

    fn assert_program_mutants(current: &Value) {
        let programs = current["coverage"]["programs"].as_array().unwrap();
        assert!(!programs.is_empty());
        let program = programs.first().unwrap().as_str().unwrap();
        let children = current["coverage"]["children"].as_array().unwrap();
        let associated = children
            .iter()
            .position(|row| row["source_owner"] == program)
            .unwrap();
        assert_eq!(
            children
                .iter()
                .filter(|row| row["source_owner"] == program)
                .count(),
            3
        );
        let mut omitted = current.clone();
        assert_eq!(
            omitted["coverage"]["programs"]
                .as_array_mut()
                .unwrap()
                .remove(0),
            program
        );
        assert_eq!(
            check_current_report(&omitted),
            Err(Refusal::ProgramCoverage)
        );
        let mut rebound = current.clone();
        rebound["coverage"]["programs"][0] = json!("fake_rebound_program");
        assert_ne!(
            rebound["coverage"]["programs"],
            current["coverage"]["programs"]
        );
        assert_eq!(
            check_current_report(&rebound),
            Err(Refusal::ProgramCoverage)
        );
        let mut missing_child = current.clone();
        let removed = missing_child["coverage"]["children"]
            .as_array_mut()
            .unwrap()
            .remove(associated);
        assert_eq!(removed["source_owner"], program);
        assert_eq!(
            check_current_report(&missing_child),
            Err(Refusal::ChildCoverage)
        );
        let mut missing_identity = current.clone();
        assert_eq!(
            missing_identity["coverage"]["children"][associated]
                .as_object_mut()
                .unwrap()
                .remove("source_owner"),
            Some(json!(program))
        );
        assert_eq!(
            check_current_report(&missing_identity),
            Err(Refusal::ChildCoverage)
        );
    }

    #[test]
    fn canonical_report_rejects_retired_and_hybrid_fields() {
        with_case(|_seed, view, _bodies, census, _coverage| {
            let current = expected_document(view, census);
            assert_fieldset(&current);
            assert_eq!(check_current_report(&current), Ok(()));
            assert_report_mutants(&current);
            assert_program_mutants(&current);
        });
    }

    #[derive(Debug, Default, Eq, PartialEq)]
    struct RetiredSites {
        declarations: BTreeSet<String>,
        calls: BTreeSet<String>,
        references: BTreeSet<String>,
        fields: BTreeSet<String>,
        kinds: BTreeSet<String>,
    }

    impl RetiredSites {
        fn combine(&mut self, other: Self) {
            self.declarations.extend(other.declarations);
            self.calls.extend(other.calls);
            self.references.extend(other.references);
            self.fields.extend(other.fields);
            self.kinds.extend(other.kinds);
        }
    }

    fn retired_function(name: &str) -> bool {
        matches!(
            name,
            "assert_selected_cache_heap" | "selected_cache_bundle" | "check_current_report_fields"
        )
    }

    fn retired_kind(kind: &str) -> bool {
        matches!(
            kind,
            "conditional_native_schema_cache_heap_candidate" | "conditional_source_shape"
        )
    }

    // Only a literal string or json!(literal string), not arbitrary dataflow.
    fn assigned_retired_kind(expression: &syn::Expr) -> Option<String> {
        let kind = match expression {
            syn::Expr::Lit(value) => {
                let syn::Lit::Str(value) = &value.lit else {
                    return None;
                };
                value.value()
            }
            syn::Expr::Macro(value)
                if value
                    .mac
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "json") =>
            {
                syn::parse2::<syn::LitStr>(value.mac.tokens.clone())
                    .ok()?
                    .value()
            }
            _ => return None,
        };
        retired_kind(&kind).then_some(kind)
    }

    fn retired_json(tokens: TokenStream) -> (bool, BTreeSet<String>) {
        let tokens: Vec<_> = tokens.into_iter().collect();
        let mut field = false;
        let mut kinds = BTreeSet::new();
        for window in tokens.windows(2) {
            let [TokenTree::Literal(key), TokenTree::Punct(colon)] = window else {
                continue;
            };
            if let Ok(key) =
                syn::parse2::<syn::LitStr>(TokenStream::from(TokenTree::Literal(key.clone())))
                && colon.as_char() == ':'
                && key.value() == "native_cache_heap_candidate"
            {
                field = true;
            }
        }
        for window in tokens.windows(3) {
            let [
                TokenTree::Literal(key),
                TokenTree::Punct(colon),
                TokenTree::Literal(value),
            ] = window
            else {
                continue;
            };
            let (Ok(key), Ok(value)) = (
                syn::parse2::<syn::LitStr>(TokenStream::from(TokenTree::Literal(key.clone()))),
                syn::parse2::<syn::LitStr>(TokenStream::from(TokenTree::Literal(value.clone()))),
            ) else {
                continue;
            };
            let kind = value.value();
            if key.value() == "kind" && colon.as_char() == ':' && retired_kind(&kind) {
                kinds.insert(kind);
            }
        }
        for token in &tokens {
            if let TokenTree::Group(group) = token {
                let (nested_field, nested_kinds) = retired_json(group.stream());
                field |= nested_field;
                kinds.extend(nested_kinds);
            }
        }
        (field, kinds)
    }

    // Authored census fixtures have no writer exclusions.
    fn census(syntax: &mut syn::File) -> RetiredSites {
        census_with(syntax, FixtureOwner::None)
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FixtureOwner {
        None,
        Report,
        Admission,
    }

    fn fixture_owner(path: &Path) -> FixtureOwner {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/schema_validation");
        if path == directory.join("report.rs") {
            FixtureOwner::Report
        } else if path == directory.join("admission.rs") {
            FixtureOwner::Admission
        } else {
            FixtureOwner::None
        }
    }

    fn private_unit_module(module: &syn::ItemMod) -> bool {
        module.ident == "tests"
            && matches!(module.vis, syn::Visibility::Inherited)
            && matches!(module.attrs.as_slice(), [attribute] if matches!(&attribute.style, syn::AttrStyle::Outer) && matches!(&attribute.meta,
                syn::Meta::List(list) if list.path.is_ident("cfg") && list.tokens.to_string() == "test"))
    }

    fn negative_fixture(owner: FixtureOwner, function: &syn::ItemFn) -> bool {
        if !matches!(function.vis, syn::Visibility::Inherited) {
            return false;
        }
        match owner {
            FixtureOwner::None => false,
            FixtureOwner::Report => {
                let expected: syn::Signature =
                    syn::parse_quote!(fn assert_report_mutants(current: &Value));
                function.sig.to_token_stream().to_string() == expected.to_token_stream().to_string()
                    && function.attrs.is_empty()
            }
            FixtureOwner::Admission => {
                let expected: syn::Signature =
                    syn::parse_quote!(fn retired_candidate_cannot_supply_costs());
                function.sig.to_token_stream().to_string() == expected.to_token_stream().to_string()
                    && matches!(function.attrs.as_slice(), [attribute] if attribute.path().is_ident("test")
                        && matches!(&attribute.style, syn::AttrStyle::Outer)
                        && matches!(&attribute.meta, syn::Meta::Path(_)))
            }
        }
    }

    // Only the two exact private negative-fixture functions may contain retired
    // writer data. Symbols/references are scanned there too. This is not a
    // wildcard test-module exemption or a general dataflow classifier.
    struct Inventory {
        sites: RetiredSites,
        writers: bool,
        owner: FixtureOwner,
        modules: Vec<String>,
        private_unit: bool,
        function: Option<String>,
    }
    impl VisitMut for Inventory {
        fn visit_item_mod_mut(&mut self, module: &mut syn::ItemMod) {
            let previous_writers = self.writers;
            let previous_unit = self.private_unit;
            let depth = self.modules.len();
            self.private_unit = self.modules.is_empty() && private_unit_module(module);
            self.modules.push(module.ident.unraw().to_string());
            self.writers = true;
            visit_mut::visit_item_mod_mut(self, module);
            self.writers = previous_writers;
            self.private_unit = previous_unit;
            self.modules.truncate(depth);
        }
        fn visit_item_fn_mut(&mut self, function: &mut syn::ItemFn) {
            let name = function.sig.ident.to_string();
            let previous_writers = self.writers;
            let ancestor = self.function.replace(name.clone());
            self.writers = !(ancestor.is_none()
                && self.private_unit
                && negative_fixture(self.owner, function));
            if retired_function(&name) {
                self.sites.declarations.insert(name);
            }
            visit_mut::visit_item_fn_mut(self, function);
            self.writers = previous_writers;
            self.function = ancestor;
        }
        fn visit_expr_path_mut(&mut self, path: &mut syn::ExprPath) {
            if let Some(segment) = path.path.segments.last()
                && retired_function(&segment.ident.to_string())
            {
                self.sites.references.insert(segment.ident.to_string());
            }
            visit_mut::visit_expr_path_mut(self, path);
        }
        fn visit_expr_call_mut(&mut self, call: &mut syn::ExprCall) {
            if let syn::Expr::Path(path) = call.func.as_ref()
                && let Some(segment) = path.path.segments.last()
                && retired_function(&segment.ident.to_string())
            {
                self.sites.calls.insert(segment.ident.to_string());
            }
            visit_mut::visit_expr_call_mut(self, call);
        }
        fn visit_expr_assign_mut(&mut self, assign: &mut syn::ExprAssign) {
            if self.writers
                && let syn::Expr::Index(index) = assign.left.as_ref()
                && let syn::Expr::Lit(key) = index.index.as_ref()
                && let syn::Lit::Str(key) = &key.lit
            {
                match key.value().as_str() {
                    "native_cache_heap_candidate" => {
                        self.sites.fields.insert(key.value());
                    }
                    "kind" => {
                        if let Some(kind) = assigned_retired_kind(assign.right.as_ref()) {
                            self.sites.kinds.insert(kind);
                        }
                    }
                    _ => {}
                }
            }
            visit_mut::visit_expr_assign_mut(self, assign);
        }
        fn visit_macro_mut(&mut self, mac: &mut syn::Macro) {
            if self.writers
                && mac
                    .path
                    .segments
                    .last()
                    .is_some_and(|segment| segment.ident == "json")
            {
                let (field, kinds) = retired_json(mac.tokens.clone());
                if field {
                    self.sites
                        .fields
                        .insert("native_cache_heap_candidate".to_owned());
                }
                self.sites.kinds.extend(kinds);
            }
            visit_mut::visit_macro_mut(self, mac);
        }
    }
    fn census_with(syntax: &mut syn::File, owner: FixtureOwner) -> RetiredSites {
        let mut inventory = Inventory {
            sites: RetiredSites::default(),
            writers: true,
            owner,
            modules: Vec::new(),
            private_unit: false,
            function: None,
        };
        inventory.visit_file_mut(syntax);
        inventory.sites
    }

    #[derive(Debug, Eq, PartialEq)]
    enum CensusRefusal {
        ModuleCoverage,
        Source,
    }

    fn path_token(tokens: TokenStream) -> bool {
        tokens.into_iter().any(|token| match token {
            TokenTree::Ident(name) => name == "path",
            TokenTree::Group(group) => path_token(group.stream()),
            TokenTree::Literal(_) | TokenTree::Punct(_) => false,
        })
    }

    fn ordinary_source(
        directory: &Path,
        name: &str,
    ) -> Result<(PathBuf, syn::File), CensusRefusal> {
        let mut found = Vec::new();
        for path in [
            directory.join(format!("{name}.rs")),
            directory.join(name).join("mod.rs"),
        ] {
            match fs::symlink_metadata(&path) {
                Ok(_) => found.push(path),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(_error) => return Err(CensusRefusal::Source),
            }
        }
        let [path] = found.as_slice() else {
            return Err(CensusRefusal::ModuleCoverage);
        };
        let bytes = read_regular(path).map_err(|_error| CensusRefusal::Source)?;
        let syntax = syn::parse_file(from_utf8(&bytes).map_err(|_error| CensusRefusal::Source)?)
            .map_err(|_error| CensusRefusal::Source)?;
        Ok((path.clone(), syntax))
    }

    // Follow all ordinary declarations, including cfg branches, with the
    // existing bounded regular reader. Missing/ambiguous modules never skip.
    fn module_census(
        syntax: &mut syn::File,
        directory: &Path,
        seen: &mut BTreeSet<PathBuf>,
        resolve: &mut impl FnMut(&Path, &str) -> Result<(PathBuf, syn::File), CensusRefusal>,
        owner: FixtureOwner,
    ) -> Result<RetiredSites, CensusRefusal> {
        struct Modules {
            directory: PathBuf,
            requests: Vec<(PathBuf, String)>,
            overridden: bool,
        }
        impl VisitMut for Modules {
            fn visit_item_mod_mut(&mut self, module: &mut syn::ItemMod) {
                self.overridden |= module.attrs.iter().any(|attribute| {
                    attribute.path().is_ident("path") || matches!(&attribute.meta,
                        syn::Meta::List(list) if list.path.is_ident("cfg_attr") && path_token(list.tokens.clone()))
                });
                let name = module.ident.unraw().to_string();
                if module.content.is_some() {
                    self.directory.push(&name);
                    visit_mut::visit_item_mod_mut(self, module);
                    self.directory.pop();
                } else {
                    self.requests.push((self.directory.clone(), name));
                }
            }
        }
        let mut result = census_with(syntax, owner);
        let mut modules = Modules {
            directory: directory.to_owned(),
            requests: Vec::new(),
            overridden: false,
        };
        modules.visit_file_mut(syntax);
        if modules.overridden {
            return Err(CensusRefusal::ModuleCoverage);
        }
        for (directory, name) in modules.requests {
            let (path, mut child) = resolve(&directory, &name)?;
            let owner = fixture_owner(&path);
            if !seen.insert(path) {
                return Err(CensusRefusal::ModuleCoverage);
            }
            result.combine(module_census(
                &mut child,
                &directory.join(name),
                seen,
                resolve,
                owner,
            )?);
        }
        Ok(result)
    }

    fn assert_module_controls() {
        let mut syntax: syn::File = syn::parse_quote!(
            mod observations;
        );
        let mut seen = BTreeSet::from([PathBuf::from("fixture/root.rs")]);
        let mut requested = false;
        let found = module_census(
            &mut syntax,
            Path::new("fixture"),
            &mut seen,
            &mut |directory, name| {
                assert_eq!(directory, Path::new("fixture"));
                assert_eq!(name, "observations");
                requested = true;
                Ok((
                    PathBuf::from("fixture/observations.rs"),
                    syn::parse_quote!(
                        fn selected_cache_bundle() {}
                    ),
                ))
            },
            FixtureOwner::None,
        )
        .unwrap();
        assert!(requested);
        assert!(seen.contains(Path::new("fixture/observations.rs")));
        assert_eq!(
            found,
            RetiredSites {
                declarations: BTreeSet::from(["selected_cache_bundle".to_owned()]),
                ..RetiredSites::default()
            }
        );
        let mut missing = BTreeSet::from([PathBuf::from("fixture/root.rs")]);
        assert_eq!(
            module_census(
                &mut syntax,
                Path::new("fixture"),
                &mut missing,
                &mut |_, _| Err(CensusRefusal::ModuleCoverage),
                FixtureOwner::None
            ),
            Err(CensusRefusal::ModuleCoverage)
        );
        let mut unit_data: syn::File = syn::parse_quote! {
            #[cfg(test)] mod tests {
                fn negative_fixture() { json!({"native_cache_heap_candidate":{}, "kind":"conditional_native_schema_cache_heap_candidate"}); }
                fn assert_selected_cache_heap() {}
            }
        };
        assert_eq!(
            census_with(&mut unit_data, FixtureOwner::None),
            RetiredSites {
                declarations: BTreeSet::from(["assert_selected_cache_heap".to_owned()]),
                fields: BTreeSet::from(["native_cache_heap_candidate".to_owned()]),
                kinds: BTreeSet::from(
                    ["conditional_native_schema_cache_heap_candidate".to_owned()]
                ),
                ..RetiredSites::default()
            }
        );
    }

    fn expected_retired_writers() -> RetiredSites {
        RetiredSites {
            fields: BTreeSet::from(["native_cache_heap_candidate".to_owned()]),
            kinds: BTreeSet::from(["conditional_native_schema_cache_heap_candidate".to_owned()]),
            ..RetiredSites::default()
        }
    }

    fn assert_fixture_owner_controls() {
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/schema_validation");
        assert_eq!(
            fixture_owner(&directory.join("report.rs")),
            FixtureOwner::Report
        );
        assert_eq!(
            fixture_owner(&directory.join("admission.rs")),
            FixtureOwner::Admission
        );
        assert_eq!(
            fixture_owner(&directory.join("observations.rs")),
            FixtureOwner::None
        );
        assert_eq!(
            fixture_owner(&directory.join("observations/report.rs")),
            FixtureOwner::None
        );
        let mut report: syn::File = syn::parse_quote! {
            #[cfg(test)] mod tests {
                fn assert_report_mutants(current: &Value) {
                    json!({"native_cache_heap_candidate":{}, "kind":"conditional_native_schema_cache_heap_candidate"});
                }
            }
        };
        assert_eq!(
            census_with(&mut report, FixtureOwner::Report),
            RetiredSites::default()
        );
        assert_eq!(
            census_with(&mut report, FixtureOwner::None),
            expected_retired_writers()
        );
        let mut admission: syn::File = syn::parse_quote! {
            #[cfg(test)] mod tests {
                #[test] fn retired_candidate_cannot_supply_costs() {
                    json!({"native_cache_heap_candidate":{}, "kind":"conditional_native_schema_cache_heap_candidate"});
                }
            }
        };
        assert_eq!(
            census_with(&mut admission, FixtureOwner::Admission),
            RetiredSites::default()
        );
        assert_eq!(
            census_with(&mut admission, FixtureOwner::Report),
            expected_retired_writers()
        );
        let mut symbols: syn::File = syn::parse_quote! {
            #[cfg(test)] mod tests {
                fn assert_report_mutants(current: &Value) {
                    let f = selected_cache_bundle;
                    assert_selected_cache_heap();
                    json!({"native_cache_heap_candidate":{}, "kind":"conditional_native_schema_cache_heap_candidate"});
                }
            }
        };
        assert_eq!(
            census_with(&mut symbols, FixtureOwner::Report),
            RetiredSites {
                calls: BTreeSet::from(["assert_selected_cache_heap".to_owned()]),
                references: BTreeSet::from([
                    "selected_cache_bundle".to_owned(),
                    "assert_selected_cache_heap".to_owned()
                ]),
                ..RetiredSites::default()
            }
        );
    }

    fn assert_writer_scope_refusals() {
        let mut observations: syn::File = syn::parse_quote! {
            #[cfg(test)] mod tests {
                pub(super) fn renamed_active_writer() {
                    json!({"native_cache_heap_candidate":{}, "kind":"conditional_native_schema_cache_heap_candidate"});
                }
                pub(super) use self::renamed_active_writer as exported_writer;
            }
        };
        assert_eq!(
            census_with(&mut observations, FixtureOwner::None),
            expected_retired_writers()
        );
        let examples: [syn::File; 4] = [
            syn::parse_quote! {
                #[cfg(test)] mod tests {
                    pub(super) fn assert_report_mutants(current: &Value) {
                        json!({"native_cache_heap_candidate":{}, "kind":"conditional_native_schema_cache_heap_candidate"});
                    }
                    pub(super) use self::assert_report_mutants as exported_writer;
                }
            },
            syn::parse_quote! {
                #[cfg(test)] mod tests { mod namespace {
                    fn assert_report_mutants(current: &Value) {
                        json!({"native_cache_heap_candidate":{}, "kind":"conditional_native_schema_cache_heap_candidate"});
                    }
                }}
            },
            syn::parse_quote! {
                #[cfg(test)] pub(super) mod tests {
                    fn assert_report_mutants(current: &Value) {
                        json!({"native_cache_heap_candidate":{}, "kind":"conditional_native_schema_cache_heap_candidate"});
                    }
                }
            },
            syn::parse_quote! {
                #[cfg(test)] mod tests {
                    fn assert_report_mutants(current: &Value) -> Value {
                        json!({"native_cache_heap_candidate":{}, "kind":"conditional_native_schema_cache_heap_candidate"})
                    }
                }
            },
        ];
        for mut syntax in examples {
            assert_eq!(
                census_with(&mut syntax, FixtureOwner::Report),
                expected_retired_writers()
            );
        }
    }

    fn assert_census_controls() {
        assert_eq!(
            census(&mut syn::parse_quote! {
                fn retained_component(report: &Value) { let _read = &report["native_cache_heap_candidate"]; }
            }),
            RetiredSites::default()
        );
        assert_eq!(
            census(&mut syn::parse_quote! { fn selected_cache_bundle() {} }),
            RetiredSites {
                declarations: BTreeSet::from(["selected_cache_bundle".to_owned()]),
                ..RetiredSites::default()
            }
        );
        assert_eq!(
            census(&mut syn::parse_quote! { fn caller() { assert_selected_cache_heap(); } }),
            RetiredSites {
                calls: BTreeSet::from(["assert_selected_cache_heap".to_owned()]),
                references: BTreeSet::from(["assert_selected_cache_heap".to_owned()]),
                ..RetiredSites::default()
            }
        );
        assert_eq!(
            census(&mut syn::parse_quote! {
                fn reference() { let f = selected_cache_bundle; let borrowed = &assert_selected_cache_heap; }
            }),
            RetiredSites {
                references: BTreeSet::from([
                    "selected_cache_bundle".to_owned(),
                    "assert_selected_cache_heap".to_owned()
                ]),
                ..RetiredSites::default()
            }
        );
        assert_eq!(
            census(
                &mut syn::parse_quote! { fn writer(mut report: Value) { report["native_cache_heap_candidate"] = Value::Null; } }
            ),
            RetiredSites {
                fields: BTreeSet::from(["native_cache_heap_candidate".to_owned()]),
                ..RetiredSites::default()
            }
        );
        assert_eq!(
            census(
                &mut syn::parse_quote! { fn writer() { json!({"native_cache_heap_candidate":{}}); } }
            ),
            RetiredSites {
                fields: BTreeSet::from(["native_cache_heap_candidate".to_owned()]),
                ..RetiredSites::default()
            }
        );
        assert_eq!(
            census(
                &mut syn::parse_quote! { fn writer() { json!({"kind":"conditional_native_schema_cache_heap_candidate"}); } }
            ),
            RetiredSites {
                kinds: BTreeSet::from(
                    ["conditional_native_schema_cache_heap_candidate".to_owned()]
                ),
                ..RetiredSites::default()
            }
        );
    }

    fn assert_retired_shape_controls() {
        assert_eq!(
            census(&mut syn::parse_quote! { fn check_current_report_fields() {} }),
            RetiredSites {
                declarations: BTreeSet::from(["check_current_report_fields".to_owned()]),
                ..RetiredSites::default()
            }
        );
        assert_eq!(
            census(&mut syn::parse_quote! {
                fn caller() { let checker = check_current_report_fields; check_current_report_fields(); }
            }),
            RetiredSites {
                calls: BTreeSet::from(["check_current_report_fields".to_owned()]),
                references: BTreeSet::from(["check_current_report_fields".to_owned()]),
                ..RetiredSites::default()
            }
        );
        let expected = RetiredSites {
            kinds: BTreeSet::from(["conditional_source_shape".to_owned()]),
            ..RetiredSites::default()
        };
        assert_eq!(
            census(&mut syn::parse_quote! {
                fn writer() { json!({"kind":"conditional_source_shape",
                    "numeric_policy_adequacy":"not_evaluated", "attempted_work_funding":"not_established"}); }
            }),
            expected
        );
        let mut negative: syn::File = syn::parse_quote! {
            #[cfg(test)] mod tests {
                #[test] fn retired_candidate_cannot_supply_costs() {
                    json!({"kind":"conditional_source_shape",
                        "numeric_policy_adequacy":"not_evaluated", "attempted_work_funding":"not_established"});
                }
            }
        };
        assert_eq!(
            census_with(&mut negative, FixtureOwner::Admission),
            RetiredSites::default()
        );
        assert_eq!(census_with(&mut negative, FixtureOwner::None), expected);
    }

    fn assert_indexed_kind_controls() {
        let examples: [(syn::File, &str); 4] = [
            (
                syn::parse_quote! {
                    fn writer(mut report: Value) { report["kind"] = json!("conditional_native_schema_cache_heap_candidate"); }
                },
                "conditional_native_schema_cache_heap_candidate",
            ),
            (
                syn::parse_quote! {
                    fn writer(mut report: Value) { report["kind"] = serde_json::json!("conditional_source_shape"); }
                },
                "conditional_source_shape",
            ),
            (
                syn::parse_quote! {
                    fn writer(mut report: StringFields) { report["kind"] = "conditional_native_schema_cache_heap_candidate"; }
                },
                "conditional_native_schema_cache_heap_candidate",
            ),
            (
                syn::parse_quote! {
                    fn writer(mut report: StringFields) { report["kind"] = "conditional_source_shape"; }
                },
                "conditional_source_shape",
            ),
        ];
        for (mut syntax, kind) in examples {
            assert_eq!(
                census(&mut syntax),
                RetiredSites {
                    kinds: BTreeSet::from([kind.to_owned()]),
                    ..RetiredSites::default()
                }
            );
        }
        assert_eq!(
            census(&mut syn::parse_quote! {
                fn writer(mut report: Value) {
                    report["kind"] = json!("schema_validation_resources");
                    report["message"] = json!("conditional_source_shape");
                }
            }),
            RetiredSites::default()
        );
    }

    #[test]
    fn retired_assembly_has_no_active_callers() {
        assert_census_controls();
        assert_module_controls();
        assert_fixture_owner_controls();
        assert_writer_scope_refusals();
        assert_retired_shape_controls();
        assert_indexed_kind_controls();
        let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
        let root = directory.join("schema_source_conformance.rs");
        let bytes = read_regular(&root).expect("existing bounded ordinary-source reader");
        let mut actual = syn::parse_file(from_utf8(&bytes).unwrap()).expect("ordinary root source");
        let mut seen = BTreeSet::from([root]);
        let retired = module_census(
            &mut actual,
            &directory,
            &mut seen,
            &mut ordinary_source,
            FixtureOwner::None,
        )
        .expect("complete ordinary module graph, never an omitted unreadable owner");
        for required in [
            "schema_validation/report.rs",
            "schema_validation/storage/mod.rs",
            "schema_validation/storage/source.rs",
        ] {
            assert!(
                seen.contains(&directory.join(required)),
                "required active report/storage owner: {required}"
            );
        }
        assert_eq!(retired, RetiredSites::default());
    }

    #[test]
    fn unqualified_source_never_grants_serving_permission() {
        with_case(|seed, view, bodies, census, coverage| {
            let result = describe_validation(seed, view, bodies, census, coverage);
            assert_eq!(
                result.as_ref().map(|report| &report.document),
                Ok(&expected_document(view, census))
            );
            let report = result.unwrap();
            assert_eq!(qualification_refusal(&report), Refusal::IncompleteSource);
            for field in [
                "resource_qualification",
                "physical_fit",
                "serving_permission",
            ] {
                assert_eq!(report.document[field], false);
            }
            let mut forged = report.document.clone();
            forged["status"] = json!("CommonSourceComplete");
            assert_eq!(
                check_current_report(&forged),
                Err(Refusal::NonCanonicalField)
            );
            let mut hypothetical = report.document.clone();
            assert!(hypothetical.get("supplied_target_facts").is_none());
            hypothetical["supplied_target_facts"] =
                json!({"kind":"hypothetical", "requested_bytes":7_u64});
            assert_eq!(
                check_current_report(&hypothetical),
                Err(Refusal::NonCanonicalField)
            );
            let native_observation =
                json!({"kind":"native_requested_heap_observation", "peak_requested_bytes":7_u64});
            assert_eq!(
                check_current_report(&native_observation),
                Err(Refusal::NonCanonicalField)
            );
            let mut missing_source = report.document.clone();
            let children = mem::take(
                missing_source["coverage"]["children"]
                    .as_array_mut()
                    .unwrap(),
            );
            assert!(!children.is_empty());
            assert_eq!(
                check_current_report(&missing_source),
                Err(Refusal::ChildCoverage)
            );
        });
    }
}
