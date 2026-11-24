//! Public-name/source-dispatch correspondence, not common V, costs or serving authority.

use super::handoff;
use crate::schema_build::application::CapturedSeed;
use crate::schema_residency::descriptor::ValidationView;
use crate::schema_selection_source::public_names::{Disposition, PublicName, PublicNameCensus};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    Binding,
    MissingName(String),
    UnexpectedName(String),
    UnexpectedRoute(String),
    DuplicateRoute(String),
    Route(String),
    Disposition(String),
}

fn check_public_dispositions(
    seed: &CapturedSeed,
    view: ValidationView<'_>,
    census: &PublicNameCensus,
) -> Result<(), Refusal> {
    view.recheck().map_err(|_error| Refusal::Binding)?;
    handoff::check_seed(seed, view).map_err(|_error| Refusal::Binding)?;
    let result = (|| {
        let graph = view.graph();
        let auxiliary = graph.get("auxiliary").ok_or(Refusal::Binding)?;
        if auxiliary.get("status").and_then(Value::as_str) != Some("Complete") {
            return Err(Refusal::Binding);
        }
        let members = view
            .selection()
            .get("members")
            .and_then(Value::as_array)
            .ok_or(Refusal::Binding)?;
        let routes = auxiliary
            .get("dispatch_edges")
            .and_then(Value::as_array)
            .ok_or(Refusal::Binding)?
            .iter()
            .filter(|row| row.get("from").and_then(Value::as_str) == Some("native_owner"))
            .cloned()
            .collect::<Vec<_>>();
        let declarations = graph.get("declarations").ok_or(Refusal::Binding)?;
        check_relation(&census.names, members, &routes, declarations)
    })();
    handoff::check_seed(seed, view).map_err(|_error| Refusal::Binding)?;
    view.recheck().map_err(|_error| Refusal::Binding)?;
    result
}

// Candidate projections are test data only, never constructors for a sealed view.
fn check_relation(
    expected: &[PublicName],
    members: &[Value],
    routes: &[Value],
    declarations: &Value,
) -> Result<(), Refusal> {
    let names: BTreeSet<_> = expected.iter().map(|name| name.name.as_str()).collect();
    if names.is_empty() || names.len() != expected.len() {
        return Err(Refusal::Binding);
    }
    let owner_rows = declarations.as_array().ok_or(Refusal::Binding)?;
    let mut roots = BTreeMap::new();
    for member in members {
        let name = member
            .get("name")
            .and_then(Value::as_str)
            .ok_or(Refusal::Binding)?;
        let is_root = member
            .get("is_root")
            .and_then(Value::as_bool)
            .ok_or(Refusal::Binding)?;
        if is_root && roots.insert(name, member).is_some() {
            return Err(Refusal::UnexpectedName(name.into()));
        }
    }
    for name in roots.keys() {
        if !names.contains(*name) {
            return Err(Refusal::UnexpectedName((*name).into()));
        }
    }
    for name in expected {
        let member = roots
            .get(name.name.as_str())
            .ok_or_else(|| Refusal::MissingName(name.name.clone()))?;
        check_disposition(name, member, owner_rows)?;
    }
    let schema_names: BTreeSet<_> = expected
        .iter()
        .filter(|name| matches!(&name.disposition, Disposition::SchemaOwner(_)))
        .map(|name| name.name.as_str())
        .collect();
    let mut dispatch_by_name = BTreeMap::new();
    for route in routes {
        let name = route
            .get("member")
            .and_then(Value::as_str)
            .ok_or(Refusal::Binding)?;
        if !schema_names.contains(name) {
            return Err(Refusal::UnexpectedRoute(name.into()));
        }
        if dispatch_by_name.insert(name, route).is_some() {
            return Err(Refusal::DuplicateRoute(name.into()));
        }
    }
    for name in expected {
        if let Disposition::SchemaOwner(owner) = &name.disposition {
            let refusal = || Refusal::Route(name.name.clone());
            let route = dispatch_by_name
                .get(name.name.as_str())
                .ok_or_else(refusal)?;
            if route.get("from").and_then(Value::as_str) != Some("native_owner")
                || route.get("owner").and_then(Value::as_str) != Some(owner.as_str())
                || route.get("role").and_then(Value::as_str) != Some("native-owner-lookup")
                || route.get("ownership").and_then(Value::as_str) != Some("literal-only")
            {
                return Err(refusal());
            }
        }
    }
    Ok(())
}

