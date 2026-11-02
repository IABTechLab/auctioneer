//! Original source associations, never executable factories or reviewed definitions.

use super::bidder_manifest::{
    Artifact, Category, HttpsEndpointDefault, Manifest, Provenance, Retained, Source, SourceTree,
};
use super::check;
use super::import::{Files, insert, name};
use super::inventory::{Capture, Document};
use super::source::REVISION;
use super::{Error, metadata, yaml};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const BINDINGS: &[u8] = include_bytes!(
    "../../../auctioneer-bidder/tests/conformance/fixtures/reference/catalog/source-bindings.toml"
);
pub(super) const REPOSITORY: &str = "https://github.com/prebid/prebid-server.git";
const MAX_SCHEMA_BYTES: usize = 16 << 10;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RoutingReview {
    MissingEndpoint,
    HttpsDefaultPending,
    EndpointPolicyPending,
    AliasOnly,
}

impl RoutingReview {
    fn note(&self) -> &'static str {
        match self {
            Self::MissingEndpoint => {
                "Source routing unavailable: missing endpoint; no runtime definition is generated."
            }
            Self::HttpsDefaultPending => {
                "Source HTTP template uses the HTTPS default; effective metadata binding and route tests remain pending; no new runtime definition is generated."
            }
            Self::EndpointPolicyPending => {
                "Source endpoint policy is unreviewed; no runtime definition is generated."
            }
            Self::AliasOnly => {
                "Source identity is white-label-only; it supplies no independently callable base."
            }
        }
    }
}

pub(super) fn route_review(document: &Value, parent: Option<&Value>) -> RoutingReview {
    let endpoint = endpoint_source(document, parent);
    if document.get("whiteLabelOnly").and_then(Value::as_bool) == Some(true) {
        RoutingReview::AliasOnly
    } else if endpoint.is_none() {
        RoutingReview::MissingEndpoint
    } else if endpoint.is_some_and(|(url, _)| http_template_tail(url).is_some()) {
        RoutingReview::HttpsDefaultPending
    } else {
        RoutingReview::EndpointPolicyPending
    }
}

fn endpoint_source<'a>(
    document: &'a Value,
    parent: Option<&'a Value>,
) -> Option<(&'a str, Option<&'a str>)> {
    document
        .get("endpoint")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(|url| (url, None))
        .or_else(|| {
            parent
                .and_then(|base| base.get("endpoint"))
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(|url| (url, document.get("aliasOf").and_then(Value::as_str)))
        })
}

fn http_template_tail(template: &str) -> Option<&str> {
    let (scheme, tail) = template.split_once("://")?;
    scheme.eq_ignore_ascii_case("http").then_some(tail)
}

fn https_default(document: &Value, parent: Option<&Value>) -> Option<HttpsEndpointDefault> {
    if document.get("whiteLabelOnly").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let (source_url, inherited_from) = endpoint_source(document, parent)?;
    // Preserve template bytes; URL parsing/serialization could rewrite macros or paths.
    let selected_url = format!("https://{}", http_template_tail(source_url)?);
    Some(HttpsEndpointDefault {
        source_url: source_url.to_owned(),
        selected_url,
        inherited_from: inherited_from.map(str::to_owned),
    })
}

struct Package {
    manifest: Manifest,
    source_directory: String,
    bases: BTreeSet<String>,
}

impl Package {
    fn new(key: &str, directory: &str, tree: &str, license: &Document) -> Result<Self, Error> {
        Ok(Self {
            manifest: Manifest {
                key: key.to_owned(),
                factory: false,
                required_feature: None,
                native_only: false,
                endpoint: None,
                settings: None,
                definitions: BTreeMap::new(),
                provenance: Provenance::Reference {
                    sources: BTreeMap::from([("upstream".to_owned(), Source {
                        repository: REPOSITORY.to_owned(), revision: REVISION.to_owned(),
                    })]),
                    trees: vec![SourceTree {
                        category: Category::Implementation,
                        source: "upstream".to_owned(),
                        source_path: directory.to_owned(),
                        tree: tree.to_owned(),
                        port_differences: vec!["Reference-only source association; no Rust port, factory, endpoint approval or protocol parity is claimed.".to_owned()],
                    }],
                    artifacts: vec![artifact(license, Category::License, None)?],
                },
            },
            source_directory: directory.to_owned(),
            bases: BTreeSet::new(),
        })
    }

    fn artifacts(&mut self) -> Result<&mut Vec<Artifact>, Error> {
        match &mut self.manifest.provenance {
            Provenance::Reference { artifacts, .. } => Ok(artifacts),
            Provenance::Local {} => Err(Error::InvalidSource),
        }
    }

