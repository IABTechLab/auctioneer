use super::Error;
use super::source::{Entry, GitSource, REVISION};
use super::sync::{self, Declaration, Family, Member, Origin, Summary};
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

#[derive(Serialize)]
pub(super) struct Inventory {
    schema_version: u32,
    kind: &'static str,
    revision: &'static str,
    counts: Counts,
    excluded_implementation_directories: Vec<String>,
    implementations: Vec<Implementation>,
    identity: Vec<Identity>,
    schema: Vec<Artifact>,
    sync_family: Vec<Family>,
}

impl Inventory {
    fn validate_source_pin(&self) -> Result<(), Error> {
        // Independently frozen source inventory, not a digest learned from this scan.
        let bytes = toml::to_string_pretty(self).map_err(|_error| Error::Output)?;
        if hex::encode(Sha256::digest(bytes.as_bytes()))
            != "b22f74407fd081cd7d073b8724ba8f74e1385163b9bc30c2277342adc38bb6c6"
        {
            return Err(Error::InvalidSource);
        }
        Ok(())
    }

    pub fn identity_count(&self) -> usize {
        self.identity.len()
    }

    pub fn implementation_keys(&self) -> impl Iterator<Item = (&str, &str)> {
        self.implementations.iter().map(|implementation| {
            (
                implementation.source_directory.as_str(),
                implementation.internal_key.as_str(),
            )
        })
    }
}

#[derive(Serialize)]
struct Counts {
    identities: usize,
    bases: usize,
    aliases: usize,
    implementations: usize,
    schemas: usize,
    schema_bytes: usize,
    largest_schema_bytes: usize,
    http_bases: usize,
    enabled_http_bases: usize,
    disabled_http_bases: usize,
    sync_identities: usize,
    sync_families: usize,
    without_sync: usize,
    declared_sync_identities: usize,
    inherited_sync_identities: usize,
    supports_only_identities: usize,
}

#[derive(Serialize)]
struct Implementation {
    source_directory: String,
    internal_key: String,
}

#[derive(Serialize)]
struct Artifact {
    name: String,
    path: String,
    blob: String,
    bytes: usize,
    sha256: String,
}

#[derive(Serialize)]
struct Identity {
    #[serde(flatten)]
    artifact: Artifact,
    #[serde(skip_serializing_if = "Option::is_none")]
    alias_of: Option<String>,
    source_disabled: bool,
    source_vendor_id: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    endpoint: Option<String>,
    metadata_fields: Vec<String>,
    sync: Option<Summary>,
    #[serde(skip)]
    declared_sync: Option<Declaration>,
}

#[derive(Deserialize)]
struct Metadata {
    #[serde(rename = "aliasOf")]
    alias_of: Option<String>,
    #[serde(default)]
    disabled: bool,
    endpoint: Option<String>,
    #[serde(rename = "gvlVendorID")]
    vendor_id: Option<u16>,
    #[serde(rename = "userSync")]
    sync: Option<Declaration>,
    #[serde(flatten)]
    other: BTreeMap<String, Value>,
}

fn artifact(entry: &Entry, bytes: &[u8], root: &str, suffix: &str) -> Result<Artifact, Error> {
    let name = entry
        .path
        .strip_prefix(root)
        .and_then(|name| name.strip_suffix(suffix))
        .ok_or(Error::InvalidSource)?;
    if name.is_empty() || name.contains('/') {
        return Err(Error::InvalidSource);
    }
    Ok(Artifact {
        name: name.to_owned(),
        path: entry.path.clone(),
        blob: entry.oid.clone(),
        bytes: entry.bytes,
        sha256: hex::encode(Sha256::digest(bytes)),
    })
}

fn parse_metadata(entry: &Entry, bytes: &[u8]) -> Result<Identity, Error> {
    let value = super::yaml::parse(bytes)?;
    let fields = value.as_mapping().ok_or(Error::InvalidSource)?;
    let metadata_fields = fields
        .keys()
        .map(|key| key.as_str().map(str::to_owned).ok_or(Error::InvalidSource))
        .collect::<Result<BTreeSet<_>, _>>()?
        .into_iter()
        .collect();
    let parsed: Metadata =
        serde_yaml_ng::from_value(value).map_err(|_error| Error::InvalidSource)?;
    // Unmapped fields remain visible in the inventory; import must review them.
    let _unmapped = parsed.other;
    Ok(Identity {
        artifact: artifact(entry, bytes, "static/bidder-info/", ".yaml")?,
        alias_of: parsed.alias_of.filter(|name| !name.is_empty()),
        source_disabled: parsed.disabled,
        source_vendor_id: parsed.vendor_id.unwrap_or(0),
        endpoint: parsed.endpoint,
        metadata_fields,
        sync: None,
        declared_sync: parsed.sync,
    })
}

