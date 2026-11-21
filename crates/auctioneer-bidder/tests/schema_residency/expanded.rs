//! Expansion consumes accepted build-local inputs, not bidder-source paths.

use crate::schema_build::application::CapturedSeed;
use serde_json::Value;

pub(crate) fn expand(_seed: &CapturedSeed) -> Result<Value, String> {
    Err("schema seed expansion not implemented".into())
}

#[cfg(test)]
mod tests {
    use super::expand;
    use crate::schema_build::application::CapturedSeed;
    use auctioneer_config::schema_staging_build::plan_snapshot;
    use quote::{ToTokens as _, quote};
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::{collections::BTreeSet, fs};

    fn digest(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn fixture(pattern: &str) -> (tempfile::TempDir, CapturedSeed) {
        let directory = tempfile::tempdir().unwrap();
        let out_dir = directory.path().canonicalize().unwrap();
        let mut snapshots = Vec::new();
        let mut declarations = Vec::new();
        let mut rust = proc_macro2::TokenStream::new();
        for (index, (owner, marker)) in
            [("fake_a", quote!(FakeA)), ("fake_b", quote!(FakeB))]
                .into_iter()
                .enumerate()
        {
            let bytes = serde_json::to_vec(&json!({
                "$schema":"http://json-schema.org/draft-04/schema#",
                "type":"object","properties":{
                    "code":{"type":"string","pattern":pattern},
                    "host":{"type":"string","format":"hostname"},
                    "choice":{"enum":[1.5_f64,{"nested":["value"]}]}
                }
            }))
            .unwrap();
            let snapshot = plan_snapshot(owner, index, bytes).unwrap();
            let path = out_dir.join(&snapshot.relative_path);
            fs::write(&path, &snapshot.bytes).unwrap();
            let literal = syn::LitStr::new(path.to_str().unwrap(), proc_macro2::Span::call_site());
            rust.extend(quote! {
                #[jsonschema::validator(
                    path = #literal, draft = Draft4, validate_formats = true,
                    ignore_unknown_formats = false,
                    formats = { "hostname" => crate::schema::staging::formats::hostname },
                    pattern_options = { engine = regex, cache = explicit,
                        size_limit = 65_536, dfa_size_limit = 65_536 },
                    methods = { is_valid = true, validate = false, iter_errors = false }
                )]
                struct #marker;
            });
            declarations.push(json!({"owner":owner,"marker":marker.to_string(),
                "snapshot":snapshot,"source":{"name":owner,
                    "path":format!("{owner}/parameters.json"),
                    "source_path":format!("static/bidder-params/{owner}.json"),
                    "source_blob":"a".repeat(40),"bytes":snapshot.bytes.len(),
                    "sha256":snapshot.sha256,"repository":"https://fake.invalid/source",
                    "revision":"b".repeat(40)}}));
            snapshots.push(snapshot);
        }
        let rust = rust.to_string().into_bytes();
        let identity_sha256 = digest(b"fake accepted artifact identity");
        let manifest = json!({"schema_version":2_u8,
            "kind":"unqualified_non_serving_schema_staging",
            "identity":{"sha256":identity_sha256,"qualification":"Unqualified"},
            "linked_or_serving_qualification":false,"owner_graph_status":"unexpanded",
            "emitted_rust_sha256":digest(&rust),"declarations":declarations});
        let seed = CapturedSeed {
            identity_sha256,
            manifest_bytes: serde_json::to_vec(&manifest).unwrap(),
            rust,
            build_root: out_dir.clone(),
            out_dir,
            snapshots,
        };
        (directory, seed)
    }

    fn change_manifest(seed: &mut CapturedSeed, update: impl FnOnce(&mut Value)) {
        let mut manifest: Value = serde_json::from_slice(&seed.manifest_bytes).unwrap();
        update(&mut manifest);
        seed.manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    }

    fn change_rust(seed: &mut CapturedSeed, update: impl FnOnce(&mut syn::File)) {
        let mut syntax = syn::parse_file(std::str::from_utf8(&seed.rust).unwrap()).unwrap();
        update(&mut syntax);
        seed.rust = syntax.into_token_stream().to_string().into_bytes();
        let sha256 = digest(&seed.rust);
        change_manifest(seed, |value| value["emitted_rust_sha256"] = json!(sha256));
    }

    fn change_options(seed: &mut CapturedSeed, field: &str) {
        let path = seed.out_dir.join(&seed.snapshots[0].relative_path);
        let path = syn::LitStr::new(path.to_str().unwrap(), proc_macro2::Span::call_site());
        let draft = if field == "draft" {
            quote!(Draft202012)
        } else {
            quote!(Draft4)
        };
        let engine = if field == "engine" {
            quote!(fancy_regex)
        } else {
            quote!(regex)
        };
        let cache = if field == "cache" {
            quote!(implicit)
        } else {
            quote!(explicit)
        };
        let limit = syn::LitInt::new(
            if field == "limit" { "65535" } else { "65536" },
            proc_macro2::Span::call_site(),
        );
        let validate_formats = field != "format_validation";
        let errors = field == "methods";
        let callback = if field == "callback" {
            quote!(crate::schema::staging::formats::fake_hostname)
        } else {
            quote!(crate::schema::staging::formats::hostname)
        };
        let extra = if field == "unknown" {
            quote!(, fake_option = true)
        } else {
            quote!()
        };
        change_rust(seed, |syntax| {
            let syn::Item::Struct(marker) = &mut syntax.items[0] else {
                panic!("fixture marker");
            };
            marker.attrs[0] = syn::parse_quote! {
                #[jsonschema::validator(
                    path = #path, draft = #draft, validate_formats = #validate_formats,
                    ignore_unknown_formats = false,
                    formats = { "hostname" => #callback },
                    pattern_options = { engine = #engine, cache = #cache,
                        size_limit = #limit, dfa_size_limit = 65_536 },
                    methods = { is_valid = true, validate = #errors, iter_errors = false }
                    #extra
                )]
            };
        });
    }

    #[test]
    fn fake_expansion_keeps_independent_namespaces_for_equal_pattern_owners() {
        let (_directory, seed) = fixture("^a[0-9]+$");
        let graph = expand(&seed).expect("expand actual captured validator attributes");
        assert_eq!(graph["identity_sha256"], seed.identity_sha256);
        assert_eq!(graph["qualification"], "Unqualified");
        assert_eq!(graph["resource_qualification"], false);
        let rows = graph["declarations"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter()
                .map(|row| row["owner"].as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["fake_a", "fake_b"])
        );
        let programs: Vec<_> = rows
            .iter()
            .flat_map(|row| row["programs"].as_array().unwrap())
            .collect();
        assert_eq!(programs.len(), 2);
        assert_ne!(programs[0]["identity"], programs[1]["identity"]);
        assert_eq!(programs[0]["pattern"], programs[1]["pattern"]);
        assert!(
            rows.iter()
                .all(|row| !row["expansion_sha256"].as_str().unwrap().is_empty())
        );
        for row in rows {
            let state = row["schema_state"].as_array().unwrap();
            assert_eq!(state.len(), 2);
            assert!(state.iter().all(|owner| owner["kind"] == "lazy_json_array"));
            let values = state
                .iter()
                .map(|owner| serde_json::to_string(&owner["value"]).unwrap())
                .collect::<BTreeSet<_>>();
            assert_eq!(
                values,
                BTreeSet::from([
                    serde_json::to_string(&json!([1.5_f64])).unwrap(),
                    serde_json::to_string(&json!([{"nested":["value"]}])).unwrap(),
                ])
            );
        }
    }

    #[test]
    fn fake_expansion_refuses_each_stale_input_before_compiler_expansion() {
        for (field, reason) in [
            ("identity", "stale captured staging identity"),
            ("rust", "stale captured staging Rust"),
            ("snapshot", "captured schema snapshot mismatch"),
            ("mapping", "captured declaration membership mismatch"),
        ] {
            let (_directory, mut seed) = fixture("^a[0-9]+$");
            match field {
                "identity" => seed.identity_sha256 = digest(b"different artifact"),
                "rust" => seed.rust.push(b' '),
                "snapshot" => {
                    fs::write(seed.out_dir.join(&seed.snapshots[0].relative_path), b"false").unwrap();
                }
                "mapping" => change_manifest(&mut seed, |value| {
                    value["declarations"].as_array_mut().unwrap().pop().unwrap();
                }),
                _ => unreachable!("fixed stale-input matrix"),
            }
            assert_eq!(expand(&seed).err().as_deref(), Some(reason), "{field}");
        }
    }

    #[test]
    fn fake_expansion_checks_semantic_options_independently_of_consistent_hashes() {
        for field in [
            "draft", "engine", "cache", "limit", "format_validation", "methods", "callback", "unknown",
        ] {
            let (_directory, mut seed) = fixture("^a[0-9]+$");
            change_options(&mut seed, field);
            assert_eq!(
                expand(&seed).err().as_deref(),
                Some("unqualified captured validator options"),
                "{field}"
            );
        }
    }

    #[test]
    fn fake_expansion_rejects_missing_duplicate_and_unknown_declarations() {
        for case in 0_u8..4 {
            let (_directory, mut seed) = fixture("^a[0-9]+$");
            change_rust(&mut seed, |syntax| match case {
                0 => {
                    syntax.items.remove(0);
                }
                1 => syntax.items.push(syntax.items[0].clone()),
                2 => {
                    let syn::Item::Struct(marker) = &mut syntax.items[0] else {
                        panic!("fixture marker");
                    };
                    marker.attrs.push(marker.attrs[0].clone());
                }
                3 => syntax.items.push(syn::parse_quote!(mod fake_external;)),
                _ => unreachable!("fixed declaration matrix"),
            });
            let reason = match case {
                0 | 1 => "captured declaration membership mismatch",
                2 => "unqualified captured validator options",
                3 => "unsupported captured staging syntax",
                _ => unreachable!("fixed declaration matrix"),
            };
            assert_eq!(expand(&seed).err().as_deref(), Some(reason), "{case}");
        }
    }

    #[test]
    fn fake_expansion_preserves_escaped_literal_semantics_in_patterns() {
        let (_quoted_directory, quoted) = fixture("a\"b");
        let (_plain_directory, plain) = fixture("ab");
        let quoted_graph = expand(&quoted).unwrap();
        let plain_graph = expand(&plain).unwrap();
        let quoted_program = &quoted_graph["declarations"][0]["programs"][0];
        let plain_program = &plain_graph["declarations"][0]["programs"][0];
        assert_eq!(quoted_program["pattern"], "a\"b");
        assert_eq!(quoted_program["pattern_sha256"], digest(b"a\"b"));
        assert_eq!(plain_program["pattern"], "ab");
        assert_ne!(quoted_program["pattern"], plain_program["pattern"]);
        assert_ne!(
            quoted_program["pattern_sha256"],
            plain_program["pattern_sha256"]
        );
    }
}