    fn schema_path(&self, name: &str) -> Result<PathBuf, Error> {
        if !self.bases.contains(name) {
            return Err(Error::InvalidSource);
        }
        let directory_name = self
            .source_directory
            .strip_prefix("adapters/")
            .ok_or(Error::InvalidSource)?;
        if self.bases.len() == 1 || name == directory_name {
            Ok(PathBuf::from("parameters.json"))
        } else {
            Ok(PathBuf::from(format!("schemas/{name}.schema.json")))
        }
    }
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn artifact(
    document: &Document,
    category: Category,
    retained: Option<&Path>,
) -> Result<Artifact, Error> {
    let bytes = u64::try_from(document.bytes.len()).map_err(|_error| Error::SourceLimit)?;
    let sha256 = digest(&document.bytes);
    Ok(Artifact {
        category,
        source: "upstream".to_owned(),
        source_path: document.entry.path.clone(),
        source_blob: document.entry.oid.clone(),
        source_bytes: bytes,
        source_sha256: sha256.clone(),
        retained: retained
            .map(|path| -> Result<Retained, Error> {
                Ok(Retained {
                    path: path.to_str().ok_or(Error::InvalidSource)?.to_owned(),
                    bytes,
                    sha256,
                })
            })
            .transpose()?,
        transformation: None,
        https_default: None,
        port_differences: Vec::new(),
    })
}

fn owners(
    converted: &BTreeMap<String, metadata::Converted>,
    directory_keys: &BTreeMap<&str, &str>,
    packages: &mut BTreeMap<String, Package>,
) -> Result<BTreeMap<String, String>, Error> {
    let supplied = check::parse_factory_bindings(BINDINGS)?;
    let bindings = supplied
        .get("bindings")
        .and_then(toml::Value::as_table)
        .ok_or(Error::InvalidSource)?;
    let mut owners = BTreeMap::new();
    for (name, value) in converted {
        if value
            .document
            .get("aliasOf")
            .and_then(Value::as_str)
            .is_some_and(|parent| !parent.is_empty())
        {
            continue;
        }
        let directory = bindings
            .get(name)
            .and_then(toml::Value::as_str)
            .ok_or(Error::InvalidSource)?;
        let key = *directory_keys.get(directory).ok_or(Error::InvalidSource)?;
        packages
            .get_mut(key)
            .ok_or(Error::InvalidSource)?
            .bases
            .insert(name.clone());
        owners.insert(name.clone(), key.to_owned());
    }
    if owners.len() != 261 || owners.len() != bindings.len() {
        return Err(Error::InvalidSource);
    }
    for (name, value) in converted {
        let Some(parent) = value
            .document
            .get("aliasOf")
            .and_then(Value::as_str)
            .filter(|parent| !parent.is_empty())
        else {
            continue;
        };
        let key = owners.get(parent).ok_or(Error::InvalidSource)?.clone();
        if owners.insert(name.clone(), key).is_some() {
            return Err(Error::InvalidSource);
        }
    }
    if owners.len() != 358 || packages.values().any(|package| package.bases.is_empty()) {
        return Err(Error::InvalidSource);
    }
    Ok(owners)
}

pub(super) fn collect(capture: Capture) -> Result<Files, Error> {
    let mut packages = BTreeMap::new();
    let directory_keys: BTreeMap<_, _> = capture.inventory.implementation_keys().collect();
    for (directory, key) in &directory_keys {
        let tree = capture
            .implementation_trees
            .get(*directory)
            .ok_or(Error::InvalidSource)?;
        packages.insert(
            (*key).to_owned(),
            Package::new(key, directory, tree, &capture.license)?,
        );
    }
    let converted: BTreeMap<_, _> = capture
        .metadata
        .iter()
        .map(|document| {
            Ok((
                name(document, "static/bidder-info/", ".yaml")?,
                metadata::convert(yaml::parse(&document.bytes)?)?,
            ))
        })
        .collect::<Result<_, Error>>()?;
    let assignments = owners(&converted, &directory_keys, &mut packages)?;
    let mut files = Files::new();
    for document in &capture.metadata {
        let public = name(document, "static/bidder-info/", ".yaml")?;
        let key = assignments.get(&public).ok_or(Error::InvalidSource)?;
        let source = converted.get(&public).ok_or(Error::InvalidSource)?;
        let parent = source
            .document
            .get("aliasOf")
            .and_then(Value::as_str)
            .filter(|parent| !parent.is_empty())
            .map(|parent| {
                converted
                    .get(parent)
                    .map(|value| &value.document)
                    .ok_or(Error::InvalidSource)
            })
            .transpose()?;
        let mut witness = artifact(document, Category::Metadata, None)?;
        witness.https_default = https_default(&source.document, parent);
        witness
            .port_differences
            .push(route_review(&source.document, parent).note().to_owned());
        witness.port_differences.extend(source.ignored_fields.iter().map(|field| {
            format!("Ignored source spelling {field} is identified by the pinned source witness, not promoted into an effective setting.")
        }));
        packages
            .get_mut(key)
            .ok_or(Error::InvalidSource)?
            .artifacts()?
            .push(witness);
    }
    for document in &capture.schemas {
        if document.bytes.len() > MAX_SCHEMA_BYTES {
            return Err(Error::SourceLimit);
        }
        let public = name(document, "static/bidder-params/", ".json")?;
        let key = assignments.get(&public).ok_or(Error::InvalidSource)?;
        let package = packages.get_mut(key).ok_or(Error::InvalidSource)?;
        let path = package.schema_path(&public)?;
        package
            .artifacts()?
            .push(artifact(document, Category::Schema, Some(&path))?);
        insert(
            &mut files,
            Path::new(key).join(path),
            document.bytes.clone(),
        )?;
    }
    for (key, package) in packages {
        let bytes = toml::to_string_pretty(&package.manifest)
            .map_err(|_error| Error::ImportOutput)?
            .into_bytes();
        insert(&mut files, Path::new(&key).join("bidder.toml"), bytes)?;
    }
    insert(
        &mut files,
        PathBuf::from("LICENSE.reference"),
        capture.license.bytes,
    )?;
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::{https_default, route_review};
    use serde_json::json;

    #[test]
    fn https_default_changes_only_the_source_http_scheme() {
        for endpoint in [
            "http://Bid.Example.test:80/a%2fb?account={{.AccountID}}&src=fake",
            "http://{{.Host}}.example.test/{{.PublisherID}}?tag=one+two",
            "HTTP://bid.example.test:8080/rtb",
        ] {
            for disabled in [false, true] {
                let selected =
                    https_default(&json!({"endpoint": endpoint, "disabled": disabled}), None)
                        .unwrap();
                assert_eq!(selected.source_url, endpoint);
                assert_eq!(
                    selected.selected_url,
                    format!("https://{}", endpoint.split_once("://").unwrap().1)
                );
                assert_eq!(selected.inherited_from, None);
            }
        }
    }

    #[test]
    fn https_default_binds_inheritance_but_respects_explicit_endpoint_precedence() {
        let parent = json!({"endpoint": "http://parent.example.test/rtb?source=fake", "whiteLabelOnly": true});
        for endpoint in [json!(null), json!("")] {
            let selected = https_default(
                &json!({"aliasOf": "fake_parent", "endpoint": endpoint}),
                Some(&parent),
            )
            .unwrap();
            assert_eq!(
                selected.source_url,
                "http://parent.example.test/rtb?source=fake"
            );
            assert_eq!(
                selected.selected_url,
                "https://parent.example.test/rtb?source=fake"
            );
            assert_eq!(selected.inherited_from.as_deref(), Some("fake_parent"));
        }
        let explicit = https_default(
            &json!({"aliasOf": "fake_parent", "endpoint": "http://alias.example.test/rtb"}),
            Some(&parent),
        )
        .unwrap();
        assert_eq!(explicit.source_url, "http://alias.example.test/rtb");
        assert_eq!(explicit.selected_url, "https://alias.example.test/rtb");
        assert_eq!(explicit.inherited_from, None);
        for document in [
            json!({"aliasOf": "fake_parent", "endpoint": "https://alias.example.test/rtb"}),
            json!({"whiteLabelOnly": true, "endpoint": "http://parent.example.test/rtb"}),
        ] {
            assert_eq!(https_default(&document, Some(&parent)), None);
        }
    }

    #[test]
    fn https_default_does_not_invent_an_endpoint_or_an_http_exception() {
        for document in [
            json!({}),
            json!({"endpoint": ""}),
            json!({"endpoint": null}),
            json!({"endpoint": "https://bid.example.test/rtb"}),
            json!({"endpoint": "ftp://bid.example.test/rtb"}),
            json!({"endpoint": "//bid.example.test/rtb"}),
            json!({"whiteLabelOnly": true, "endpoint": "http://bid.example.test/rtb"}),
        ] {
            assert_eq!(https_default(&document, None), None);
        }
    }

    #[test]
    fn http_transport_selection_no_longer_requires_independent_tls_review() {
        assert_eq!(
            route_review(&json!({"endpoint": "http://bid.example.test/rtb"}), None).note(),
            "Source HTTP template uses the HTTPS default; effective metadata binding and route tests remain pending; no new runtime definition is generated."
        );
    }
}
