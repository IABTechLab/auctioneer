//! Build-only structural ownership for bidder-owned manifests, not migration evidence.
//!
//! Root: key, factory, `required_feature`?, `native_only`?, endpoint?, settings?, definitions?, provenance.
//! Provenance: { origin = "local" }, or { origin = "reference", sources,
//! trees?, artifacts? }. Sources name immutable repository/revision pairs.
//! Trees witness source tree IDs without pretending they are byte-addressed blobs.
//! Artifacts witness source blobs and optionally a retained local copy with
//! `source_path`, `source_bytes` and `source_sha256` in its own nested table.
//! Transformation and port-difference notes are associations, not verified receipts.

pub use crate::bidder_manifest as model;
use crate::endpoint_contract;
pub use model::{Artifact, Category, Manifest, Provenance, Source};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{self, File, Metadata};
use std::io::{self, Read as _};
use std::path::{Component, Path, PathBuf};
use std::str::from_utf8;
use syn::Ident;
use url::Url;

// Native build inputs use the existing reference import's per-file ceiling.
const MAX_FILE_BYTES: u64 = 1 << 20;

#[derive(Debug)]
pub struct Catalog {
    pub manifests: BTreeMap<String, Manifest>,
    pub input_paths: BTreeSet<PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    Input { path: PathBuf, message: String },
    Manifest { path: PathBuf },
    Contract { key: String, message: String },
    DuplicatePublicName { first: String, second: String },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Manifest { path, .. } => {
                write!(f, "{}: invalid bidder manifest", path.display())
            }
            Self::Contract { key, message } => write!(f, "bidder {key}: {message}"),
            Self::DuplicatePublicName { first, second } => {
                write!(f, "duplicate public bidder identity: {first} and {second}")
            }
        }
    }
}

/// Read and validate one bidder-owned manifest and its retained provenance.
///
/// # Errors
/// Refuses unreadable inputs, malformed manifests, and invalid ownership contracts.
pub fn read_manifest(
    directory: &Path,
    declared_features: &BTreeSet<String>,
) -> Result<Manifest, Refusal> {
    require_directory(directory)?;
    let path = directory.join("bidder.toml");
    let bytes = read_file(&path)?;
    let text = from_utf8(&bytes).map_err(|_error| Refusal::Manifest { path: path.clone() })?;
    let manifest: Manifest = toml::from_str(text).map_err(|_error| Refusal::Manifest { path })?;
    let directory_key = directory
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| valid_key(name))
        .unwrap_or("invalid_directory");
    let fail = |message: &'static str| Refusal::Contract {
        key: directory_key.to_owned(),
        message: message.to_owned(),
    };
    if !valid_key(&manifest.key) {
        return Err(fail("invalid bidder key"));
    }
    if let Some(endpoint) = &manifest.endpoint {
        if endpoint.fields.is_empty() {
            return Err(fail(
                "endpoint.fields: must declare at least one field; omit endpoint when unused",
            ));
        }
        if endpoint.fields.keys().any(|field| !valid_key(field)) {
            return Err(fail(
                "endpoint.fields: names must be lowercase nonkeyword Rust identifiers",
            ));
        }
        if !endpoint_contract::valid_placeholders(endpoint.fields.values().map(String::as_str)) {
            return Err(fail(
                "endpoint.fields: invalid or duplicate placeholder name",
            ));
        }
    }
    if manifest
        .settings
        .as_ref()
        .is_some_and(|settings| settings.fields.is_empty())
    {
        return Err(fail(
            "settings.fields: must declare at least one field; omit settings when unused",
        ));
    }
    if let Some(settings) = &manifest.settings
        && settings.fields.keys().any(|field| !valid_key(field))
    {
        return Err(fail(
            "settings.fields: names must be lowercase nonkeyword Rust identifiers",
        ));
    }
    if matches!(manifest.key.as_str(), "harness" | "tests") {
        return Err(fail("reserved bidder key"));
    }
    if directory.file_name().and_then(|name| name.to_str()) != Some(&manifest.key) {
        return Err(fail("bidder directory does not match key"));
    }
    if let Some(feature) = &manifest.required_feature
        && (!declared_features.contains(feature)
            || matches!(feature.as_str(), "default" | "test-harness-override"))
    {
        return Err(fail(
            "required_feature must name an allowed declared feature",
        ));
    }
    let module = directory.join("mod.rs");
    if manifest.factory {
        require_file(&module)?;
    } else {
        match fs::symlink_metadata(&module) {
            Ok(_) => return Err(fail("factory=false forbids mod.rs")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_error) => return Err(input(&module, "cannot read input")),
        }
        if !manifest.definitions.is_empty() {
            return Err(fail("factory=false forbids effective definitions"));
        }
    }
    for definition in manifest.definitions.values() {
        for (field, expected, reason) in [
            (
                "implementation",
                Some(manifest.key.as_str()),
                "definitions.implementation must match the owning key",
            ),
            (
                "required_feature",
                manifest.required_feature.as_deref(),
                "definitions.required_feature must match the owning feature",
            ),
        ] {
            if let Some(value) = definition.get(field)
                && (expected.is_none() || value.as_str() != expected)
            {
                return Err(fail(reason));
            }
        }
    }
    validate_provenance(directory, &manifest)?;
    Ok(manifest)
}

/// Scan the bidder source tree, retaining every input path for rebuild tracking.
///
/// # Errors
/// Refuses invalid inputs, conflicting public identities, and invalid manifests.
pub fn scan(root: &Path, declared_features: &BTreeSet<String>) -> Result<Catalog, Refusal> {
    require_directory(root)?;
    let mut input_paths = BTreeSet::from([root.to_owned()]);
    let mut manifests = BTreeMap::new();
    let mut owners = BTreeMap::<String, String>::new();
    let mut helpers = BTreeSet::new();
    for path in children(root)? {
        let metadata = input_metadata(&path)?;
        input_paths.insert(path.clone());
        if metadata.is_dir() {
            track_tree(&path, &mut input_paths)?;
            let manifest = read_manifest(&path, declared_features)?;
            for name in manifest.definitions.keys() {
                let folded = name.to_ascii_lowercase();
                if let Some(first) = owners.get(&folded) {
                    return Err(Refusal::DuplicatePublicName {
                        first: first.clone(),
                        second: manifest.key.clone(),
                    });
                }
                owners.insert(folded, manifest.key.clone());
            }
            manifests.insert(manifest.key.clone(), manifest);
            continue;
        }
        if path.extension().is_some_and(|extension| extension == "rs")
            && path.file_name().is_some_and(|name| name != "mod.rs")
        {
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .ok_or_else(|| input(&path, "non-UTF-8 helper name"))?;
            helpers.insert(name.to_owned());
        }
    }
    if let Some(key) = manifests.keys().find(|key| helpers.contains(*key)) {
        return Err(Refusal::Contract {
            key: key.clone(),
            message: "bidder key collides with helper module".into(),
        });
    }
    validate_aliases(&manifests)?;
    Ok(Catalog {
        manifests,
        input_paths,
    })
}

