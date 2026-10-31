use super::fixtures::{Flow, config, flows, input};
use super::harness::{Answer, Secrets, assert_disabled, assert_success, boot, send};
use edgezero_core::app::App;
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc};

fn codes(answer: &Answer, category: &str, name: &str) -> Vec<u64> {
    answer.wire["ext"][category][name]
        .as_array()
        .unwrap_or_else(|| panic!("missing {category} attribution for {name}: {}", answer.wire))
        .iter()
        .map(|row| row["code"].as_u64().unwrap())
        .collect()
}

fn assert_no_errors(answer: &Answer) {
    match &answer.wire["ext"]["errors"] {
        Value::Null => {}
        Value::Object(errors) => assert!(
            errors
                .values()
                .all(|rows| rows.as_array().is_some_and(Vec::is_empty)),
            "{}",
            answer.wire
        ),
        Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => {
            panic!("malformed production error projection: {}", answer.wire);
        }
    }
}

fn dispatch(app: &App, requested: &[&Flow], runtime: &Value) -> (Answer, Arc<Secrets>) {
    let secrets = Secrets::new(requested, false);
    (
        send(app, requested, &input(requested), runtime, &secrets),
        secrets,
    )
}

fn reject_parameters(request: &mut Value, name: &str) {
    for imp in request["imp"].as_array_mut().unwrap() {
        let bidders = imp
            .pointer_mut("/ext/prebid/bidder")
            .unwrap()
            .as_object_mut()
            .unwrap();
        if let Some(params) = bidders.get_mut(name) {
            // Every owner schema requires an object. Do not invent malformed
            // owner-specific keys or alter any valid sibling's parameters.
            *params = json!([]);
        }
    }
}

fn fresh_activation_snapshots(app: &App, cohort: &[Flow]) {
    let mut dispatched = BTreeSet::new();
    let mut gzip_calls = 0_usize;
    let mut correlated_calls = 0_usize;
    for flow in cohort {
        let requested = [flow];
        let enabled = config(&requested);
        let mut defaults = enabled.clone();
        for overrides in defaults["bidders"].as_object_mut().unwrap().values_mut() {
            overrides.as_object_mut().unwrap().remove("enabled");
        }
        let (default_answer, default_secrets) = dispatch(app, &requested, &defaults);
        if flow.default_active() {
            assert_success(&default_answer, &requested);
            assert_no_errors(&default_answer);
        } else {
            assert_disabled(&default_answer, flow);
            assert!(
                default_secrets.reads.lock().unwrap().is_empty(),
                "inactive bidder cannot poll host services"
            );
        }

        // The exact same generated App and its process registry serve each
        // new integrity-enveloped config snapshot. No test registry is built.
        for reenabled in [false, true] {
            let (answer, secrets) = dispatch(app, &requested, &enabled);
            assert_success(&answer, &requested);
            assert_no_errors(&answer);
            let expected_reads: Vec<_> = flow
                .credential()
                .map(|(reference, _)| reference.to_owned())
                .into_iter()
                .collect();
            assert_eq!(*secrets.reads.lock().unwrap(), expected_reads);
            assert!(
                answer.received > 0,
                "{} needs actual calls and replies",
                flow.name()
            );
            dispatched.extend(answer.calls.iter().map(|call| call.owner.clone()));
            if !reenabled {
                gzip_calls = gzip_calls
                    .checked_add(answer.calls.iter().filter(|call| call.gzip).count())
                    .unwrap();
                correlated_calls = correlated_calls
                    .checked_add(
                        answer
                            .calls
                            .iter()
                            .filter(|call| call.correlation.iter().any(Option::is_some))
                            .count(),
                    )
                    .unwrap();
                let mut disabled = enabled.clone();
                disabled["bidders"][flow.name()]["enabled"] = json!(false);
                let (disabled_answer, disabled_secrets) = dispatch(app, &requested, &disabled);
                assert_disabled(&disabled_answer, flow);
                assert!(disabled_secrets.reads.lock().unwrap().is_empty());
            }
        }
    }
    assert_eq!(
        dispatched,
        cohort.iter().map(|flow| flow.name().to_owned()).collect()
    );
    assert_eq!(
        dispatched.len(),
        14,
        "neither source nor supplemental priority identity may be skipped"
    );
    assert!(
        gzip_calls > 0,
        "gzip must be exercised at the actual finalized-call boundary"
    );
    assert!(
        correlated_calls > 0,
        "real owner correlation must reach the actual fanout"
    );
}

fn compiled_ancestor_veto(app: &App, cohort: &[Flow]) {
    let aliases: Vec<_> = cohort
        .iter()
        .filter(|flow| flow.ancestry.len() > 1)
        .collect();
    assert!(
        !aliases.is_empty(),
        "priority witness includes compiled alias ancestry"
    );
    for flow in aliases {
        let requested = [flow];
        let enabled = config(&requested);
        for explicitly_disabled in [false, true] {
            let mut vetoed = enabled.clone();
            for ancestor in flow.ancestry.iter().skip(1) {
                let overrides = vetoed["bidders"][&ancestor.name].as_object_mut().unwrap();
                if explicitly_disabled {
                    overrides.insert("enabled".into(), json!(false));
                } else {
                    overrides.remove("enabled");
                }
            }
            assert_eq!(vetoed["bidders"][flow.name()]["enabled"], true);
            let (answer, secrets) = dispatch(app, &requested, &vetoed);
            assert_disabled(&answer, flow);
            assert!(secrets.reads.lock().unwrap().is_empty());
        }
        let (answer, _) = dispatch(app, &requested, &enabled);
        assert_success(&answer, &requested);
        assert_no_errors(&answer);
    }
}