fn implementation_key(name: &str) -> Result<String, Error> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(Error::InvalidSource);
    }
    let bytes = name.as_bytes();
    let mut key = String::new();
    if bytes.first().is_some_and(u8::is_ascii_digit) {
        key.push_str("bidder_");
    }
    for (index, byte) in bytes.iter().copied().enumerate() {
        let previous = index.checked_sub(1).and_then(|before| bytes.get(before));
        let next = bytes.get(index.saturating_add(1));
        if byte.is_ascii_uppercase()
            && previous.is_some_and(|before| {
                before.is_ascii_lowercase()
                    || before.is_ascii_digit()
                    || (before.is_ascii_uppercase() && next.is_some_and(u8::is_ascii_lowercase))
            })
        {
            key.push('_');
        }
        key.push(char::from(byte.to_ascii_lowercase()));
    }
    if matches!(
        key.as_str(),
        "as" | "async"
            | "await"
            | "break"
            | "const"
            | "continue"
            | "crate"
            | "dyn"
            | "else"
            | "enum"
            | "extern"
            | "false"
            | "fn"
            | "for"
            | "gen"
            | "if"
            | "impl"
            | "in"
            | "let"
            | "loop"
            | "match"
            | "mod"
            | "move"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "static"
            | "struct"
            | "super"
            | "trait"
            | "true"
            | "type"
            | "unsafe"
            | "use"
            | "where"
            | "while"
            | "abstract"
            | "become"
            | "box"
            | "do"
            | "final"
            | "macro"
            | "override"
            | "priv"
            | "try"
            | "typeof"
            | "unsized"
            | "virtual"
            | "yield"
    ) {
        key.push_str("_bidder");
    }
    if matches!(key.as_str(), "_" | "harness" | "url" | "media_type") {
        return Err(Error::InvalidSource);
    }
    Ok(key)
}

fn implementations(entries: &[Entry]) -> Result<Vec<Implementation>, Error> {
    let directories: BTreeSet<_> = entries
        .iter()
        .filter_map(|entry| {
            entry
                .path
                .strip_prefix("adapters/")?
                .split_once('/')
                .map(|(directory, _file)| directory)
        })
        .collect();
    if directories.len() != 260 || !directories.contains("adapterstest") {
        return Err(Error::InvalidSource);
    }
    let mut seen = BTreeSet::new();
    directories
        .into_iter()
        .filter(|name| *name != "adapterstest")
        .map(|name| {
            let key = implementation_key(name)?;
            if !seen.insert(key.clone()) {
                return Err(Error::InvalidSource);
            }
            Ok(Implementation {
                source_directory: format!("adapters/{name}"),
                internal_key: key,
            })
        })
        .collect()
}

