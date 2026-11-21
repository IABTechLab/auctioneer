//! Expansion consumes accepted build-local inputs, not bidder-source paths.

use crate::schema_build::application::CapturedSeed;
use crate::schema_cache_model::emitted_patterns;
use auctioneer_config::schema_staging_build::SchemaSnapshot;
use quote::ToTokens as _;
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

const MEMBERSHIP: &str = "captured declaration membership mismatch";

fn manifest(seed: &CapturedSeed) -> Result<Value, String> {
    let value: Value = serde_json::from_slice(&seed.manifest_bytes)
        .map_err(|_error| "invalid captured staging manifest".to_owned())?;
    if value["schema_version"] != 2_u8
        || value["kind"] != "unqualified_non_serving_schema_staging"
        || value["linked_or_serving_qualification"] != false
        || value["owner_graph_status"] != "unexpanded"
        || value["identity"]["qualification"] != "Unqualified"
    {
        return Err("invalid captured staging manifest".into());
    }
    if value["identity"]["sha256"] != seed.identity_sha256 {
        return Err("stale captured staging identity".into());
    }
    if value["emitted_rust_sha256"] != digest(&seed.rust) {
        return Err("stale captured staging Rust".into());
    }
    seed.recheck_snapshots()
        .map_err(|_error| "captured schema snapshot mismatch".to_owned())?;
    Ok(value)
}

fn declarations<'a>(
    manifest: &'a Value,
    seed: &'a CapturedSeed,
) -> Result<BTreeMap<String, (&'a Value, &'a SchemaSnapshot)>, String> {
    let snapshots: BTreeMap<_, _> = seed
        .snapshots
        .iter()
        .map(|row| (row.owner.as_str(), row))
        .collect();
    if snapshots.len() != seed.snapshots.len() {
        return Err(MEMBERSHIP.into());
    }
    let rows = manifest["declarations"].as_array().ok_or(MEMBERSHIP)?;
    let mut markers = BTreeMap::new();
    let mut owners = BTreeSet::new();
    for row in rows {
        let marker = row["marker"].as_str().ok_or(MEMBERSHIP)?;
        let owner = row["owner"].as_str().ok_or(MEMBERSHIP)?;
        let snapshot = *snapshots.get(owner).ok_or(MEMBERSHIP)?;
        if marker.is_empty()
            || owner.is_empty()
            || !owners.insert(owner)
            || serde_json::to_value(snapshot).map_err(|error| error.to_string())? != row["snapshot"]
            || row["source"]["name"] != owner
            || row["source"]["sha256"] != snapshot.sha256
            || row["source"]["bytes"].as_u64() != u64::try_from(snapshot.bytes.len()).ok()
            || markers.insert(marker.to_owned(), (row, snapshot)).is_some()
        {
            return Err(MEMBERSHIP.into());
        }
    }
    if markers.len() != snapshots.len() {
        return Err(MEMBERSHIP.into());
    }
    Ok(markers)
}

fn validator(item: &syn::ItemStruct) -> Result<Option<&syn::Attribute>, String> {
    let attributes = item
        .attrs
        .iter()
        .filter(|attribute| {
            attribute
                .path()
                .segments
                .iter()
                .map(|part| part.ident.to_string())
                .eq(["jsonschema", "validator"])
        })
        .collect::<Vec<_>>();
    match attributes.as_slice() {
        [] => Ok(None),
        [attribute] => Ok(Some(attribute)),
        _ => Err("unqualified captured validator options".into()),
    }
}

fn expand_declaration(
    seed: &CapturedSeed,
    item: &syn::ItemStruct,
    attribute: &syn::Attribute,
    row: &Value,
    snapshot: &SchemaSnapshot,
) -> Result<Value, String> {
    let syn::Meta::List(options) = &attribute.meta else {
        return Err("unqualified captured validator options".into());
    };
    let schema: Value =
        serde_json::from_slice(&snapshot.bytes).map_err(|error| error.to_string())?;
    if digest(&snapshot.bytes) != snapshot.sha256 {
        return Err("captured schema snapshot mismatch".into());
    }
    let path = seed.out_dir.join(&snapshot.relative_path);
    let callbacks = super::options::check(options.tokens.clone(), &path, &schema)?;
    let mut declaration = item.clone();
    // Remove only the invoking attribute, preserving all other emitted syntax.
    declaration.attrs = item
        .attrs
        .iter()
        .filter(|candidate| !std::ptr::eq(*candidate, attribute))
        .cloned()
        .collect();
    let tokens =
        jsonschema_macros_core::expand(options.tokens.clone(), declaration.to_token_stream());
    let expansion_sha256 = digest(tokens.to_string().as_bytes());
    let mut syntax: syn::File = syn::parse2(tokens).map_err(|error| error.to_string())?;
    crate::reject_compiler_errors(&mut syntax)?;
    let patterns = emitted_patterns(&syntax)
        .map_err(|error| format!("unsupported expanded regex owner: {error:?}"))?;
    let namespace = format!("{}::{}", snapshot.owner, item.ident);
    let storage = super::storage::collect(&mut syntax, &namespace, &path, &patterns)?;
    let programs = patterns
        .iter()
        .map(|(name, pattern)| {
            json!({
                "identity":format!("{namespace}::{name}"),"symbol":name,
                "pattern":pattern,"pattern_sha256":digest(pattern.as_bytes()),
                "lifetime":"first-initialization-to-process-exit",
                "search_cache_lifetime":"validation-call","sharing":"declaration-local"
            })
        })
        .collect::<Vec<_>>();
    Ok(
        json!({"owner":snapshot.owner,"marker":item.ident.to_string(),
        "source":row["source"],"snapshot":row["snapshot"],
        "expansion_sha256":expansion_sha256,"programs":programs,
        "schema_state":storage.schema_state,"readonly":storage.readonly,
        "initialization":storage.initialization,"callbacks":callbacks,
        "qualification":"Unqualified","resource_qualification":false}),
    )
}