fn validate_aliases(manifests: &BTreeMap<String, Manifest>) -> Result<(), Refusal> {
    let definitions: BTreeMap<_, _> = manifests
        .values()
        .flat_map(|owner| {
            owner
                .definitions
                .iter()
                .map(move |(name, row)| (name.as_str(), (owner, row)))
        })
        .collect();
    for (&name, &(owner, _)) in &definitions {
        let fail = |message: &str| Refusal::Contract {
            key: owner.key.clone(),
            message: format!("definition {name}: {message}"),
        };
        let mut current = name;
        let mut visited = BTreeSet::new();
        loop {
            if !visited.insert(current) {
                return Err(fail("aliasOf contains a cycle"));
            }
            let &(child_owner, row) = definitions
                .get(current)
                .ok_or_else(|| fail("aliasOf parent is absent"))?;
            let Some(parent_value) = row.get("aliasOf") else {
                break;
            };
            let parent = parent_value
                .as_str()
                .ok_or_else(|| fail("aliasOf must name a public definition"))?;
            let (parent_owner, _) = definitions
                .get(parent)
                .ok_or_else(|| fail("aliasOf parent is absent"))?;
            if child_owner.native_only != parent_owner.native_only
                || child_owner.required_feature != parent_owner.required_feature
            {
                return Err(fail(
                    "aliasOf must retain its parent's target and feature predicates",
                ));
            }
            if child_owner.key != parent_owner.key {
                return Err(fail(
                    "aliasOf must retain its parent's implementation owner",
                ));
            }
            current = parent;
        }
    }
    Ok(())
}

fn valid_key(key: &str) -> bool {
    key != "gen"
        && key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && syn::parse_str::<Ident>(key).is_ok()
}

fn normal_path(path: &Path, absolute: bool) -> bool {
    path.is_absolute() == absolute
        && !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
        && path.components().collect::<PathBuf>().as_os_str() == path.as_os_str()
}

fn local_path(value: &str) -> bool {
    !value.contains('\\') && normal_path(Path::new(value), false)
}

fn hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        && value.bytes().any(|byte| byte != b'0')
}

fn validate_provenance(directory: &Path, manifest: &Manifest) -> Result<(), Refusal> {
    let Provenance::Reference {
        sources,
        trees,
        artifacts,
    } = &manifest.provenance
    else {
        return Ok(());
    };
    let fail = |message: &'static str| Refusal::Contract {
        key: manifest.key.clone(),
        message: message.to_owned(),
    };
    if sources.is_empty() {
        return Err(fail("reference provenance requires sources"));
    }
    if trees.is_empty() && artifacts.is_empty() {
        return Err(fail(
            "reference provenance requires tree or artifact witnesses",
        ));
    }
    for source in sources.values() {
        let valid = Url::parse(&source.repository).is_ok_and(|url| {
            url.scheme() == "https"
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
        });
        if !valid {
            return Err(fail(
                "provenance.repository must be an absolute HTTPS repository without credentials, query or fragment",
            ));
        }
        if !hex(&source.revision, 40) {
            return Err(fail(
                "provenance.revision must be a full immutable Git object ID",
            ));
        }
    }
    let mut identities = BTreeSet::new();
    for tree in trees {
        validate_source(&manifest.key, sources, &tree.source, &tree.source_path)?;
        if !hex(&tree.tree, 40) {
            return Err(fail(
                "provenance.tree must be a full immutable Git object ID",
            ));
        }
        if !identities.insert((&tree.source, &tree.source_path)) {
            return Err(fail("duplicate source artifact"));
        }
    }
    for artifact in artifacts {
        validate_source(
            &manifest.key,
            sources,
            &artifact.source,
            &artifact.source_path,
        )?;
        if !hex(&artifact.source_blob, 40) {
            return Err(fail(
                "provenance.source_blob must be a full immutable Git object ID",
            ));
        }
        if !hex(&artifact.source_sha256, 64) {
            return Err(fail("provenance.source_sha256 must be lowercase SHA256"));
        }
        if !identities.insert((&artifact.source, &artifact.source_path)) {
            return Err(fail("duplicate source artifact"));
        }
        validate_retained(directory, &manifest.key, artifact)?;
    }
    Ok(())
}

fn validate_retained(directory: &Path, key: &str, artifact: &Artifact) -> Result<(), Refusal> {
    let fail = |message: &'static str| Refusal::Contract {
        key: key.to_owned(),
        message: message.to_owned(),
    };
    let Some(retained) = &artifact.retained else {
        return if artifact.category == Category::Schema {
            Err(fail("schema requires a retained original"))
        } else {
            Ok(())
        };
    };
    if !local_path(&retained.path) {
        return Err(fail(
            "provenance.retained.source_path must be a normal relative path",
        ));
    }
    if retained.path == "bidder.toml" {
        return Err(fail("manifest self-hash is forbidden"));
    }
    if !hex(&retained.sha256, 64) {
        return Err(fail(
            "provenance.retained.source_sha256 must be lowercase SHA256",
        ));
    }
    let changed =
        retained.bytes != artifact.source_bytes || retained.sha256 != artifact.source_sha256;
    if changed && matches!(artifact.category, Category::Schema | Category::License) {
        return Err(fail("schema and license captures must be verbatim"));
    }
    if changed
        && artifact
            .transformation
            .as_ref()
            .is_none_or(|note| note.trim().is_empty())
    {
        return Err(fail("changed retained output requires transformation"));
    }
    let path = directory.join(&retained.path);
    let location = directory.join("bidder.toml");
    let bytes = read_file(&path).map_err(|error| match error {
        Refusal::Input { message, .. } => Refusal::Input {
            path: location.clone(),
            message,
        },
        other @ (Refusal::Manifest { .. }
        | Refusal::Contract { .. }
        | Refusal::DuplicatePublicName { .. }) => other,
    })?;
    if u64::try_from(bytes.len()).ok() != Some(retained.bytes) {
        return Err(input(&location, "retained byte length mismatch"));
    }
    if format!("{:x}", Sha256::digest(bytes)) != retained.sha256 {
        return Err(input(&location, "retained SHA256 mismatch"));
    }
    Ok(())
}

fn validate_source(
    key: &str,
    sources: &BTreeMap<String, Source>,
    source: &str,
    path: &str,
) -> Result<(), Refusal> {
    let message = if !sources.contains_key(source) {
        Some("provenance.source must name a declared source")
    } else if !local_path(path) {
        Some("provenance.source_path must be a normal relative path")
    } else {
        None
    };
    match message {
        Some(detail) => Err(Refusal::Contract {
            key: key.to_owned(),
            message: detail.to_owned(),
        }),
        None => Ok(()),
    }
}