fn invalid_parameters_preserve_a_real_sibling(app: &App, cohort: &[Flow]) {
    for target in cohort {
        let sibling = cohort
            .iter()
            .find(|candidate| {
                candidate.name() != target.name()
                    && candidate.credential().is_none()
                    && candidate.case.outbound.len() == 1
            })
            .unwrap();
        let requested = [target, sibling];
        let runtime = config(&requested);
        let mut request = input(&requested);
        reject_parameters(&mut request, target.name());
        let secrets = Secrets::new(&requested, false);
        let answer = send(app, &requested, &request, &runtime, &secrets);
        assert_success(&answer, &[sibling]);
        let refused = target.case.request["imp"].as_array().unwrap().len();
        // This target holds native scratch, so every available selected
        // schema is admitted here, including the existing legacy publication.
        let expected = vec![2; refused];
        assert_eq!(
            codes(&answer, "errors", target.name()),
            expected,
            "{}",
            answer.wire
        );
        assert!(
            secrets.reads.lock().unwrap().is_empty(),
            "invalid parameters cannot reach credential services"
        );
    }
}

fn settings_and_secret_refusals_preserve_siblings(app: &App, cohort: &[Flow]) {
    let credentialed: Vec<_> = cohort
        .iter()
        .filter(|flow| flow.credential().is_some())
        .collect();
    assert!(
        !credentialed.is_empty(),
        "real credentialed owner is mandatory for host-service acceptance"
    );
    for target in credentialed {
        let sibling = cohort
            .iter()
            .find(|candidate| {
                candidate.name() != target.name()
                    && candidate.credential().is_none()
                    && candidate.case.outbound.len() == 1
            })
            .unwrap();
        let requested = [target, sibling];
        let runtime = config(&requested);
        let request = input(&requested);
        let (reference, bundle) = target.credential().unwrap();

        let (healthy_answer, healthy_secrets) = dispatch(app, &requested, &runtime);
        assert_success(&healthy_answer, &requested);
        assert_no_errors(&healthy_answer);
        assert_eq!(*healthy_secrets.reads.lock().unwrap(), [reference]);
        let response_text = healthy_answer.wire.to_string();
        assert!(
            !response_text.contains(bundle["password"].as_str().unwrap()),
            "fake credential value must not reach the response"
        );

        let mut missing_settings = runtime.clone();
        for ancestor in &target.ancestry {
            missing_settings["bidders"][&ancestor.name]
                .as_object_mut()
                .unwrap()
                .remove("settings");
        }
        let settings_secrets = Secrets::new(&requested, false);
        let settings_answer = send(
            app,
            &requested,
            &request,
            &missing_settings,
            &settings_secrets,
        );
        assert_success(&settings_answer, &[sibling]);
        assert_eq!(codes(&settings_answer, "errors", target.name()), [5]);
        assert!(settings_secrets.reads.lock().unwrap().is_empty());

        let secrets = Secrets::new(&requested, true);
        let answer = send(app, &requested, &request, &runtime, &secrets);
        assert_success(&answer, &[sibling]);
        assert_eq!(codes(&answer, "errors", target.name()), [5]);
        assert_eq!(*secrets.reads.lock().unwrap(), [reference]);
    }
}

fn all_priority_privacy_denials_precede_schema_and_host_services(app: &App, cohort: &[Flow]) {
    let mut denied_names = BTreeSet::new();
    for target in cohort {
        let sibling = cohort
            .iter()
            .find(|candidate| {
                candidate.name() != target.name()
                    && candidate.credential().is_none()
                    && candidate.case.outbound.len() == 1
            })
            .unwrap();
        let requested = [target, sibling];
        let mut denied = config(&requested);
        denied["accounts"] = json!({(target.account()): {"privacy":{"allowactivities":{"fetchBids":{
            "default":true, "rules":[{"allow":false,"condition":{"componentName":[target.name()]}}]
        }}}}});
        let mut request = input(&requested);
        reject_parameters(&mut request, target.name());
        let secrets = Secrets::new(&requested, false);
        let answer = send(app, &requested, &request, &denied, &secrets);
        assert_success(&answer, &[sibling]);
        assert_no_errors(&answer);
        assert_eq!(codes(&answer, "warnings", target.name()), [10017]);
        assert!(
            secrets.reads.lock().unwrap().is_empty(),
            "privacy denial must precede schema/legacy parameter decoding and every credential read"
        );
        assert!(denied_names.insert(target.name().to_owned()));
    }
    assert_eq!(denied_names.len(), 14);
    assert_eq!(
        denied_names,
        cohort.iter().map(|flow| flow.name().to_owned()).collect()
    );
}

#[test]
fn all_frozen_native_flows_use_generated_app_dispatch_and_fresh_activation_snapshots() {
    let app = boot();
    let cohort = flows();
    assert_eq!(
        cohort.iter().filter(|flow| flow.native_admission).count(),
        13,
        "new priority flows use native schema admission; the published legacy flow is preserved"
    );
    assert_eq!(
        cohort.iter().filter(|flow| flow.default_active()).count(),
        1,
        "only the existing priority publication remains active by default"
    );
    fresh_activation_snapshots(&app, &cohort);
    compiled_ancestor_veto(&app, &cohort);
    invalid_parameters_preserve_a_real_sibling(&app, &cohort);
    settings_and_secret_refusals_preserve_siblings(&app, &cohort);
    all_priority_privacy_denials_precede_schema_and_host_services(&app, &cohort);
    super::privacy::all_priority_outbound_privacy(&app, &cohort);
    super::sync::applicable_templates_use_signed_generated_app_callbacks(&app, &cohort);
}