fn check_disposition(
    name: &PublicName,
    member: &Value,
    owner_rows: &[Value],
) -> Result<(), Refusal> {
    let refusal = || Refusal::Disposition(name.name.clone());
    let ancestry = member
        .get("ancestry")
        .and_then(Value::as_array)
        .ok_or_else(refusal)?;
    if member.get("implementation").and_then(Value::as_str) != Some(name.implementation.as_str())
        || ancestry.first().and_then(Value::as_str) != name.parent.as_deref()
        || (name.parent.is_none() && !ancestry.is_empty())
    {
        return Err(refusal());
    }
    match &name.disposition {
        Disposition::SchemaOwner(owner) => {
            if owner.is_empty()
                || member.get("schema_owner").and_then(Value::as_str) != Some(owner.as_str())
                || owner_rows
                    .iter()
                    .filter(|row| row.get("owner").and_then(Value::as_str) == Some(owner.as_str()))
                    .count()
                    != 1
            {
                return Err(refusal());
            }
        }
        Disposition::LocalSchemaFree => {
            if name.parent.is_some() || !member.get("schema_owner").is_some_and(Value::is_null) {
                return Err(refusal());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Refusal, check_public_dispositions, check_relation};
    use crate::schema_build::application::CapturedSeed;
    use crate::schema_residency::{expanded, with_current_descriptor};
    use crate::schema_selection_source::{
        Lane,
        public_names::{self, Disposition, PublicName, PublicNameCensus},
    };
    use quote::ToTokens as _;
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::{collections::BTreeSet, path::Path, str};

    fn routes(graph: &Value) -> Vec<Value> {
        graph["auxiliary"]["dispatch_edges"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|row| row["from"] == "native_owner")
            .cloned()
            .collect()
    }

    fn copy_seed(seed: &CapturedSeed) -> CapturedSeed {
        CapturedSeed {
            identity_sha256: seed.identity_sha256.clone(),
            manifest_bytes: seed.manifest_bytes.clone(),
            rust: seed.rust.clone(),
            build_root: seed.build_root.clone(),
            out_dir: seed.out_dir.clone(),
            snapshots: seed.snapshots.clone(),
        }
    }

    fn assert_ancestry_only_route_refuses(
        census: &PublicNameCensus,
        members: &[Value],
        original_routes: &[Value],
        declarations: &Value,
    ) {
        let (relation_members, ancestor_name, owner) = if let Some(ancestor) = members
            .iter()
            .find(|row| row["is_root"].as_bool() == Some(false))
        {
            assert_eq!(ancestor["is_root"], false);
            (
                members.to_vec(),
                ancestor["name"].as_str().unwrap().to_owned(),
                ancestor["schema_owner"].as_str().unwrap().to_owned(),
            )
        } else {
            // Hypothetical PRIVATE relation fixture only. This additional
            // ancestor is never passed to the actual-source join entrypoint.
            let imported = census
                .imported_aliases
                .iter()
                .find(|name| {
                    !members
                        .iter()
                        .any(|row| row["name"].as_str() == Some(name.as_str()))
                })
                .expect("real imported-only alias for hypothetical ancestor metadata");
            let mut ancestor = members
                .iter()
                .find(|row| row["schema_owner"].is_string())
                .expect("known schema-bearing metadata for the private fixture")
                .clone();
            assert_eq!(ancestor["is_root"], true);
            ancestor["name"] = json!(imported);
            ancestor["is_root"] = json!(false);
            let owner = ancestor["schema_owner"].as_str().unwrap().to_owned();
            let mut hypothetical_members = members.to_vec();
            hypothetical_members.push(ancestor);
            (hypothetical_members, imported.clone(), owner)
        };
        assert_eq!(&relation_members[..members.len()], members);
        assert!(!census.names.iter().any(|row| row.name == ancestor_name));
        assert!(
            !original_routes
                .iter()
                .any(|row| row["member"] == ancestor_name)
        );
        assert_eq!(
            declarations
                .as_array()
                .unwrap()
                .iter()
                .filter(|row| row["owner"].as_str() == Some(owner.as_str()))
                .count(),
            1
        );
        assert_eq!(
            check_relation(
                &census.names,
                &relation_members,
                original_routes,
                declarations
            ),
            Ok(())
        );

        let mut changed_routes = original_routes.to_vec();
        let mut extra_route = original_routes
            .first()
            .expect("actual native owner route")
            .clone();
        extra_route["member"] = json!(ancestor_name);
        extra_route["owner"] = json!(owner);
        changed_routes.push(extra_route);
        assert_eq!(
            check_relation(
                &census.names,
                &relation_members,
                &changed_routes,
                declarations
            ),
            Err(Refusal::UnexpectedRoute(ancestor_name))
        );
    }

    // Build mutation inputs with syn/quote; the existing auxiliary classifier
    // remains the only parser/validator for native_owner dispatch semantics.
    fn seed_with_routes(seed: &CapturedSeed, routes: &[Value]) -> CapturedSeed {
        let mut changed = copy_seed(seed);
        let mut syntax = syn::parse_file(str::from_utf8(&seed.rust).unwrap()).unwrap();
        let names = routes
            .iter()
            .map(|row| row["member"].as_str().unwrap())
            .collect::<Vec<_>>();
        let owners = routes
            .iter()
            .map(|row| row["owner"].as_str().unwrap())
            .collect::<Vec<_>>();
        {
            let mut functions = syntax.items.iter_mut().filter_map(|item| {
                if let syn::Item::Fn(function) = item {
                    (function.sig.ident == "native_owner").then_some(function)
                } else {
                    None
                }
            });
            let function = functions.next().expect("captured native_owner function");
            assert!(functions.next().is_none());
            *function.block = syn::parse_quote!({
                match name { #(#names => Some(#owners),)* _ => None }
            })
        };
        changed.rust = syntax.to_token_stream().to_string().into_bytes();
        let mut manifest: Value = serde_json::from_slice(&changed.manifest_bytes).unwrap();
        manifest["emitted_rust_sha256"] = json!(hex::encode(Sha256::digest(&changed.rust)));
        changed.manifest_bytes = serde_json::to_vec(&manifest).unwrap();
        changed
    }

    fn assert_census_beforestate(
        census: &PublicNameCensus,
        members: &[Value],
        original_routes: &[Value],
        declarations: &Value,
    ) {
        assert_eq!(
            census
                .names
                .iter()
                .map(|row| row.name.to_ascii_lowercase())
                .collect::<BTreeSet<_>>()
                .len(),
            census.names.len()
        );
        for name in &census.names {
            let selected = members.iter().find(|row| row["name"] == name.name).unwrap();
            assert_eq!(
                selected["implementation"].as_str(),
                Some(name.implementation.as_str())
            );
            if let Some(parent) = &name.parent {
                assert_eq!(selected["ancestry"][0].as_str(), Some(parent.as_str()));
            }
            match &name.disposition {
                Disposition::SchemaOwner(owner) => {
                    assert_eq!(selected["schema_owner"].as_str(), Some(owner.as_str()));
                    assert_eq!(
                        declarations
                            .as_array()
                            .unwrap()
                            .iter()
                            .filter(|row| row["owner"].as_str() == Some(owner.as_str()))
                            .count(),
                        1
                    );
                }
                Disposition::LocalSchemaFree => assert!(selected["schema_owner"].is_null()),
            }
        }
        let schema_free = census
            .names
            .iter()
            .find(|row| row.disposition == Disposition::LocalSchemaFree)
            .expect("actual metadata must positively declare a local schema-free name");
        assert!(schema_free.parent.is_none());
        assert!(
            !original_routes
                .iter()
                .any(|row| row["member"] == schema_free.name)
        );
    }

    fn imported_alias<'a>(census: &'a PublicNameCensus, members: &[Value]) -> &'a String {
        let imported = census
            .imported_aliases
            .iter()
            .find(|name| {
                !members
                    .iter()
                    .any(|row| row["name"].as_str() == Some(name.as_str()))
            })
            .expect("real imported-only alias inventory");
        assert!(!census.names.iter().any(|row| row.name == *imported));
        assert!(
            !members
                .iter()
                .any(|row| row["name"].as_str() == Some(imported.as_str()))
        );
        imported
    }

    fn assert_name_controls(
        census: &PublicNameCensus,
        members: &[Value],
        original_routes: &[Value],
        declarations: &Value,
        alias: &PublicName,
        imported: &str,
    ) -> Vec<Value> {
        let member_index = members
            .iter()
            .position(|row| row["name"] == alias.name)
            .unwrap();
        let route_index = original_routes
            .iter()
            .position(|row| row["member"] == alias.name)
            .unwrap();
        let mut missing_members = members.to_vec();
        assert_eq!(missing_members.remove(member_index)["name"], alias.name);
        let mut missing_routes = original_routes.to_vec();
        assert_eq!(missing_routes.remove(route_index)["member"], alias.name);
        assert_eq!(
            check_relation(
                &census.names,
                &missing_members,
                &missing_routes,
                declarations
            ),
            Err(Refusal::MissingName(alias.name.clone()))
        );

        let mut imported_members = members.to_vec();
        let mut extra_member = members[member_index].clone();
        extra_member["name"] = json!(imported);
        imported_members.push(extra_member);
        let mut imported_routes = original_routes.to_vec();
        let mut extra_route = original_routes[route_index].clone();
        extra_route["member"] = json!(imported);
        imported_routes.push(extra_route);
        assert_eq!(
            check_relation(
                &census.names,
                &imported_members,
                &imported_routes,
                declarations
            ),
            Err(Refusal::UnexpectedName(imported.to_owned()))
        );
        missing_routes
    }

    fn assert_changed_disposition_refuses(
        census: &PublicNameCensus,
        members: &[Value],
        missing_routes: &[Value],
        declarations: &Value,
        alias: &PublicName,
        member_index: usize,
    ) {
        let mut changed_disposition = members.to_vec();
        assert!(changed_disposition[member_index]["schema_owner"].is_string());
        changed_disposition[member_index]["schema_owner"] = Value::Null;
        assert_eq!(
            check_relation(
                &census.names,
                &changed_disposition,
                missing_routes,
                declarations
            ),
            Err(Refusal::Disposition(alias.name.clone()))
        );
    }

    fn assert_dispatch_source_controls(
        seed: &CapturedSeed,
        original_routes: &[Value],
        declarations: &Value,
        duplicate: &[Value],
        misrouted: &[Value],
    ) {
        let rebuilt = seed_with_routes(seed, original_routes);
        let rebuilt_graph =
            expanded::expand(&rebuilt).expect("unchanged dispatch remains independently valid");
        assert_eq!(routes(&rebuilt_graph), original_routes);
        assert_eq!(rebuilt_graph["declarations"], *declarations);
        for changed_routes in [duplicate, misrouted] {
            let changed = seed_with_routes(seed, changed_routes);
            assert_eq!(
                expanded::expand(&changed).map(|_graph| ()),
                Err("unsupported captured staging syntax".to_owned())
            );
        }
    }

    #[test]
    fn every_public_name_has_exact_schema_disposition() {
        with_current_descriptor(|seed, descriptor| {
            let lane = if cfg!(feature = "insecure-bidders") {
                Lane::Development
            } else {
                Lane::Secure
            };
            let census = public_names::discover(Path::new(env!("CARGO_MANIFEST_DIR")), lane)?;
            let view = descriptor.validation_view().unwrap();
            let members = view.selection()["members"].as_array().unwrap();
            let original_routes = routes(view.graph());
            let declarations = &view.graph()["declarations"];
            assert!(!census.names.is_empty());
            assert!(!original_routes.is_empty());
            assert_eq!(view.graph()["auxiliary"]["status"], "Complete");
            assert_census_beforestate(&census, members, &original_routes, declarations);
            let alias = census
                .names
                .iter()
                .find(|row| {
                    row.parent.is_some() && matches!(&row.disposition, Disposition::SchemaOwner(_))
                })
                .expect("actual executable schema-bearing alias");
            let member_index = members
                .iter()
                .position(|row| row["name"] == alias.name)
                .unwrap();
            let route_index = original_routes
                .iter()
                .position(|row| row["member"] == alias.name)
                .unwrap();
            let alias_owner = original_routes[route_index]["owner"].as_str().unwrap();
            let other_owner = declarations
                .as_array()
                .unwrap()
                .iter()
                .find_map(|row| row["owner"].as_str().filter(|owner| *owner != alias_owner))
                .expect("another real declaration for a misroute control");
            let imported = imported_alias(&census, members);

            assert_eq!(check_public_dispositions(seed, view, &census), Ok(()));
            assert_eq!(
                check_relation(&census.names, members, &original_routes, declarations),
                Ok(())
            );

            assert_ancestry_only_route_refuses(&census, members, &original_routes, declarations);

            let missing_routes = assert_name_controls(
                &census,
                members,
                &original_routes,
                declarations,
                alias,
                imported,
            );

            let mut duplicate = original_routes.clone();
            duplicate.push(original_routes[route_index].clone());
            assert_eq!(
                check_relation(&census.names, members, &duplicate, declarations),
                Err(Refusal::DuplicateRoute(alias.name.clone()))
            );

            let mut misrouted = original_routes.clone();
            misrouted[route_index]["owner"] = json!(other_owner);
            assert_ne!(misrouted[route_index], original_routes[route_index]);
            assert_eq!(
                check_relation(&census.names, members, &misrouted, declarations),
                Err(Refusal::Route(alias.name.clone()))
            );

            assert_changed_disposition_refuses(
                &census,
                members,
                &missing_routes,
                declarations,
                alias,
                member_index,
            );

            assert_dispatch_source_controls(
                seed,
                &original_routes,
                declarations,
                &duplicate,
                &misrouted,
            );

            let mut other_binding = copy_seed(seed);
            other_binding.identity_sha256 = "fake_other_capture".into();
            assert_eq!(
                check_public_dispositions(&other_binding, view, &census),
                Err(Refusal::Binding)
            );
            assert_eq!(view.check_handoff(view.assignments()), Ok(()));
            Ok(())
        })
        .unwrap();
    }
}