fn input(path: &Path, message: &str) -> Refusal {
    Refusal::Input {
        path: path.to_owned(),
        message: message.to_owned(),
    }
}

fn input_metadata(path: &Path) -> Result<Metadata, Refusal> {
    if !normal_path(path, true) {
        return Err(input(path, "non-normal absolute path"));
    }
    let mut prefix = PathBuf::new();
    for part in path.components() {
        prefix.push(part);
        let metadata = fs::symlink_metadata(&prefix).map_err(|error| {
            input(
                &prefix,
                if error.kind() == io::ErrorKind::NotFound {
                    "missing input"
                } else {
                    "cannot read input"
                },
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(input(&prefix, "symlink input"));
        }
        if !metadata.is_file() && !metadata.is_dir() {
            return Err(input(&prefix, "nonregular input"));
        }
    }
    fs::symlink_metadata(path).map_err(|_error| input(path, "cannot read input"))
}

fn require_directory(path: &Path) -> Result<(), Refusal> {
    if !input_metadata(path)?.is_dir() {
        return Err(input(path, "expected directory"));
    }
    Ok(())
}

fn require_file(path: &Path) -> Result<(), Refusal> {
    if !input_metadata(path)?.is_file() {
        return Err(input(path, "expected regular file"));
    }
    Ok(())
}

fn read_file(path: &Path) -> Result<Vec<u8>, Refusal> {
    require_file(path)?;
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_error| input(path, "cannot read input"))?
        .take(MAX_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_error| input(path, "cannot read input"))?;
    if u64::try_from(bytes.len()).map_or(true, |length| length > MAX_FILE_BYTES) {
        return Err(input(path, "input byte limit"));
    }
    Ok(bytes)
}

fn children(path: &Path) -> Result<BTreeSet<PathBuf>, Refusal> {
    fs::read_dir(path)
        .map_err(|_error| input(path, "cannot read directory"))?
        .map(|entry| {
            entry
                .map(|child| child.path())
                .map_err(|_error| input(path, "cannot read directory"))
        })
        .collect()
}

fn track_tree(path: &Path, paths: &mut BTreeSet<PathBuf>) -> Result<(), Refusal> {
    for child in children(path)? {
        if input_metadata(&child)?.is_dir() {
            track_tree(&child, paths)?;
        }
        paths.insert(child);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Category, Provenance, Refusal, read_manifest, scan};
    use sha2::{Digest as _, Sha256};
    use std::collections::BTreeSet;
    use std::env::temp_dir;
    use std::fs;
    use std::io::ErrorKind;
    use std::path::{Path, PathBuf};
    use std::process::id;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const REVISION: &str = "1111111111111111111111111111111111111111";
    const BLOB: &str = "2222222222222222222222222222222222222222";
    const TREE: &str = "3333333333333333333333333333333333333333";
    const CONTENT: &[u8] = b"original\n";
    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        #[expect(
            clippy::create_dir,
            reason = "exclusive fixture ownership requires create_dir to reject an existing path"
        )]
        fn new() -> Self {
            let parent = temp_dir().canonicalize().unwrap();
            loop {
                let root = parent.join(format!(
                    "mc-{}-{}",
                    id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                match fs::create_dir(&root) {
                    Ok(()) => return Self(root),
                    Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("create fixture: {error}"),
                }
            }
        }

        fn bidder(&self, key: &str, factory: bool, suffix: &str) -> PathBuf {
            let directory = self.0.join(key);
            fs::create_dir_all(&directory).unwrap();
            fs::write(
                directory.join("bidder.toml"),
                format!("key = '{key}'\nfactory = {factory}\n{suffix}"),
            )
            .unwrap();
            if factory {
                fs::write(directory.join("mod.rs"), "not parsed or executed").unwrap();
            }
            directory
        }

        fn local(&self, key: &str, factory: bool) -> PathBuf {
            self.bidder(key, factory, "[provenance]\norigin = 'local'\n")
        }

        fn reference(&self, category: &str) -> PathBuf {
            let directory = self.bidder(
                "fake_probe",
                false,
                &reference(&artifact(category, "capture", CONTENT)),
            );
            fs::write(directory.join("capture"), CONTENT).unwrap();
            directory
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _removed = fs::remove_dir_all(&self.0);
        }
    }

    fn digest(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn public_aliases_require_present_acyclic_and_consistently_gated_parents() {
        for definitions in [
            "[definitions.fake_alias]\naliasOf = 'fake_absent'\n",
            "[definitions.fake_alias]\naliasOf = 'fake_base'\n[definitions.fake_base]\naliasOf = 'fake_alias'\n",
        ] {
            let fixture = Fixture::new();
            fixture.bidder(
                "fake_owner",
                true,
                &format!("{definitions}[provenance]\norigin = 'local'\n"),
            );
            scan(&fixture.0, &BTreeSet::new()).expect_err("missing or cyclic alias parent");
        }
        for predicate in [
            "native_only = true\n",
            "required_feature = 'experimental'\n",
        ] {
            let fixture = Fixture::new();
            fixture.bidder(
                "fake_base_owner",
                true,
                "[definitions.fake_base]\n[provenance]\norigin = 'local'\n",
            );
            fixture.bidder("fake_alias_owner", true, &format!("{predicate}[definitions.fake_alias]\naliasOf = 'fake_base'\n[provenance]\norigin = 'local'\n"));
            scan(&fixture.0, &BTreeSet::from(["experimental".to_owned()]))
                .expect_err("inconsistent alias publication predicates");
        }
    }

    #[test]
    fn native_only_is_optional_and_does_not_require_publication() {
        let fixture = Fixture::new();
        fixture.local("fake_legacy", true);
        fixture.bidder(
            "fake_candidate",
            true,
            "native_only = true\n[provenance]\norigin = 'local'\n",
        );
        let catalog = scan(&fixture.0, &BTreeSet::new()).unwrap();
        assert!(!catalog.manifests["fake_legacy"].native_only);
        assert!(catalog.manifests["fake_candidate"].native_only);
        assert!(catalog.manifests["fake_candidate"].definitions.is_empty());
    }

    #[test]
    fn compiled_alias_chain_can_share_consistent_native_and_feature_predicates() {
        let fixture = Fixture::new();
        fixture.bidder("fake_owner", true, "native_only = true\nrequired_feature = 'experimental'\n[definitions.fake_base]\n[definitions.fake_middle]\naliasOf = 'fake_base'\n[definitions.fake_leaf]\naliasOf = 'fake_middle'\n[provenance]\norigin = 'local'\n");
        let catalog = scan(&fixture.0, &BTreeSet::from(["experimental".to_owned()])).unwrap();
        assert_eq!(catalog.manifests["fake_owner"].definitions.len(), 3);
    }

    fn reference(witnesses: &str) -> String {
        format!(
            "[provenance]\norigin = 'reference'\n\
             [provenance.sources.upstream]\n\
             repository = 'https://example.test/upstream.git'\nrevision = '{REVISION}'\n\
             {witnesses}"
        )
    }

    fn artifact(category: &str, path: &str, bytes: &[u8]) -> String {
        format!(
            "[[provenance.artifacts]]\ncategory = '{category}'\nsource = 'upstream'\n\
             source_path = 'external/{category}'\nsource_blob = '{BLOB}'\n\
             source_bytes = {}\nsource_sha256 = '{}'\n\
             [provenance.artifacts.retained]\nsource_path = '{path}'\nsource_bytes = {}\nsource_sha256 = '{}'\n",
            bytes.len(),
            digest(bytes),
            bytes.len(),
            digest(bytes),
        )
    }

    fn read(directory: &Path) -> Result<super::Manifest, Refusal> {
        read_manifest(directory, &BTreeSet::new())
    }

    fn contract(message: &str) -> Refusal {
        Refusal::Contract {
            key: "fake_probe".into(),
            message: message.into(),
        }
    }

    fn input(path: &Path, message: &str) -> Refusal {
        Refusal::Input {
            path: path.into(),
            message: message.into(),
        }
    }

    fn parse_error(directory: &Path) -> Refusal {
        Refusal::Manifest {
            path: directory.join("bidder.toml"),
        }
    }

    fn rewrite(directory: &Path, from: &str, to: &str) {
        let path = directory.join("bidder.toml");
        let original = fs::read_to_string(&path).unwrap();
        assert!(original.contains(from), "mutation must alter the fixture");
        fs::write(path, original.replacen(from, to, 1)).unwrap();
    }

    fn rewrite_retained(directory: &Path, from: &str, to: &str) {
        let path = directory.join("bidder.toml");
        let original = fs::read_to_string(&path).unwrap();
        let marker = "[provenance.artifacts.retained]\n";
        let (source, retained) = original.split_once(marker).unwrap();
        assert!(
            retained.contains(from),
            "mutation must alter the retained witness"
        );
        fs::write(
            path,
            format!("{source}{marker}{}", retained.replacen(from, to, 1)),
        )
        .unwrap();
    }

    #[test]
    fn endpoint_fields_accept_arbitrary_owned_names_without_promoting_a_factory() {
        let fixture = Fixture::new();
        let directory = fixture.bidder(
            "fake_probe",
            false,
            "[endpoint.fields]\nroute_id = 'RouteID'\nzone = 'Zone9'\n[provenance]\norigin = 'local'\n",
        );
        let manifest = read(&directory).unwrap();
        assert!(!manifest.factory);
        assert!(manifest.definitions.is_empty());
        let serialized = toml::to_string(&manifest).unwrap();
        let value: toml::Value = toml::from_str(&serialized).unwrap();
        assert_eq!(
            value["endpoint"]["fields"]["route_id"].as_str(),
            Some("RouteID")
        );
        assert_eq!(value["endpoint"]["fields"]["zone"].as_str(), Some("Zone9"));
    }

    #[test]
    fn endpoint_fields_reject_empty_declarations_with_an_omit_control() {
        let fixture = Fixture::new();
        let absent = fixture.local("fake_probe", false);
        read(&absent).unwrap();
        for table in ["[endpoint]", "[endpoint.fields]"] {
            let row = Fixture::new();
            let directory = row.bidder(
                "fake_probe",
                false,
                &format!("{table}\n[provenance]\norigin = 'local'\n"),
            );
            assert_eq!(
                read(&directory).unwrap_err(),
                contract(
                    "endpoint.fields: must declare at least one field; omit endpoint when unused"
                )
            );
        }
    }

    #[test]
    fn endpoint_field_keys_must_be_lowercase_nonkeyword_rust_identifiers() {
        for field in [
            "", "type", "gen", "self", "_", "route-id", "9route", "Bad", "r#type",
        ] {
            let fixture = Fixture::new();
            let directory = fixture.bidder(
                "fake_probe",
                false,
                &format!(
                    "[endpoint.fields]\n'{field}' = 'RouteID'\n[provenance]\norigin = 'local'\n"
                ),
            );
            assert_eq!(
                read(&directory).unwrap_err(),
                contract("endpoint.fields: names must be lowercase nonkeyword Rust identifiers")
            );
        }
    }

    #[test]
    fn endpoint_placeholders_refuse_malformed_reserved_and_duplicate_names() {
        for fields in [
            "a = ''",
            "a = '9Route'",
            "a = 'route-id'",
            "a = 'Rout\u{e9}'",
            "a = '{{.Route}}'",
            "a = 'Route ID'",
            "a = 'Host'",
            "a = 'GDPR'",
            "a = 'GDPRConsent'",
            "a = 'USPrivacy'",
            "a = 'GPP'",
            "a = 'GPPSID'",
            "a = 'RedirectURL'",
            "a = 'RouteID'\nb = 'RouteID'",
            "a = 'ExternalURL'",
            "a = 'SyncerKey'",
            "a = 'BidderName'",
            "a = 'SyncType'",
            "a = 'UserMacro'",
        ] {
            let fixture = Fixture::new();
            let directory = fixture.bidder(
                "fake_probe",
                false,
                &format!("[endpoint.fields]\n{fields}\n[provenance]\norigin = 'local'\n"),
            );
            assert_eq!(
                read(&directory).unwrap_err(),
                contract("endpoint.fields: invalid or duplicate placeholder name"),
                "{fields}"
            );
        }
    }

    #[test]
    fn endpoint_declaration_rejects_unknown_keys_and_nonstring_bindings() {
        for declaration in [
            "[endpoint]\nfieldz = {}",
            "[endpoint.fields]\na = 1",
            "[endpoint.fields]\na = { name = 'RouteID' }",
            "endpoint = ['RouteID']",
        ] {
            let fixture = Fixture::new();
            let directory = fixture.bidder(
                "fake_probe",
                false,
                &format!("{declaration}\n[provenance]\norigin = 'local'\n"),
            );
            assert_eq!(read(&directory).unwrap_err(), parse_error(&directory));
        }
    }

    #[test]
    fn settings_allow_only_two_explicit_optional_kinds_without_promoting_a_factory() {
        let fixture = Fixture::new();
        let directory = fixture.bidder(
            "fake_probe",
            false,
            "[settings.fields]\nname = 'optional_string'\nnumber = 'optional_i64'\n\
             [provenance]\norigin = 'local'\n",
        );
        let manifest = read(&directory).unwrap();
        assert!(!manifest.factory);
        assert!(manifest.definitions.is_empty());
        assert_eq!(manifest.settings.as_ref().unwrap().fields.len(), 2);
        let serialized = toml::to_string(&manifest).unwrap();
        let reparsed: super::Manifest = toml::from_str(&serialized).unwrap();
        assert_eq!(reparsed.settings.unwrap().fields.len(), 2);
    }

    #[test]
    fn absent_settings_schema_is_accepted() {
        let fixture = Fixture::new();
        let absent = fixture.local("fake_absent", false);
        assert!(read(&absent).unwrap().settings.is_none());
    }

    #[test]
    fn settings_reject_empty_declarations_with_an_omit_control() {
        for (key, declaration) in [
            ("fake_empty", "[settings]"),
            ("fake_fields", "[settings.fields]"),
        ] {
            let fixture = Fixture::new();
            let directory = fixture.bidder(
                key,
                false,
                &format!("{declaration}\n[provenance]\norigin = 'local'\n"),
            );
            assert_eq!(
                read(&directory).unwrap_err(),
                Refusal::Contract {
                    key: key.to_owned(),
                    message: "settings.fields: must declare at least one field; omit settings when unused".to_owned(),
                }
            );
        }
    }

    #[test]
    fn settings_schema_rejects_unknown_keys_kinds_and_nontyped_declarations() {
        for declaration in [
            "[settings]\nfieldz = {}",
            "[settings.fields]\nname = 'optional_bool'",
            "[settings.fields]\nname = 'String'",
            "[settings.fields]\nname = 1",
            "[settings.fields]\nname = { kind = 'optional_i64', min = 1 }",
        ] {
            let fixture = Fixture::new();
            let directory = fixture.bidder(
                "fake_probe",
                false,
                &format!("{declaration}\n[provenance]\norigin = 'local'\n"),
            );
            assert_eq!(read(&directory).unwrap_err(), parse_error(&directory));
        }
    }

    #[test]
    fn settings_field_names_must_be_safe_generated_identifiers() {
        for field in [
            "",
            "type",
            "gen",
            "self",
            "_",
            "hb-source",
            "9source",
            "Bad",
            "r#type",
        ] {
            let fixture = Fixture::new();
            let directory = fixture.bidder(
                "fake_probe",
                false,
                &format!(
                    "[settings.fields]\n'{field}' = 'optional_i64'\n\
                 [provenance]\norigin = 'local'\n"
                ),
            );
            assert_eq!(
                read(&directory).unwrap_err(),
                contract("settings.fields: names must be lowercase nonkeyword Rust identifiers")
            );
        }
    }

    #[test]
    fn compiled_candidate_has_no_required_public_definition() {
        let fixture = Fixture::new();
        let directory = fixture.local("fake_probe", true);
        let manifest = read(&directory).unwrap();
        assert_eq!(manifest.key, "fake_probe");
        assert!(manifest.factory);
        assert!(manifest.required_feature.is_none());
        assert!(manifest.definitions.is_empty());
        assert!(matches!(manifest.provenance, Provenance::Local {}));
    }

    #[test]
    fn local_reference_only_role_does_not_infer_factory_or_definitions() {
        let fixture = Fixture::new();
        let manifest = read(&fixture.local("fake_probe", false)).unwrap();
        assert!(!manifest.factory);
        assert!(manifest.definitions.is_empty());
    }

    #[test]
    fn exact_public_names_and_raw_metadata_remain_config_owned() {
        let fixture = Fixture::new();
        let directory = fixture.bidder(
            "fake_probe",
            true,
            "[provenance]\norigin = 'local'\n\
             [definitions.'fake_360-Public']\nendpoint = 'https://bid.test'\n\
             renderer_owned = { arbitrary = [1, 2] }\n\
             [definitions.fake_Alias]\nimplementation = 'fake_probe'\n",
        );
        let manifest = read(&directory).unwrap();
        assert_eq!(
            manifest
                .definitions
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["fake_360-Public", "fake_Alias"]
        );
        assert_eq!(
            manifest.definitions["fake_360-Public"]["renderer_owned"]["arbitrary"][1].as_integer(),
            Some(2)
        );
    }

    #[test]
    fn factory_is_required_and_boolean_without_inferred_default() {
        for (from, to) in [
            ("factory = true\n", ""),
            ("factory = true", "factory = 'true'"),
        ] {
            let fixture = Fixture::new();
            let directory = fixture.local("fake_probe", true);
            rewrite(&directory, from, to);
            assert_eq!(read(&directory).unwrap_err(), parse_error(&directory));
        }
    }

    #[test]
    fn executable_role_requires_regular_mod_rs() {
        for directory_instead in [false, true] {
            let fixture = Fixture::new();
            let directory = fixture.local("fake_probe", true);
            let module = directory.join("mod.rs");
            fs::remove_file(&module).unwrap();
            let message = if directory_instead {
                fs::create_dir_all(&module).unwrap();
                "expected regular file"
            } else {
                "missing input"
            };
            assert_eq!(read(&directory).unwrap_err(), input(&module, message));
        }
    }

    #[test]
    fn nonfactory_role_forbids_mod_rs_even_without_definitions() {
        let fixture = Fixture::new();
        let directory = fixture.local("fake_probe", false);
        fs::write(directory.join("mod.rs"), "").unwrap();
        assert_eq!(
            read(&directory).unwrap_err(),
            contract("factory=false forbids mod.rs")
        );
    }

    #[test]
    fn nonfactory_role_forbids_effective_definitions_without_module() {
        let fixture = Fixture::new();
        let directory = fixture.bidder(
            "fake_probe",
            false,
            "[provenance]\norigin = 'local'\n[definitions.fake_imported]\n",
        );
        assert_eq!(
            read(&directory).unwrap_err(),
            contract("factory=false forbids effective definitions")
        );
    }

    #[test]
    fn unknown_root_fields_and_duplicate_keys_are_refused() {
        for field in ["source", "module", "constructor", "manifest_sha256"] {
            let fixture = Fixture::new();
            let directory = fixture.local("fake_probe", true);
            rewrite(
                &directory,
                "[provenance]",
                &format!("{field} = 'arbitrary'\n[provenance]"),
            );
            assert_eq!(read(&directory).unwrap_err(), parse_error(&directory));
        }
        let fixture = Fixture::new();
        let directory = fixture.local("fake_probe", true);
        rewrite(
            &directory,
            "factory = true",
            "factory = true\nfactory = false",
        );
        assert_eq!(read(&directory).unwrap_err(), parse_error(&directory));
    }

    #[test]
    fn nested_provenance_structs_reject_unknown_fields() {
        for (from, to) in [
            ("revision =", "unknown = true\nrevision ="),
            ("source_bytes =", "unknown = true\nsource_bytes ="),
            (
                "source_path = 'capture'",
                "source_path = 'capture'\nunknown = true",
            ),
        ] {
            let fixture = Fixture::new();
            let directory = fixture.reference("schema");
            rewrite(&directory, from, to);
            assert_eq!(read(&directory).unwrap_err(), parse_error(&directory));
        }
    }

    #[test]
    fn keys_are_folder_matching_lowercase_rust_identifiers() {
        for key in [
            "",
            "_",
            "Probe",
            "1probe",
            "bad-key",
            "r#type",
            "../probe",
            "type",
            "async",
            "gen",
            "self",
            "true",
            "try",
            "yield",
            "\u{fc}ber",
        ] {
            let fixture = Fixture::new();
            let directory = fixture.local("fake_probe", true);
            rewrite(&directory, "key = 'fake_probe'", &format!("key = '{key}'"));
            assert_eq!(
                read(&directory).unwrap_err(),
                Refusal::Contract {
                    key: "fake_probe".into(),
                    message: "invalid bidder key".into()
                }
            );
        }
        let fixture = Fixture::new();
        let directory = fixture.local("fake_different", true);
        rewrite(&directory, "key = 'fake_different'", "key = 'fake_probe'");
        assert_eq!(
            read(&directory).unwrap_err(),
            Refusal::Contract {
                key: "fake_different".into(),
                message: "bidder directory does not match key".into()
            }
        );
    }

    #[test]
    fn declared_features_are_checked_even_without_feature_filtering() {
        let fixture = Fixture::new();
        let directory = fixture.local("fake_probe", true);
        rewrite(
            &directory,
            "[provenance]",
            "required_feature = 'experimental'\n[provenance]",
        );
        assert_eq!(
            read(&directory).unwrap_err(),
            contract("required_feature must name an allowed declared feature")
        );
        let manifest = read_manifest(&directory, &BTreeSet::from(["experimental".into()])).unwrap();
        assert_eq!(manifest.required_feature.as_deref(), Some("experimental"));
    }

    #[test]
    fn duplicated_metadata_ownership_values_must_match() {
        for (field, value) in [
            ("implementation", "'other'"),
            ("implementation", "true"),
            ("required_feature", "'other'"),
            ("required_feature", "false"),
        ] {
            let fixture = Fixture::new();
            let directory = fixture.bidder(
                "fake_probe",
                true,
                &format!(
                    "[provenance]\norigin = 'local'\n[definitions.fake_Public]\n{field} = {value}\n"
                ),
            );
            assert_eq!(
                read(&directory).unwrap_err(),
                contract(if field == "implementation" {
                    "definitions.implementation must match the owning key"
                } else {
                    "definitions.required_feature must match the owning feature"
                })
            );
        }
    }

    #[test]
    fn all_artifact_categories_have_independent_retained_byte_witnesses() {
        for (category, expected) in [
            ("implementation", Category::Implementation),
            ("helper", Category::Helper),
            ("schema", Category::Schema),
            ("metadata", Category::Metadata),
            ("protocol", Category::Protocol),
            ("parameter", Category::Parameter),
            ("license", Category::License),
        ] {
            let fixture = Fixture::new();
            let directory = fixture.reference(category);
            let manifest = read(&directory).unwrap();
            let Provenance::Reference { artifacts, .. } = manifest.provenance else {
                panic!("reference role lost")
            };
            assert_eq!(artifacts.len(), 1);
            assert_eq!(artifacts[0].category, expected);
            assert_eq!(artifacts[0].source_path, format!("external/{category}"));
            fs::write(directory.join("capture"), b"modified\n").unwrap();
            assert_eq!(
                read(&directory).unwrap_err(),
                input(&directory.join("bidder.toml"), "retained SHA256 mismatch")
            );
        }
    }

    #[test]
    fn malformed_manifest_diagnostics_do_not_retain_rejected_values() {
        const TOKEN: &str = "PASTED_SECRET_92ae7b";
        for (retained, field) in [
            (false, "factory"),
            (false, "category"),
            (false, "source_bytes"),
            (true, "source_bytes"),
        ] {
            let fixture = Fixture::new();
            let directory = fixture.reference("schema");
            let from = match field {
                "factory" => "factory = false".to_owned(),
                "category" => "category = 'schema'".to_owned(),
                _ => format!("source_bytes = {}", CONTENT.len()),
            };
            let to = format!("{field} = '{TOKEN}'");
            if retained {
                rewrite_retained(&directory, &from, &to);
            } else {
                rewrite(&directory, &from, &to);
            }
            let error = read(&directory).unwrap_err();
            assert!(matches!(error, Refusal::Manifest { .. }));
            assert!(!format!("{error:?}").contains(TOKEN));
            assert!(!error.to_string().contains(TOKEN));
        }
    }

    #[test]
    fn semantic_manifest_diagnostics_do_not_echo_rejected_values_or_references() {
        const TOKEN: &str = "PASTED_SECRET_92ae7b";
        for (from, to) in [
            ("key = 'fake_probe'".to_owned(), format!("key = '{TOKEN}'")),
            (
                "[provenance]".to_owned(),
                format!("required_feature = '{TOKEN}'\n[provenance]"),
            ),
            (
                "source = 'upstream'".to_owned(),
                format!("source = '{TOKEN}'"),
            ),
            (
                "source_path = 'external/schema'".to_owned(),
                format!("source_path = '../{TOKEN}'"),
            ),
            (
                "source_path = 'capture'".to_owned(),
                format!("source_path = '../{TOKEN}'"),
            ),
            (
                "source_path = 'capture'".to_owned(),
                format!("source_path = '{TOKEN}'"),
            ),
        ] {
            let fixture = Fixture::new();
            let directory = fixture.reference("schema");
            let before = read(&directory).unwrap();
            assert_eq!(before.key, "fake_probe");
            rewrite(&directory, &from, &to);
            let error = read(&directory).unwrap_err();
            assert!(!format!("{error:?}").contains(TOKEN));
            assert!(!error.to_string().contains(TOKEN));
        }
    }

    #[test]
    fn local_origin_cannot_claim_undeclared_external_sources() {
        let fixture = Fixture::new();
        let directory = fixture.local("fake_probe", true);
        rewrite(
            &directory,
            "origin = 'local'",
            "origin = 'local'\nsources = {}",
        );
        assert_eq!(read(&directory).unwrap_err(), parse_error(&directory));
    }

    #[test]
    fn reference_origin_requires_a_source_and_meaningful_witness() {
        for (suffix, message) in [
            (
                "[provenance]\norigin = 'reference'\nsources = {}\n".to_owned(),
                "reference provenance requires sources",
            ),
            (
                reference(""),
                "reference provenance requires tree or artifact witnesses",
            ),
        ] {
            let fixture = Fixture::new();
            let directory = fixture.bidder("fake_probe", false, &suffix);
            assert_eq!(read(&directory).unwrap_err(), contract(message));
        }
    }

    #[test]
    fn supplemental_source_and_tree_identity_are_preserved_without_fake_blob_fields() {
        let fixture = Fixture::new();
        let suffix = reference(&format!(
            "[provenance.sources.supplemental]\nrepository = 'https://example.test/supplemental.git'\nrevision = '{BLOB}'\n\
             [[provenance.trees]]\ncategory = 'implementation'\nsource = 'upstream'\nsource_path = 'adapters/probe'\ntree = '{TREE}'\n\
             port_differences = ['Rust sink is bounded']\n\
             [[provenance.trees]]\ncategory = 'helper'\nsource = 'supplemental'\nsource_path = 'shared/helper'\ntree = '{TREE}'\n"
        ));
        let directory = fixture.bidder("fake_probe", false, &suffix);
        let manifest = read(&directory).unwrap();
        let Provenance::Reference {
            sources,
            trees,
            artifacts,
        } = manifest.provenance
        else {
            panic!("reference role lost")
        };
        assert_eq!(sources.len(), 2);
        assert_eq!(sources["supplemental"].revision, BLOB);
        assert_eq!(trees.len(), 2);
        assert_eq!(trees[1].category, Category::Helper);
        assert_eq!(trees[0].port_differences, ["Rust sink is bounded"]);
        assert!(artifacts.is_empty());
    }

    #[test]
    fn transformation_notes_allow_distinct_raw_output_but_do_not_claim_port_proof() {
        let fixture = Fixture::new();
        let directory = fixture.reference("metadata");
        rewrite(
            &directory,
            "[provenance.artifacts.retained]",
            "transformation = 'YAML to JSON'\nport_differences = ['endpoint policy remains unreviewed']\n[provenance.artifacts.retained]",
        );
        rewrite_retained(
            &directory,
            &format!(
                "source_bytes = {}\nsource_sha256 = '{}'",
                CONTENT.len(),
                digest(CONTENT)
            ),
            &format!("source_bytes = 3\nsource_sha256 = '{}'", digest(b"{}\n")),
        );
        fs::write(directory.join("capture"), b"{}\n").unwrap();
        let manifest = read(&directory).unwrap();
        let Provenance::Reference { artifacts, .. } = manifest.provenance else {
            panic!("reference role lost")
        };
        assert_eq!(artifacts[0].transformation.as_deref(), Some("YAML to JSON"));
        assert_eq!(
            artifacts[0].port_differences,
            ["endpoint policy remains unreviewed"]
        );
    }

    #[test]
    fn changed_retained_output_requires_explicit_transformation() {
        let fixture = Fixture::new();
        let directory = fixture.reference("metadata");
        rewrite_retained(
            &directory,
            &format!("source_sha256 = '{}'", digest(CONTENT)),
            &format!("source_sha256 = '{}'", digest(b"modified\n")),
        );
        assert_eq!(
            read(&directory).unwrap_err(),
            contract("changed retained output requires transformation")
        );
    }

    #[test]
    fn source_schemas_and_licenses_cannot_be_replaced_by_a_transformation_note() {
        for category in ["schema", "license"] {
            let fixture = Fixture::new();
            let directory = fixture.reference(category);
            rewrite(
                &directory,
                "[provenance.artifacts.retained]",
                "transformation = 'arbitrary replacement'\n[provenance.artifacts.retained]",
            );
            rewrite_retained(
                &directory,
                &format!(
                    "source_bytes = {}\nsource_sha256 = '{}'",
                    CONTENT.len(),
                    digest(CONTENT)
                ),
                &format!("source_bytes = 3\nsource_sha256 = '{}'", digest(b"{}\n")),
            );
            fs::write(directory.join("capture"), b"{}\n").unwrap();
            assert_eq!(
                read(&directory).unwrap_err(),
                contract("schema and license captures must be verbatim")
            );
        }
    }

    #[test]
    fn declared_parameter_schema_requires_its_retained_original() {
        let fixture = Fixture::new();
        let directory = fixture.reference("schema");
        let path = directory.join("bidder.toml");
        let original = fs::read_to_string(&path).unwrap();
        let (witness, _) = original
            .split_once("[provenance.artifacts.retained]")
            .unwrap();
        fs::write(path, witness).unwrap();
        assert_eq!(
            read(&directory).unwrap_err(),
            contract("schema requires a retained original")
        );
    }

    #[test]
    fn reserved_and_helper_names_remain_invalid_for_reference_only_packages() {
        for factory in [false, true] {
            for key in ["harness", "tests"] {
                let fixture = Fixture::new();
                let directory = fixture.local(key, factory);
                assert_eq!(
                    read(&directory).unwrap_err(),
                    Refusal::Contract {
                        key: key.into(),
                        message: "reserved bidder key".into(),
                    }
                );
            }
            let fixture = Fixture::new();
            fixture.local("url", factory);
            fs::write(fixture.0.join("url.rs"), "").unwrap();
            assert_eq!(
                scan(&fixture.0, &BTreeSet::new()).unwrap_err(),
                Refusal::Contract {
                    key: "url".into(),
                    message: "bidder key collides with helper module".into(),
                }
            );
        }
    }

    #[test]
    fn immutable_source_revisions_and_blob_ids_must_be_full_hex_pins() {
        for pin in [
            "main",
            "1234",
            "111111111111111111111111111111111111111G",
            "0000000000000000000000000000000000000000",
        ] {
            for (field, old, message) in [
                (
                    "revision",
                    REVISION,
                    "provenance.revision must be a full immutable Git object ID",
                ),
                (
                    "source_blob",
                    BLOB,
                    "provenance.source_blob must be a full immutable Git object ID",
                ),
            ] {
                let fixture = Fixture::new();
                let directory = fixture.reference("schema");
                rewrite(
                    &directory,
                    &format!("{field} = '{old}'"),
                    &format!("{field} = '{pin}'"),
                );
                assert_eq!(read(&directory).unwrap_err(), contract(message));
            }
        }
    }

    #[test]
    fn source_and_retained_digests_are_strict_lowercase_sha256() {
        for bad in [
            "",
            "ab12",
            "GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG",
        ] {
            for (retained, message) in [
                (false, "provenance.source_sha256 must be lowercase SHA256"),
                (
                    true,
                    "provenance.retained.source_sha256 must be lowercase SHA256",
                ),
            ] {
                let fixture = Fixture::new();
                let directory = fixture.reference("schema");
                let needle = format!("\nsource_sha256 = '{}'", digest(CONTENT));
                let replacement = format!("\nsource_sha256 = '{bad}'");
                if retained {
                    rewrite_retained(&directory, &needle, &replacement);
                } else {
                    rewrite(&directory, &needle, &replacement);
                }
                assert_eq!(read(&directory).unwrap_err(), contract(message));
            }
        }
    }

    #[test]
    fn source_repository_requires_structured_absolute_identity_without_credentials() {
        for repository in [
            "../repo",
            "not a URL",
            "https://user:secret@example.test/repo",
            "https://example.test/repo?branch=main",
            "https://example.test/repo#main",
        ] {
            let fixture = Fixture::new();
            let directory = fixture.reference("schema");
            rewrite(&directory, "https://example.test/upstream.git", repository);
            assert_eq!(
                read(&directory).unwrap_err(),
                contract(
                    "provenance.repository must be an absolute HTTPS repository without credentials, query or fragment"
                )
            );
        }
    }

    #[test]
    fn undeclared_source_and_duplicate_artifact_identity_are_refused() {
        let fixture = Fixture::new();
        let directory = fixture.reference("schema");
        rewrite(&directory, "source = 'upstream'", "source = 'absent'");
        assert_eq!(
            read(&directory).unwrap_err(),
            contract("provenance.source must name a declared source")
        );
        let duplicate_fixture = Fixture::new();
        let item = artifact("schema", "capture", CONTENT);
        let duplicate_directory =
            duplicate_fixture.bidder("fake_probe", false, &reference(&format!("{item}{item}")));
        fs::write(duplicate_directory.join("capture"), CONTENT).unwrap();
        assert_eq!(
            read(&duplicate_directory).unwrap_err(),
            contract("duplicate source artifact")
        );
    }

    #[test]
    fn source_and_retained_paths_reject_escaping_absolute_and_non_normal_spellings() {
        for path in [
            "",
            "../escape",
            "/absolute",
            "C:\\absolute",
            "a\\b",
            "./capture",
            "a/../capture",
            "a//capture",
            "a/./capture",
            "capture/",
        ] {
            for retained in [false, true] {
                let fixture = Fixture::new();
                let directory = fixture.reference("schema");
                let old = if retained {
                    "capture"
                } else {
                    "external/schema"
                };
                rewrite(
                    &directory,
                    &format!("\nsource_path = '{old}'"),
                    &format!("\nsource_path = '{path}'"),
                );
                assert_eq!(
                    read(&directory).unwrap_err(),
                    contract(if retained {
                        "provenance.retained.source_path must be a normal relative path"
                    } else {
                        "provenance.source_path must be a normal relative path"
                    })
                );
            }
        }
    }

    #[test]
    fn provenance_cannot_hash_its_own_manifest() {
        let fixture = Fixture::new();
        let directory = fixture.reference("metadata");
        rewrite_retained(
            &directory,
            "source_path = 'capture'",
            "source_path = 'bidder.toml'",
        );
        assert_eq!(
            read(&directory).unwrap_err(),
            contract("manifest self-hash is forbidden")
        );
    }

    #[test]
    fn retained_files_must_exist_be_regular_and_match_length() {
        for mutation in ["missing", "directory", "length"] {
            let fixture = Fixture::new();
            let directory = fixture.reference("protocol");
            let path = directory.join("capture");
            fs::remove_file(&path).unwrap();
            let message = match mutation {
                "directory" => {
                    fs::create_dir_all(&path).unwrap();
                    "expected regular file"
                }
                "length" => {
                    fs::write(&path, b"short").unwrap();
                    "retained byte length mismatch"
                }
                _ => "missing input",
            };
            assert_eq!(
                read(&directory).unwrap_err(),
                input(&directory.join("bidder.toml"), message)
            );
        }
    }

    #[test]
    fn scan_catches_global_public_name_duplicates_including_gated_packages() {
        let fixture = Fixture::new();
        fixture.bidder(
            "fake_alpha",
            true,
            "[provenance]\norigin = 'local'\n[definitions.fake_Public]\n",
        );
        fixture.bidder("fake_zeta", true, "required_feature = 'experimental'\n[provenance]\norigin = 'local'\n[definitions.fake_Public]\n");
        assert_eq!(
            scan(&fixture.0, &BTreeSet::from(["experimental".into()])).unwrap_err(),
            Refusal::DuplicatePublicName {
                first: "fake_alpha".into(),
                second: "fake_zeta".into(),
            }
        );
    }

    #[test]
    fn scan_is_deterministic_and_tracks_nested_additions_and_deletions() {
        let fixture = Fixture::new();
        let zeta = fixture.local("fake_zeta", true);
        let alpha = fixture.local("fake_alpha", false);
        let helper = fixture.0.join("url.rs");
        fs::write(&helper, "").unwrap();
        fs::create_dir_all(zeta.join("fixtures")).unwrap();
        let nested = zeta.join("fixtures/source");
        fs::write(&nested, [0xff]).unwrap();
        let catalog = scan(&fixture.0, &BTreeSet::new()).unwrap();
        assert_eq!(
            catalog
                .manifests
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["fake_alpha", "fake_zeta"]
        );
        assert_eq!(
            catalog.input_paths,
            BTreeSet::from([
                fixture.0.clone(),
                helper,
                alpha.clone(),
                alpha.join("bidder.toml"),
                zeta.clone(),
                zeta.join("bidder.toml"),
                zeta.join("mod.rs"),
                zeta.join("fixtures"),
                nested.clone(),
            ])
        );
        fs::remove_file(&nested).unwrap();
        assert!(
            !scan(&fixture.0, &BTreeSet::new())
                .unwrap()
                .input_paths
                .contains(&nested)
        );
    }

    #[test]
    fn absolute_scan_and_read_roots_must_be_normal() {
        let fixture = Fixture::new();
        for path in [
            PathBuf::from("relative"),
            fixture.0.join("../escape"),
            fixture.0.join("./probe"),
        ] {
            assert_eq!(
                read(&path).unwrap_err(),
                input(&path, "non-normal absolute path")
            );
            assert_eq!(
                scan(&path, &BTreeSet::new()).unwrap_err(),
                input(&path, "non-normal absolute path")
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_rejected_at_leaves_and_ancestors_including_dangling_links() {
        use std::os::unix::fs::symlink;
        for member in ["bidder.toml", "mod.rs", "capture", "fixtures"] {
            let fixture = Fixture::new();
            let directory = fixture.reference("schema");
            let path = directory.join(member);
            if path.is_file() {
                fs::remove_file(&path).unwrap();
            }
            symlink(fixture.0.join("absent"), &path).unwrap();
            assert_eq!(
                scan(&fixture.0, &BTreeSet::new()).unwrap_err(),
                input(&path, "symlink input")
            );
        }
        let fixture = Fixture::new();
        let directory = fixture.local("fake_probe", true);
        let alias = fixture.0.join("alias");
        symlink(&directory, &alias).unwrap();
        assert_eq!(read(&alias).unwrap_err(), input(&alias, "symlink input"));
        let ancestor = fixture.0.join("ancestor");
        symlink(&fixture.0, &ancestor).unwrap();
        let path = ancestor.join("fake_probe");
        assert_eq!(read(&path).unwrap_err(), input(&ancestor, "symlink input"));
    }

    #[cfg(unix)]
    #[test]
    fn nonregular_inputs_are_refused_without_opening_them() {
        use std::os::unix::net::UnixListener;
        let fixture = Fixture::new();
        let directory = fixture.local("fake_probe", true);
        let path = directory.join("s");
        let _listener = UnixListener::bind(&path).unwrap();
        assert_eq!(
            scan(&fixture.0, &BTreeSet::new()).unwrap_err(),
            input(&path, "nonregular input")
        );
    }
}