pub(crate) fn expand(seed: &CapturedSeed) -> Result<Value, String> {
    let manifest = manifest(seed)?;
    let expected = declarations(&manifest, seed)?;
    let syntax =
        syn::parse_file(std::str::from_utf8(&seed.rust).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let mut actual = BTreeMap::new();
    let mut auxiliary = Vec::new();
    for item in &syntax.items {
        if let syn::Item::Struct(declaration) = item
            && let Some(attribute) = validator(declaration)?
        {
            if actual
                .insert(declaration.ident.to_string(), (declaration, attribute))
                .is_some()
            {
                return Err(MEMBERSHIP.into());
            }
        } else if matches!(
            item,
            syn::Item::Use(_)
                | syn::Item::Struct(_)
                | syn::Item::Static(_)
                | syn::Item::Const(_)
                | syn::Item::Fn(_)
        ) {
            auxiliary.push(item);
        } else {
            return Err("unsupported captured staging syntax".into());
        }
    }
    if actual.keys().ne(expected.keys()) {
        return Err(MEMBERSHIP.into());
    }
    let bindings = expected
        .iter()
        .map(|(marker, (_, snapshot))| (marker.as_str(), snapshot.owner.as_str()))
        .collect::<Vec<_>>();
    let classified = super::auxiliary::classify(
        &auxiliary,
        &bindings,
        &seed.identity_sha256,
        &manifest["selection"],
    )?;
    let auxiliary_hashes = auxiliary
        .iter()
        .map(|item| digest(item.to_token_stream().to_string().as_bytes()))
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for (marker, (row, snapshot)) in expected {
        let (item, attribute) = actual.get(&marker).ok_or(MEMBERSHIP)?;
        rows.push(expand_declaration(seed, item, attribute, row, snapshot)?);
    }
    seed.recheck_snapshots()
        .map_err(|_error| "captured schema snapshot mismatch".to_owned())?;
    Ok(
        json!({"schema_version":2_u8,"identity_sha256":seed.identity_sha256,
        "captured_manifest_sha256":digest(&seed.manifest_bytes),
        "captured_rust_sha256":digest(&seed.rust),"declarations":rows,
        "auxiliary_syntax_sha256":auxiliary_hashes,
        "auxiliary":{"status":classified.status.as_str(),"roots":classified.roots,
            "dispatch_edges":classified.dispatch_edges},"qualification":"Unqualified",
        "resource_qualification":false,"source_completeness":"owner-graph-only"}),
    )
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
        for (index, (owner, marker)) in [("fake_a", quote!(FakeA)), ("fake_b", quote!(FakeB))]
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
        let dfa_limit = syn::LitInt::new(
            if field == "dfa_limit" {
                "65535"
            } else {
                "65536"
            },
            proc_macro2::Span::call_site(),
        );
        let validate_formats = field != "format_validation";
        let ignore_unknown_formats = field == "ignore_unknown_formats";
        let is_valid = field != "is_valid";
        let errors = field == "methods";
        let iter_errors = field == "iter_errors";
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
                    ignore_unknown_formats = #ignore_unknown_formats,
                    formats = { "hostname" => #callback },
                    pattern_options = { engine = #engine, cache = #cache,
                        size_limit = #limit, dfa_size_limit = #dfa_limit },
                    methods = { is_valid = #is_valid, validate = #errors, iter_errors = #iter_errors }
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
        assert_eq!(graph["auxiliary"]["status"], "Absent");
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
                    fs::write(
                        seed.out_dir.join(&seed.snapshots[0].relative_path),
                        b"false",
                    )
                    .unwrap();
                }
                "mapping" => change_manifest(&mut seed, |value| {
                    value["declarations"].as_array_mut().unwrap().pop().unwrap();
                }),
                _ => panic!("unknown stale-input matrix case"),
            }
            assert_eq!(expand(&seed).err().as_deref(), Some(reason), "{field}");
        }
    }

    #[test]
    fn fake_expansion_checks_semantic_options_independently_of_consistent_hashes() {
        for field in [
            "draft",
            "engine",
            "cache",
            "limit",
            "dfa_limit",
            "format_validation",
            "ignore_unknown_formats",
            "is_valid",
            "methods",
            "iter_errors",
            "callback",
            "unknown",
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
                3 => syntax.items.push(syn::parse_quote!(
                    mod fake_external;
                )),
                _ => panic!("unknown declaration matrix case"),
            });
            let reason = match case {
                0 | 1 => "captured declaration membership mismatch",
                2 => "unqualified captured validator options",
                3 => "unsupported captured staging syntax",
                _ => panic!("unknown declaration matrix case"),
            };
            assert_eq!(expand(&seed).err().as_deref(), Some(reason), "{case}");
        }
    }

    #[test]
    fn fake_expansion_refuses_unclassified_auxiliary_storage() {
        for extra in [
            quote! { static FAKE_HIDDEN: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| vec![0_u8; 17]); },
            quote! { fn fake_hidden() { static FAKE_HIDDEN: std::sync::LazyLock<Vec<u8>> = std::sync::LazyLock::new(|| vec![0_u8; 17]); } },
            quote! { const FAKE_UNCLASSIFIED: &str = "unattributed"; },
        ] {
            let (_directory, mut seed) = fixture("^a[0-9]+$");
            change_rust(&mut seed, |syntax| {
                syntax.items.push(syn::parse2(extra).unwrap())
            });
            assert_eq!(
                expand(&seed).err().as_deref(),
                Some("unsupported captured staging syntax")
            );
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