fn validate(
    identities: &[Identity],
    schemas: &[Artifact],
    families: &[Family],
) -> Result<Counts, Error> {
    let by_name: BTreeMap<_, _> = identities
        .iter()
        .map(|row| (row.artifact.name.as_str(), row))
        .collect();
    if by_name.len() != 358 || identities.len() != 358 || schemas.len() != 261 {
        return Err(Error::InvalidSource);
    }
    let bases: BTreeSet<_> = identities
        .iter()
        .filter(|row| row.alias_of.is_none())
        .map(|row| row.artifact.name.as_str())
        .collect();
    let schema_names: BTreeSet<_> = schemas.iter().map(|row| row.name.as_str()).collect();
    if bases.len() != 261 || schema_names.len() != 261 || bases != schema_names {
        return Err(Error::InvalidSource);
    }
    let summaries = identities
        .iter()
        .map(|row| row.sync.as_ref().ok_or(Error::InvalidSource))
        .collect::<Result<Vec<_>, _>>()?;
    let defined = summaries.iter().filter(|row| row.defined).count();
    let declared = summaries
        .iter()
        .filter(|row| row.origin == Origin::Declared)
        .count();
    let inherited = summaries
        .iter()
        .filter(|row| row.origin == Origin::InheritedKey)
        .count();
    let supports_only = summaries
        .iter()
        .filter(|row| row.origin == Origin::SupportsOnly)
        .count();
    if (defined, families.len(), declared, inherited, supports_only) != (196, 187, 188, 8, 38) {
        return Err(Error::InvalidSource);
    }
    let http: Vec<_> = identities
        .iter()
        .filter(|row| {
            row.alias_of.is_none()
                && row
                    .endpoint
                    .as_deref()
                    .is_some_and(|endpoint| endpoint.starts_with("http://"))
        })
        .collect();
    let disabled: Vec<_> = http
        .iter()
        .filter(|row| row.source_disabled)
        .map(|row| row.artifact.name.as_str())
        .collect();
    if http.len() != 78 || disabled.len() != 1 {
        return Err(Error::InvalidSource);
    }
    let schema_bytes = schemas
        .iter()
        .try_fold(0_usize, |sum, row| sum.checked_add(row.bytes))
        .ok_or(Error::SourceLimit)?;
    let largest = schemas
        .iter()
        .max_by_key(|row| row.bytes)
        .ok_or(Error::InvalidSource)?;
    if schema_bytes != 164_518
        || largest.bytes != 3_778
        || largest.blob != "37909b12066ad9239b488b9751d070212e90d61a"
    {
        return Err(Error::InvalidSource);
    }
    Ok(Counts {
        identities: 358,
        bases: 261,
        aliases: 97,
        implementations: 259,
        schemas: 261,
        schema_bytes,
        largest_schema_bytes: largest.bytes,
        http_bases: http.len(),
        enabled_http_bases: http.len().saturating_sub(disabled.len()),
        disabled_http_bases: disabled.len(),
        sync_identities: defined,
        sync_families: families.len(),
        without_sync: identities.len().saturating_sub(defined),
        declared_sync_identities: declared,
        inherited_sync_identities: inherited,
        supports_only_identities: supports_only,
    })
}

pub(super) struct Document {
    pub entry: Entry,
    pub bytes: Vec<u8>,
}

pub(super) struct Capture {
    pub inventory: Inventory,
    pub implementation_trees: BTreeMap<String, String>,
    pub metadata: Vec<Document>,
    pub schemas: Vec<Document>,
    pub license: Document,
}

pub(super) async fn inspect(path: &Path) -> Result<Inventory, Error> {
    Ok(capture(path).await?.inventory)
}

