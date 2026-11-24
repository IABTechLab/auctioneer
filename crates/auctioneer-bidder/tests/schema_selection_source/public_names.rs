//! Data projection of the existing independent census, not source-proof authority.

use super::{Lane, Provenance, Schema, independent_membership_from_inputs, load_source};
use std::path::Path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Disposition {
    SchemaOwner(String),
    LocalSchemaFree,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PublicName {
    pub name: String,
    pub implementation: String,
    pub parent: Option<String>,
    pub disposition: Disposition,
}

#[derive(Debug)]
pub(crate) struct PublicNameCensus {
    pub names: Vec<PublicName>,
    pub imported_aliases: Vec<String>,
}

/// Preserve the existing typed schema disposition without changing its JSON report.
/// `LocalSchemaFree` requires positive local provenance, never merely a null owner.
pub(crate) fn discover(crate_root: &Path, lane: Lane) -> Result<PublicNameCensus, String> {
    let (inputs, catalog) = load_source(crate_root)?;
    let membership = independent_membership_from_inputs(&inputs, lane)?;
    let roots = membership["roots"]
        .as_array()
        .ok_or("missing independent roots")?;
    let members = membership["members"]
        .as_array()
        .ok_or("missing independent members")?;
    let mut names = Vec::new();
    for root in roots {
        let name = root.as_str().ok_or("invalid independent public name")?;
        let definition = inputs
            .definitions
            .iter()
            .find(|row| row.name == name)
            .ok_or("missing independent definition")?;
        let publication = inputs
            .publications
            .iter()
            .find(|row| row.name == name)
            .ok_or("public name is not an executable definition")?;
        let member = members
            .iter()
            .find(|row| row["name"].as_str() == Some(name))
            .ok_or("missing independent selected member")?;
        let manifest = catalog
            .manifests
            .get(&definition.implementation)
            .ok_or("missing independent public-name manifest")?;
        if publication.implementation != definition.implementation
            || !manifest.factory
            || !manifest.definitions.contains_key(name)
        {
            return Err("independent executable definition mismatch".into());
        }
        let owners = [definition.owner.as_deref().unwrap_or("")];
        let disposition = match definition.schema(&owners) {
            Schema::LocalSchemaFree => {
                if !matches!(&manifest.provenance, Provenance::Local {})
                    || definition.parent.is_some()
                    || !member["effective_schema_owner"].is_null()
                {
                    return Err("invalid explicit local schema-free disposition".into());
                }
                Disposition::LocalSchemaFree
            }
            Schema::Owners(_) | Schema::Inherit => {
                let owner = member["effective_schema_owner"]
                    .as_str()
                    .filter(|owner| !owner.is_empty())
                    .ok_or("schema-bearing public name has no independent owner")?;
                Disposition::SchemaOwner(owner.into())
            }
        };
        names.push(PublicName {
            name: name.into(),
            implementation: definition.implementation.clone(),
            parent: definition.parent.clone(),
            disposition,
        });
    }
    let mut imported_aliases = inputs
        .definitions
        .iter()
        .filter(|row| row.parent.is_some() && !inputs.implemented.contains(&row.name))
        .map(|row| row.name.clone())
        .collect::<Vec<_>>();
    imported_aliases.sort();
    Ok(PublicNameCensus {
        names,
        imported_aliases,
    })
}