pub(super) async fn capture(path: &Path) -> Result<Capture, Error> {
    let mut source = GitSource::open(path).await?;
    let mut metadata = Vec::new();
    let mut schemas = Vec::new();
    let mut identity = Vec::new();
    for entry in source.entries("static/bidder-info").await? {
        let bytes = source.blob(&entry).await?;
        identity.push(parse_metadata(&entry, &bytes)?);
        metadata.push(Document { entry, bytes });
    }
    let members: Vec<_> = identity
        .iter()
        .map(|row| Member {
            name: &row.artifact.name,
            parent: row.alias_of.as_deref(),
            declared: row.declared_sync.as_ref(),
        })
        .collect();
    let mut sync = sync::reconcile(&members)?;
    for row in &mut identity {
        row.sync = Some(
            sync.by_name
                .remove(&row.artifact.name)
                .ok_or(Error::InvalidSource)?,
        );
    }
    let mut schema = Vec::new();
    for entry in source.entries("static/bidder-params").await? {
        let bytes = source.blob(&entry).await?;
        let parsed: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_error| Error::InvalidSource)?;
        if !parsed.is_object() {
            return Err(Error::InvalidSource);
        }
        schema.push(artifact(&entry, &bytes, "static/bidder-params/", ".json")?);
        schemas.push(Document { entry, bytes });
    }
    let implementations = implementations(&source.entries("adapters").await?)?;
    let implementation_trees = source.implementation_trees().await?;
    let counts = validate(&identity, &schema, &sync.families)?;
    let mut license_entries = source.entries("LICENSE").await?;
    if license_entries.len() != 1 {
        return Err(Error::InvalidSource);
    }
    let entry = license_entries.pop().ok_or(Error::InvalidSource)?;
    let bytes = source.blob(&entry).await?;
    let license = Document { entry, bytes };
    let inventory = Inventory {
        schema_version: 2,
        kind: "reference_inventory_only",
        revision: REVISION,
        counts,
        excluded_implementation_directories: vec!["adapters/adapterstest".to_owned()],
        implementations,
        identity,
        schema,
        sync_family: sync.families,
    };
    inventory.validate_source_pin()?;
    Ok(Capture {
        inventory,
        implementation_trees,
        metadata,
        schemas,
        license,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::var_os;
    use std::mem::replace;

    #[tokio::test]
    async fn source_capture_pin_rejects_identity_and_schema_drift_without_count_changes() {
        let source = var_os("AUCTIONEER__BIDDER__SOURCE_DIR").unwrap();
        let mut inventory = inspect(Path::new(&source)).await.unwrap();
        assert_eq!(inventory.validate_source_pin(), Ok(()));
        let original_endpoint = inventory.identity[0]
            .endpoint
            .replace("http://fake_bidder.invalid".to_owned());
        assert_eq!(inventory.validate_source_pin(), Err(Error::InvalidSource));
        inventory.identity[0].endpoint = original_endpoint;
        let original_name = replace(&mut inventory.schema[0].name, "fake_schema".to_owned());
        assert_eq!(inventory.validate_source_pin(), Err(Error::InvalidSource));
        inventory.schema[0].name = original_name;
        let original_digest = replace(&mut inventory.schema[0].sha256, "0".repeat(64));
        assert_eq!(inventory.validate_source_pin(), Err(Error::InvalidSource));
        inventory.schema[0].sha256 = original_digest;
        assert_eq!(inventory.validate_source_pin(), Ok(()));
    }

    #[test]
    fn implementation_names_preserve_boundaries_and_escape_rust_keywords() {
        // Leading-digit and reserved-word controls must retain their exact spelling.
        for (name, expected) in [
            ("33fake", "bidder_33fake"),
            ("fake_bidderName", "fake_bidder_name"),
            ("fake_bidderBC", "fake_bidder_bc"),
            ("fake_sampleName", "fake_sample_name"),
            ("fake_HTTPServer", "fake_http_server"),
            ("fake_a_B", "fake_a_b"),
            ("type", "type_bidder"),
            ("gen", "gen_bidder"),
        ] {
            assert_eq!(implementation_key(name), Ok(expected.to_owned()));
        }
        for name in [
            "",
            "_",
            "has-dash",
            "../outside",
            "nonascii_\u{e9}",
            "url",
            "media_type",
            "harness",
        ] {
            assert_eq!(implementation_key(name), Err(Error::InvalidSource));
        }
    }

    fn entry() -> Entry {
        Entry {
            path: "static/bidder-info/fake_example.yaml".to_owned(),
            oid: "0".repeat(40),
            bytes: 0,
        }
    }

    #[test]
    fn metadata_preserves_source_facts_without_creating_a_callable_endpoint() {
        let row = parse_metadata(
            &entry(),
            b"endpoint: http://bidder.invalid/auction\ndisabled: false\ncapabilities: {}\n",
        )
        .unwrap();
        assert_eq!(
            row.endpoint.as_deref(),
            Some("http://bidder.invalid/auction")
        );
        assert!(!row.source_disabled);
        assert_eq!(
            row.metadata_fields,
            ["capabilities", "disabled", "endpoint"]
        );
        let value = toml::Value::try_from(row).unwrap();
        assert!(value.get("callable_endpoint").is_none());
        assert!(value.get("migrated").is_none());
    }

    #[test]
    fn metadata_rejects_duplicate_keys_wrong_types_and_nonobject_documents() {
        for bytes in [
            &b"endpoint: http://a.invalid\nendpoint: https://b.invalid\n"[..],
            b"disabled: perhaps\n",
            b"aliasOf: [fake_example]\n",
            b"endpoint: [https://example.invalid]\n",
            b"gvlVendorID: -1\n",
            b"gvlVendorID: 65536\n",
            b"userSync: []\n",
            b"- not\n- an_object\n",
        ] {
            assert!(matches!(
                parse_metadata(&entry(), bytes),
                Err(Error::InvalidSource)
            ));
        }
    }

    #[test]
    fn metadata_preflight_precedes_the_inventory_deserialization() {
        assert!(matches!(
            parse_metadata(&entry(), b"endpoint: &url https://example.invalid\n"),
            Err(Error::InvalidSource)
        ));
        let deep = format!("unused: {}0{}\n", "[".repeat(32), "]".repeat(32));
        assert!(matches!(
            parse_metadata(&entry(), deep.as_bytes()),
            Err(Error::SourceLimit)
        ));
        let many = format!("unused: [{}]\n", vec!["0"; 8_190].join(","));
        assert!(matches!(
            parse_metadata(&entry(), many.as_bytes()),
            Err(Error::SourceLimit)
        ));
        let large = format!(
            "#{}\nendpoint: https://example.invalid\n",
            "x".repeat(0x0001_0000)
        );
        assert!(matches!(
            parse_metadata(&entry(), large.as_bytes()),
            Err(Error::SourceLimit)
        ));
    }
}
