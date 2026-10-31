//! Offline host compiler for the reviewed request-disclosure inventories.

use crate::disclosure_types::{Delegate, Domains, Rule, Shape, Transform};
use auctioneer_openrtb::field_metadata::{Model, REQUEST, Storage};
use auctioneer_openrtb_catalogue::{
    Catalogue, SourceManifest, extract_catalogue, validate_manifest, validate_source_digest,
};
use proc_macro2::{Literal, TokenStream};
use quote::quote;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Component, Path};
use std::str;
use syn::Ident;
use time::macros::format_description;
use time::{Date, Duration};
use url::Url;

type CompileResult<T> = Result<T, Box<dyn Error>>;
type Inventory = BTreeMap<(String, String), Shape>;
type Schemas = BTreeMap<String, Vec<RawField>>;

#[derive(Default)]
struct ModelSlots {
    fields: BTreeMap<(String, String), SlotRepresentation>,
    flattened_json: BTreeSet<String>,
}

impl ModelSlots {
    fn storage(&self, object: &str, field: &str) -> Option<SlotRepresentation> {
        self.fields
            .get(&(object.to_owned(), field.to_owned()))
            .copied()
            .or_else(|| {
                self.flattened_json
                    .contains(object)
                    .then_some(SlotRepresentation::Json)
            })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SlotRepresentation {
    Typed,
    RawValue,
    Json,
}

// Independent download of the exact revision also pinned by the importer.
const PINNED_SOURCE_SHA256: &str =
    "1b730796e2776d576cb5749706f6638fd4aa35594c1182e7d0e6aedbce605728";
const USER_FPD: [&str; 4] = [
    "user.data",
    "user.keywords",
    "user.kwarray",
    "user.ext.data",
];
const TRANSACTION_IDS: [&str; 5] = [
    "source.tid",
    "source.other.tid",
    "imp.ext.tid",
    "ImpExt.other.tid",
    "Imp.other.ext.tid",
];

// Independently reviewed provenance and wire membership, not privacy policy.
const EXTENSION_LICENSE: &str = "disclosure-sources/LICENSE-extensions";
const EXTENSION_LICENSE_SHA256: &str =
    "9d130cc11efd232f041473f0cd62c43806b9389d63b599c8ee0862b699e8bc58";

struct ReviewedSource {
    id: &'static str,
    url: &'static str,
    commit: &'static str,
    artifact: &'static str,
    sha256: &'static str,
    licensed: bool,
    lines: (Option<usize>, Option<usize>),
}

const REVIEWED_SOURCES: [ReviewedSource; 3] = [
    ReviewedSource {
        id: "user-extension",
        url: "https://github.com/prebid/prebid-server/blob/d7f8515b86258688304b0d9b6668c6a0e258bc9e/openrtb_ext/user.go",
        commit: "d7f8515b86258688304b0d9b6668c6a0e258bc9e",
        artifact: "disclosure-sources/user-extension.source",
        sha256: "bf0492672514ba7dc99619b96d27c73bb0ea0339b049ae2fb9cbc32009dc9dc0",
        licensed: true,
        lines: (None, None),
    },
    ReviewedSource {
        id: "device-extension",
        url: "https://github.com/prebid/prebid-server/blob/d7f8515b86258688304b0d9b6668c6a0e258bc9e/openrtb_ext/device.go",
        commit: "d7f8515b86258688304b0d9b6668c6a0e258bc9e",
        artifact: "disclosure-sources/device-extension.source",
        sha256: "22d28cbc4315cd5a228589343070c94b5079e68c62e08064331bbceb3a037b7a",
        licensed: true,
        lines: (None, None),
    },
    ReviewedSource {
        id: "ownership",
        url: "https://github.com/IABTechLab/auctioneer/blob/b88cd1d2666e833cda9929ac6f617f9366e0f304/docs/superpowers/specs/2026-10-09-gdpr-scrub-classification-design.md",
        commit: "b88cd1d2666e833cda9929ac6f617f9366e0f304",
        artifact: "disclosure-sources/ownership-classification.md",
        sha256: "b9e4ed1a0ae1883f1f0badb00d36f8c4089b9f846b98d77a9cb288aefeed4d13",
        licensed: false,
        lines: (Some(195), Some(200)),
    },
];

struct ReviewedDeclaration {
    object: &'static str,
    field: &'static str,
    kind: &'static str,
    source: &'static str,
    reference: &'static str,
}

const REVIEWED_DECLARATIONS: [ReviewedDeclaration; 7] = [
    ReviewedDeclaration {
        object: "DeviceExt",
        field: "atts",
        kind: "integer",
        source: "device-extension",
        reference: "ExtDevice.ATTS",
    },
    ReviewedDeclaration {
        object: "DeviceExt",
        field: "prebid",
        kind: "object",
        source: "device-extension",
        reference: "ExtDevice.Prebid",
    },
    ReviewedDeclaration {
        object: "DeviceExtPrebid",
        field: "interstitial",
        kind: "object",
        source: "device-extension",
        reference: "ExtDevicePrebid.Interstitial",
    },
    ReviewedDeclaration {
        object: "DeviceInterstitial",
        field: "minheightperc",
        kind: "integer",
        source: "device-extension",
        reference: "ExtDeviceInt.MinHeightPerc",
    },
    ReviewedDeclaration {
        object: "DeviceInterstitial",
        field: "minwidthperc",
        kind: "integer",
        source: "device-extension",
        reference: "ExtDeviceInt.MinWidthPerc",
    },
    ReviewedDeclaration {
        object: "UserExt",
        field: "data",
        kind: "json",
        source: "ownership",
        reference: "FPD delegation",
    },
    ReviewedDeclaration {
        object: "UserExt",
        field: "eids",
        kind: "object array",
        source: "user-extension",
        reference: "ExtUser.Eids",
    },
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExtensions {
    version: u16,
    sources: Vec<RawSource>,
    ownership: RawOwnership,
    fields: Vec<RawExtensionField>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSource {
    id: String,
    url: String,
    commit: String,
    artifact: String,
    sha256: String,
    license: Option<String>,
    license_sha256: Option<String>,
    first_line: Option<usize>,
    last_line: Option<usize>,
    reviewer: String,
    review_date: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOwnership {
    source: String,
    user_fpd: Vec<String>,
    transaction_ids: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExtensionField {
    object: String,
    field: String,
    #[serde(rename = "type")]
    kind: String,
    source: String,
    reference: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPolicy {
    version: u16,
    reviewer: String,
    review_date: String,
    schemas: Vec<RawSchema>,
    fields: Vec<RawField>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSchema {
    object: String,
    unknown: UnknownRule,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum UnknownRule {
    RemoveRestricted,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawField {
    object: String,
    field: String,
    shape: Shape,
    #[serde(rename = "action")]
    rule: Rule,
    rationale: String,
    transform: Option<Transform>,
    child: Option<String>,
    delegate: Option<Delegate>,
}

/// Deterministic generated descriptors and non-fatal review-age diagnostics.
#[derive(Debug, PartialEq, Eq)]
pub struct CompileOutput {
    pub rust_source: String,
    pub warnings: Vec<String>,
}

/// Validate the local disclosure inputs and render static runtime descriptors.
///
/// # Errors
/// Returns an actionable error for invalid inputs or failed local file reads.
pub fn compile(data_dir: &Path, today: Date) -> Result<CompileOutput, Box<dyn Error>> {
    compile_inputs(data_dir, today).map_err(|error| format!("disclosure: {error}").into())
}

fn compile_inputs(data_dir: &Path, today: Date) -> CompileResult<CompileOutput> {
    let root = fs::canonicalize(data_dir)
        .map_err(|error| format!("resolve data directory {}: {error}", data_dir.display()))?;
    let mut warnings = Vec::new();
    let manifest: SourceManifest = read_document(&root, "openrtb-2.6-source.toml")?;
    validate_manifest(&manifest)?;
    let source = read_local(&root, "openrtb-2.6-source.md")?;
    validate_source_digest(&manifest, &source)?;
    if manifest.sha256 != PINNED_SOURCE_SHA256 {
        return Err(
            "standard source digest does not match the independently pinned revision".into(),
        );
    }
    let extracted = extract_catalogue(str::from_utf8(&source)?)?;
    let catalogue: Catalogue = read_document(&root, "openrtb-2.6-catalogue.toml")?;
    if catalogue != extracted {
        return Err("standard catalogue does not reproduce the exact pinned source".into());
    }
    validate_review(
        "standard source",
        &manifest.reviewer,
        &manifest.review_date,
        today,
        &mut warnings,
    )?;
    let mut inventory = Inventory::new();
    for member in catalogue.fields {
        let shape = membership_shape(&member.r#type)?;
        inventory.insert((member.object, member.field), shape);
    }
    let extensions: RawExtensions = read_document(&root, "supported-extension-catalogue.toml")?;
    validate_extensions(&root, extensions, today, &mut warnings, &mut inventory)?;
    let policy: RawPolicy = read_document(&root, "request-disclosure-policy.toml")?;
    validate_review(
        "policy",
        &policy.reviewer,
        &policy.review_date,
        today,
        &mut warnings,
    )?;
    let models = compiled_model_graph(REQUEST)?;
    let model_slots = compiled_model_slots(&models);
    let schemas = validate_policy(policy, &inventory, &model_slots)?;
    let closure = derive_domains(&schemas)?;
    let mut rust_source = render(&schemas, &closure)?;
    rust_source.push_str(&render_model_agreement(&models).to_string());
    rust_source.push('\n');
    Ok(CompileOutput {
        rust_source,
        warnings,
    })
}

fn read_document<T: DeserializeOwned>(root: &Path, name: &str) -> CompileResult<T> {
    let bytes = read_local(root, name)?;
    let text = str::from_utf8(&bytes).map_err(|error| format!("{name}: {error}"))?;
    toml::from_str(text).map_err(|error| format!("{name}: {error}").into())
}

fn read_local(root: &Path, name: &str) -> CompileResult<Vec<u8>> {
    let relative = Path::new(name);
    if name.is_empty()
        || name.contains(['\\', ':'])
        || relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(format!("artifact path must be a nonescaping relative file: {name}").into());
    }
    let resolved = fs::canonicalize(root.join(relative))
        .map_err(|error| format!("resolve artifact path {name}: {error}"))?;
    if !resolved.starts_with(root) || !resolved.is_file() {
        return Err(
            format!("artifact path escapes the data directory or is not a file: {name}").into(),
        );
    }
    fs::read(&resolved).map_err(|error| format!("read artifact {name}: {error}").into())
}

fn nonempty(label: &str, value: &str) -> CompileResult<()> {
    if value.trim().is_empty() {
        return Err(format!("empty {label}").into());
    }
    Ok(())
}

fn validate_review(
    label: &str,
    reviewer: &str,
    review_date: &str,
    today: Date,
    warnings: &mut Vec<String>,
) -> CompileResult<()> {
    nonempty(&format!("{label} reviewer"), reviewer)?;
    let review = Date::parse(review_date, format_description!("[year]-[month]-[day]"))
        .map_err(|_error| format!("{label} review_date is not a Gregorian YYYY-MM-DD date"))?;
    if review_date.len() != 10 || review.year() < 1_i32 {
        return Err(format!("{label} review_date is not a Gregorian YYYY-MM-DD date").into());
    }
    if review
        .checked_add(Duration::days(180))
        .is_some_and(|limit| today > limit)
    {
        warnings.push(format!(
            "{label} review_date {review_date} is over 180 days old"
        ));
    }
    Ok(())
}

fn validate_digest(label: &str, expected: &str, bytes: &[u8]) -> CompileResult<()> {
    if expected.len() != 64
        || !expected
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{label} digest must be 64 lowercase hexadecimal characters").into());
    }
    if hex::encode(Sha256::digest(bytes)) != expected {
        return Err(format!("{label} digest does not match the artifact bytes").into());
    }
    Ok(())
}

fn validate_url(source: &RawSource) -> CompileResult<()> {
    if source.commit.len() != 40
        || !source
            .commit
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!(
            "source {} commit must be a full lowercase hexadecimal revision",
            source.id
        )
        .into());
    }
    let url =
        Url::parse(&source.url).map_err(|error| format!("source {} url: {error}", source.id))?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(format!(
            "source {} url must be an immutable HTTPS source reference",
            source.id
        )
        .into());
    }
    let segments: Vec<&str> = url
        .path_segments()
        .ok_or("source url has no file path")?
        .collect();
    let revision = match (url.host_str(), segments.as_slice()) {
        (Some("github.com"), [owner, repo, "blob", commit, files @ ..])
        | (Some("raw.githubusercontent.com"), [owner, repo, commit, files @ ..])
            if !owner.is_empty()
                && !repo.is_empty()
                && !files.is_empty()
                && files.iter().all(|part| !part.is_empty()) =>
        {
            Some(*commit)
        }
        _ => None,
    };
    if revision != Some(source.commit.as_str()) {
        return Err(format!("source {} url must pin the declared full commit", source.id).into());
    }
    Ok(())
}

fn validate_ownership_set(label: &str, actual: &[String], expected: &[&str]) -> CompileResult<()> {
    let members: BTreeSet<&str> = actual.iter().map(String::as_str).collect();
    let required: BTreeSet<&str> = expected.iter().copied().collect();
    if members.len() != actual.len() || members != required {
        return Err(format!("ownership {label} must contain exactly the independently reviewed paths, without duplicates").into());
    }
    Ok(())
}

fn validate_extensions(
    root: &Path,
    extensions: RawExtensions,
    today: Date,
    warnings: &mut Vec<String>,
    inventory: &mut Inventory,
) -> CompileResult<()> {
    if extensions.version != 1 {
        return Err("unsupported extension catalogue version".into());
    }
    validate_ownership_set("user_fpd", &extensions.ownership.user_fpd, &USER_FPD)?;
    validate_ownership_set(
        "transaction_ids",
        &extensions.ownership.transaction_ids,
        &TRANSACTION_IDS,
    )?;
    let mut sources = BTreeMap::new();
    for source in extensions.sources {
        nonempty("extension source id", &source.id)?;
        let id = source.id.clone();
        if sources.insert(id.clone(), source).is_some() {
            return Err(format!("duplicate extension source id {id}").into());
        }
    }
    if !sources.contains_key(&extensions.ownership.source) {
        return Err("ownership source is missing from the declared source artifacts".into());
    }
    for source in sources.values() {
        validate_url(source)?;
        validate_review(
            &format!("source {}", source.id),
            &source.reviewer,
            &source.review_date,
            today,
            warnings,
        )?;
        let bytes = read_local(root, &source.artifact)?;
        validate_digest(&format!("source {}", source.id), &source.sha256, &bytes)?;
        validate_source_license(
            root,
            source,
            &bytes,
            source.id == extensions.ownership.source,
        )?;
    }
    let standard_objects: BTreeSet<String> = inventory
        .keys()
        .map(|(object, _field)| object.clone())
        .collect();
    for field in &extensions.fields {
        validate_name(&field.object)?;
        validate_name(&field.field)?;
        nonempty("extension reference", &field.reference)?;
        if !sources.contains_key(&field.source) {
            return Err(format!(
                "extension {}.{} has an unknown source {}",
                field.object, field.field, field.source
            )
            .into());
        }
        if standard_objects.contains(&field.object) {
            return Err(format!(
                "extension catalogue cannot add standard object member {}.{}",
                field.object, field.field
            )
            .into());
        }
        let shape = membership_shape(&field.kind)?;
        if shape == Shape::Json
            && (field.object != "UserExt"
                || field.field != "data"
                || field.source != extensions.ownership.source)
        {
            return Err("json shape belongs only to the owned UserExt.data delegation".into());
        }
        let key = (field.object.clone(), field.field.clone());
        if inventory.insert(key.clone(), shape).is_some() {
            return Err(format!("duplicate extension catalogue member {}.{}", key.0, key.1).into());
        }
    }
    validate_reviewed_sources(&sources, &extensions.ownership)?;
    validate_reviewed_declarations(&extensions.fields)?;
    Ok(())
}

fn validate_reviewed_sources(
    sources: &BTreeMap<String, RawSource>,
    ownership: &RawOwnership,
) -> CompileResult<()> {
    if sources.len() != REVIEWED_SOURCES.len() || ownership.source != "ownership" {
        return Err(
            "reviewed binding requires exactly the three pinned sources and ownership origin"
                .into(),
        );
    }
    for pin in &REVIEWED_SOURCES {
        let source = sources
            .get(pin.id)
            .ok_or_else(|| format!("reviewed binding is missing pinned source {}", pin.id))?;
        if source.url != pin.url || source.commit != pin.commit {
            return Err(format!(
                "reviewed binding source {} origin/url/commit changed",
                pin.id
            )
            .into());
        }
        if source.artifact != pin.artifact {
            return Err(format!("reviewed binding source {} artifact path changed", pin.id).into());
        }
        if source.sha256 != pin.sha256 {
            return Err(format!(
                "reviewed binding source {} digest differs from the independently pinned bytes",
                pin.id
            )
            .into());
        }
        if (source.first_line, source.last_line) != pin.lines {
            return Err(format!(
                "reviewed binding source {} excerpt line range changed",
                pin.id
            )
            .into());
        }
        let (license, digest) = if pin.licensed {
            (Some(EXTENSION_LICENSE), Some(EXTENSION_LICENSE_SHA256))
        } else {
            (None, None)
        };
        if source.license.as_deref() != license {
            return Err(format!("reviewed binding source {} license path changed", pin.id).into());
        }
        if source.license_sha256.as_deref() != digest {
            return Err(format!(
                "reviewed binding source {} license digest differs from the independent shared pin",
                pin.id
            )
            .into());
        }
    }
    Ok(())
}

fn validate_reviewed_declarations(fields: &[RawExtensionField]) -> CompileResult<()> {
    if fields.len() != REVIEWED_DECLARATIONS.len() {
        return Err("reviewed binding has missing or extra members; exactly seven declarations are supported".into());
    }
    for field in fields {
        let pin = REVIEWED_DECLARATIONS
            .iter()
            .find(|pin| field.object == pin.object && field.field == pin.field)
            .ok_or_else(|| {
                format!(
                    "reviewed binding has an unknown declaration {}.{}",
                    field.object, field.field
                )
            })?;
        if field.kind != pin.kind || field.source != pin.source || field.reference != pin.reference
        {
            return Err(format!("reviewed binding {}.{} disagrees with its pinned source, declaration reference, or catalogue type", field.object, field.field).into());
        }
    }
    Ok(())
}

fn validate_source_license(
    root: &Path,
    source: &RawSource,
    bytes: &[u8],
    ownership: bool,
) -> CompileResult<()> {
    match (&source.license, &source.license_sha256) {
        (Some(path), Some(digest)) => {
            let license = read_local(root, path)?;
            validate_digest(&format!("source {} license", source.id), digest, &license)?;
        }
        (None, None) if ownership => {}
        _ => {
            return Err(format!(
                "source {} needs a complete license and license_sha256 pair",
                source.id
            )
            .into());
        }
    }
    match (source.first_line, source.last_line) {
        (Some(first), Some(last)) if ownership && first > 0 && last >= first => {
            let length = last.checked_sub(first).and_then(|span| span.checked_add(1));
            if length != Some(str::from_utf8(bytes)?.lines().count()) {
                return Err(format!(
                    "source {} excerpt line range does not match the artifact",
                    source.id
                )
                .into());
            }
        }
        (None, None) if !ownership => {}
        _ => {
            return Err(format!(
                "source {} needs a valid ownership excerpt line range",
                source.id
            )
            .into());
        }
    }
    Ok(())
}

fn membership_shape(kind: &str) -> CompileResult<Shape> {
    match kind {
        "string" => Ok(Shape::String),
        "integer" => Ok(Shape::Integer),
        "float" => Ok(Shape::Number),
        "string array" => Ok(Shape::StringArray),
        "object" => Ok(Shape::Object),
        "object array" => Ok(Shape::ObjectArray),
        "json" => Ok(Shape::Json),
        _ => Err(format!("unknown catalogue type {kind}").into()),
    }
}

fn validate_name(name: &str) -> CompileResult<()> {
    if !name
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        || name.bytes().all(|byte| byte == b'_')
    {
        return Err(format!("invalid or empty catalogue object/field name {name}").into());
    }
    Ok(())
}

// Wire membership admits fields; Rust storage independently limits traversal.
fn compiled_model_graph(root: &Model) -> CompileResult<Vec<&Model>> {
    fn visit<'a>(
        model: &'a Model,
        models: &mut BTreeMap<&'a str, &'a Model>,
        active: &mut Vec<&'a Model>,
    ) -> CompileResult<()> {
        if active
            .iter()
            .any(|ancestor| ancestor.same_declaration(model))
        {
            return Err(format!("recursive compiled model {}", model.wire_name).into());
        }
        if let Some(previous) = models.get(model.wire_name) {
            if !previous.same_declaration(model) {
                return Err(format!(
                    "conflicting declarations for wire model {}: {} and {}",
                    model.wire_name, previous.rust_name, model.rust_name,
                )
                .into());
            }
            return Ok(());
        }
        models.insert(model.wire_name, model);
        active.push(model);
        for field in model.fields {
            if let Some(child) = field.child {
                visit(child, models, active)?;
            }
        }
        active.pop();
        Ok(())
    }
    let mut models = BTreeMap::new();
    visit(root, &mut models, &mut Vec::new())?;
    Ok(models.into_values().collect())
}

fn compiled_model_slots(models: &[&Model]) -> ModelSlots {
    let mut slots = ModelSlots::default();
    for model in models {
        for field in model.fields {
            if field.flattened {
                slots.flattened_json.insert(model.wire_name.to_owned());
                continue;
            }
            let storage = match field.storage {
                Storage::RawJson => SlotRepresentation::RawValue,
                Storage::Typed | Storage::JsonMap => SlotRepresentation::Typed,
            };
            slots
                .fields
                .insert((model.wire_name.to_owned(), field.name.to_owned()), storage);
        }
    }
    slots
}

fn render_model_agreement(models: &[&Model]) -> TokenStream {
    let entries = models.iter().map(|model| {
        let wire_name = model.wire_name;
        let rust_name = model.rust_name;
        let fields = model.fields.iter().map(|field| {
            let name = field.name;
            let flattened = field.flattened;
            let storage = match field.storage {
                Storage::Typed => quote!(Storage::Typed),
                Storage::RawJson => quote!(Storage::RawJson),
                Storage::JsonMap => quote!(Storage::JsonMap),
            };
            let child = match field.child {
                None => quote!(None),
                Some(child) => {
                    let child_wire_name = child.wire_name;
                    let child_rust_name = child.rust_name;
                    quote!(Some(
                        Identity {
                            wire_name: #child_wire_name, rust_name: #child_rust_name,
                        }
                    ))
                }
            };
            quote!(ExpectedField {
                name: #name, flattened: #flattened, storage: #storage, child: #child,
            })
        });
        quote!(ExpectedModel {
            wire_name: #wire_name, rust_name: #rust_name, fields: &[#(#fields),*],
        })
    });
    quote! {
        const _: () = {
            use ::auctioneer_openrtb::field_metadata::{
                ExpectedField, ExpectedModel, Identity, Storage, REQUEST, graph_matches,
            };
            assert!(
                graph_matches(REQUEST, &[#(#entries),*]),
                "compiled request field metadata disagrees with the build host"
            );
        };
    }
}

#[cfg(test)]
fn compiled_request_slots() -> CompileResult<ModelSlots> {
    Ok(compiled_model_slots(&compiled_model_graph(REQUEST)?))
}

fn validate_policy(
    policy: RawPolicy,
    inventory: &Inventory,
    model_slots: &ModelSlots,
) -> CompileResult<Schemas> {
    if policy.version != 1 {
        return Err("unsupported disclosure policy version".into());
    }
    let mut schemas = Schemas::new();
    let expected: BTreeSet<&str> = inventory
        .keys()
        .map(|(object, _field)| object.as_str())
        .collect();
    for RawSchema {
        object,
        unknown: UnknownRule::RemoveRestricted,
    } in policy.schemas
    {
        if !expected.contains(object.as_str()) {
            return Err(format!("extra or unknown policy schema {object}").into());
        }
        if schemas.insert(object.clone(), Vec::new()).is_some() {
            return Err(format!("duplicate policy schema {object}").into());
        }
    }
    for object in expected {
        if !schemas.contains_key(object) {
            return Err(format!("missing policy schema {object}").into());
        }
    }
    let mut fields = BTreeMap::new();
    for field in policy.fields {
        let key = (field.object.clone(), field.field.clone());
        let Some(shape) = inventory.get(&key) else {
            return Err(format!("unknown policy disposition {}.{}", key.0, key.1).into());
        };
        if *shape != field.shape {
            return Err(format!(
                "policy shape disagrees with membership for {}.{}",
                key.0, key.1
            )
            .into());
        }
        nonempty(&format!("{}.{} rationale", key.0, key.1), &field.rationale)?;
        validate_field(&field, model_slots.storage(&field.object, &field.field))?;
        if fields.insert(key.clone(), field).is_some() {
            return Err(format!("duplicate policy disposition {}.{}", key.0, key.1).into());
        }
    }
    for key in inventory.keys() {
        if !fields.contains_key(key) {
            return Err(format!("missing policy disposition {}.{}", key.0, key.1).into());
        }
    }
    for ((_object, _name), field) in fields {
        if let Some(child) = &field.child
            && !schemas.contains_key(child)
        {
            return Err(format!(
                "{}.{} child schema {child} is missing",
                field.object, field.field
            )
            .into());
        }
        let parent = schemas
            .get_mut(&field.object)
            .ok_or("policy field has no schema")?;
        parent.push(field);
    }
    validate_identifiers(&schemas)?;
    Ok(schemas)
}

#[cfg(test)]
fn validate_policy_inventory(
    policy: &str,
    inventory: &BTreeMap<(String, String), Shape>,
) -> CompileResult<()> {
    validate_policy(
        toml::from_str(policy)?,
        inventory,
        &compiled_request_slots()?,
    )
    .map(|_schemas| ())
}

#[cfg(test)]
fn validate_policy_compiled_models(
    policy: &str,
    inventory: &Inventory,
    roots: &[&Model],
) -> CompileResult<()> {
    let graphs = roots
        .iter()
        .map(|root| compiled_model_graph(root))
        .collect::<CompileResult<Vec<_>>>()?;
    let models = graphs.into_iter().flatten().collect::<Vec<_>>();
    let slots = compiled_model_slots(&models);
    validate_policy(toml::from_str(policy)?, inventory, &slots).map(|_schemas| ())
}

fn is_object(shape: Shape) -> bool {
    matches!(shape, Shape::Object | Shape::ObjectArray)
}

fn is_fpd(object: &str, field: &str) -> bool {
    matches!(
        (object, field),
        ("User", "data" | "keywords" | "kwarray") | ("UserExt", "data")
    )
}

fn validate_field(
    field: &RawField,
    discovered_storage: Option<SlotRepresentation>,
) -> CompileResult<()> {
    let label = format!("{}.{}", field.object, field.field);
    if field.rule != Rule::Transform && field.transform.is_some() {
        return Err(format!("{label} has a transform attached to a non-transform rule").into());
    }
    if field.rule != Rule::Delegate && field.delegate.is_some() {
        return Err(format!("{label} has a delegate attached to a non-delegate rule").into());
    }
    if field.shape == Shape::Json
        && (field.rule != Rule::Delegate || !is_fpd(&field.object, &field.field))
    {
        return Err(
            format!("{label} json shape is only delegated UserExt.data, not context").into(),
        );
    }
    let needs_child = match field.rule {
        Rule::Context | Rule::Id => is_object(field.shape),
        Rule::Child => {
            if !is_object(field.shape) {
                return Err(
                    format!("{label} child rule needs an object or object-array shape").into(),
                );
            }
            true
        }
        Rule::Control => {
            if !matches!(field.shape, Shape::String | Shape::Integer) {
                return Err(format!(
                    "{label} control rule needs an admitted string or integer shape"
                )
                .into());
            }
            false
        }
        Rule::Opaque => {
            if !is_object(field.shape) {
                return Err(format!("{label} opaque rule needs an object shape").into());
            }
            false
        }
        Rule::PreciseGeo => {
            if !matches!(field.shape, Shape::String | Shape::Integer | Shape::Number) {
                return Err(format!("{label} precise_geo rule needs a scalar shape").into());
            }
            false
        }
        Rule::Transform => {
            validate_transform(field)?;
            false
        }
        Rule::Delegate => {
            if field.delegate != Some(Delegate::UserFpd) {
                return Err(format!("{label} is missing the user_fpd delegate").into());
            }
            if !is_fpd(&field.object, &field.field) {
                return Err(format!("{label} is not an owned user_fpd branch").into());
            }
            false
        }
    };
    if needs_child && field.child.is_none() {
        let kind = if field.rule == Rule::Context {
            "context"
        } else {
            "object"
        };
        return Err(format!("{label} {kind} rule is missing its declared child schema").into());
    }
    if !needs_child && field.child.is_some() {
        return Err(format!("{label} rule has an extraneous child schema").into());
    }
    // RawValue slots can be removed, but their typed adapters do not visit children.
    let storage = discovered_storage
        .ok_or_else(|| format!("{label} Rust storage is missing or unclassified"))?;
    if storage == SlotRepresentation::RawValue && field.rule != Rule::Opaque {
        return Err(format!(
            "{label} raw-object typed adapter requires opaque rule; child traversal is unsupported"
        )
        .into());
    }
    if is_fpd(&field.object, &field.field) && field.rule != Rule::Delegate {
        return Err(format!("{label} must retain its exact user_fpd ownership delegation").into());
    }
    let eid_child = match (field.object.as_str(), field.field.as_str()) {
        ("User" | "UserExt", "eids") => Some("EID"),
        ("EID", "uids") => Some("UID"),
        _ => None,
    };
    if let Some(child) = eid_child
        && (field.rule != Rule::Id || field.child.as_deref() != Some(child))
    {
        return Err(
            format!("{label} must ID-gate the whole collection and declare child {child}").into(),
        );
    }
    Ok(())
}

fn validate_transform(field: &RawField) -> CompileResult<()> {
    let valid = match field.transform {
        Some(Transform::Coordinate) => {
            field.shape == Shape::Number
                && field.object == "Geo"
                && matches!(field.field.as_str(), "lat" | "lon")
        }
        Some(Transform::Ipv4) => {
            field.shape == Shape::String && field.object == "Device" && field.field == "ip"
        }
        Some(Transform::Ipv6) => {
            field.shape == Shape::String && field.object == "Device" && field.field == "ipv6"
        }
        None => false,
    };
    if !valid {
        return Err(format!(
            "{}.{} transform is missing or disagrees with its shape/address family",
            field.object, field.field
        )
        .into());
    }
    Ok(())
}

fn schema_variant(object: &str) -> String {
    match object {
        "EID" => "Eid".to_owned(),
        "UID" => "Uid".to_owned(),
        _ => pascal_case(object),
    }
}

fn pascal_case(name: &str) -> String {
    let mut result = String::new();
    for word in name.split('_') {
        let mut letters = word.chars();
        if let Some(initial) = letters.next() {
            result.push(initial.to_ascii_uppercase());
            result.push_str(letters.as_str());
        }
    }
    result
}

fn field_variant(field: &RawField) -> String {
    format!(
        "{}{}",
        schema_variant(&field.object),
        pascal_case(&field.field)
    )
}

fn validate_identifiers(schemas: &Schemas) -> CompileResult<()> {
    let mut schema_ids = BTreeSet::new();
    let mut field_ids = BTreeSet::new();
    for (object, fields) in schemas {
        validate_name(object)?;
        if !schema_ids.insert(schema_variant(object)) {
            return Err(format!("generated SchemaId collision for {object}").into());
        }
        for field in fields {
            validate_name(&field.field)?;
            if !field_ids.insert(field_variant(field)) {
                return Err(
                    format!("generated FieldId collision for {object}.{}", field.field).into(),
                );
            }
        }
    }
    Ok(())
}

fn direct_domains(rule: Rule) -> Domains {
    match rule {
        Rule::Context | Rule::Control | Rule::Child => Domains::NONE,
        Rule::Id => Domains::ID,
        Rule::Opaque => Domains::ID.union(Domains::GEO),
        Rule::PreciseGeo | Rule::Transform => Domains::GEO,
        Rule::Delegate => Domains::FPD,
    }
}

fn derive_domains(schemas: &Schemas) -> CompileResult<BTreeMap<String, Domains>> {
    let mut closure = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    for object in schemas.keys() {
        close_schema(object, schemas, &mut visiting, &mut closure)?;
    }
    Ok(closure)
}

fn close_schema(
    object: &str,
    schemas: &Schemas,
    visiting: &mut BTreeSet<String>,
    closure: &mut BTreeMap<String, Domains>,
) -> CompileResult<Domains> {
    if let Some(domains) = closure.get(object) {
        return Ok(*domains);
    }
    if !visiting.insert(object.to_owned()) {
        return Err(format!("child schema cycle at {object}").into());
    }
    let fields = schemas
        .get(object)
        .ok_or_else(|| format!("missing child schema {object}"))?;
    // Every scoped schema is protected; unknowns close under either ID or GEO.
    let mut domains = Domains::ID.union(Domains::GEO);
    for field in fields {
        domains = domains.union(direct_domains(field.rule));
        if let Some(child) = &field.child {
            domains = domains.union(close_schema(child, schemas, visiting, closure)?);
        }
    }
    visiting.remove(object);
    closure.insert(object.to_owned(), domains);
    Ok(domains)
}

struct IdEntry {
    variant: Ident,
    descriptor: TokenStream,
}

fn render_ids(
    name: &str,
    descriptor_type: &str,
    entries: &[IdEntry],
) -> CompileResult<TokenStream> {
    let name_ident = syn::parse_str::<Ident>(name)?;
    let descriptor_ident = syn::parse_str::<Ident>(descriptor_type)?;
    let variants: Vec<_> = entries.iter().map(|entry| &entry.variant).collect();
    let indices = (0..entries.len()).map(Literal::usize_unsuffixed);
    let descriptors = entries.iter().map(|entry| &entry.descriptor);
    Ok(quote! {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum #name_ident {
            #(#variants,)*
        }

        impl #name_ident {
            #[must_use]
            pub const fn index(self) -> usize {
                match self {
                    #(Self::#variants => #indices,)*
                }
            }

            #[must_use]
            pub fn descriptor(self) -> &'static #descriptor_ident {
                match self {
                    #(Self::#variants => #descriptors,)*
                }
            }
        }
    })
}

fn render(schemas: &Schemas, closure: &BTreeMap<String, Domains>) -> CompileResult<String> {
    let mut schema_ids = Vec::new();
    let mut field_ids = Vec::new();
    let mut field_arrays = Vec::new();
    for (schema_index, (object, fields)) in schemas.iter().enumerate() {
        let fields_ident = syn::parse_str::<Ident>(&format!("SCHEMA_FIELDS_{schema_index}"))?;
        let schema_index_literal = Literal::usize_unsuffixed(schema_index);
        schema_ids.push(IdEntry {
            variant: syn::parse_str::<Ident>(&schema_variant(object))?,
            descriptor: quote! { &SCHEMAS[#schema_index_literal] },
        });
        let mut rendered_fields = Vec::new();
        for (field_index, field) in fields.iter().enumerate() {
            let field_index_literal = Literal::usize_unsuffixed(field_index);
            field_ids.push(IdEntry {
                variant: syn::parse_str::<Ident>(&field_variant(field))?,
                descriptor: quote! { &#fields_ident[#field_index_literal] },
            });
            rendered_fields.push(render_field(field, closure)?);
        }
        let field_count = Literal::usize_unsuffixed(fields.len());
        field_arrays.push(quote! {
            static #fields_ident: [FieldRule; #field_count] = [#(#rendered_fields,)*];
        });
    }
    let schema_id_items = render_ids("SchemaId", "SchemaDesc", &schema_ids)?;
    let field_id_items = render_ids("FieldId", "FieldRule", &field_ids)?;
    let mut schema_descriptors = Vec::new();
    for (index, (object, _fields)) in schemas.iter().enumerate() {
        let variant = syn::parse_str::<Ident>(&schema_variant(object))?;
        let fields_ident = syn::parse_str::<Ident>(&format!("SCHEMA_FIELDS_{index}"))?;
        let domains = Literal::u8_unsuffixed(
            closure
                .get(object)
                .ok_or("missing derived schema domains")?
                .bits(),
        );
        schema_descriptors.push(quote! {
            SchemaDesc {
                id: SchemaId::#variant,
                name: #object,
                fields: &#fields_ident,
                domains: Domains::from_bits(#domains)
            }
        });
    }
    let schema_count = Literal::usize_unsuffixed(schemas.len());
    let source = quote! {
        #schema_id_items
        #field_id_items
        #(#field_arrays)*
        pub static SCHEMAS: [SchemaDesc; #schema_count] = [#(#schema_descriptors,)*];
    };
    let mut output = String::from("// Generated from validated local disclosure inventories.\n");
    output.push_str(&source.to_string());
    output.push('\n');
    Ok(output)
}

fn render_field(
    field: &RawField,
    closure: &BTreeMap<String, Domains>,
) -> CompileResult<TokenStream> {
    let shape = match field.shape {
        Shape::String => quote! { Shape::String },
        Shape::Integer => quote! { Shape::Integer },
        Shape::Number => quote! { Shape::Number },
        Shape::StringArray => quote! { Shape::StringArray },
        Shape::Object => quote! { Shape::Object },
        Shape::ObjectArray => quote! { Shape::ObjectArray },
        Shape::Json => quote! { Shape::Json },
    };
    let rule = match field.rule {
        Rule::Context => quote! { Rule::Context },
        Rule::Control => quote! { Rule::Control },
        Rule::Id => quote! { Rule::Id },
        Rule::Child => quote! { Rule::Child },
        Rule::Opaque => quote! { Rule::Opaque },
        Rule::PreciseGeo => quote! { Rule::PreciseGeo },
        Rule::Transform => quote! { Rule::Transform },
        Rule::Delegate => quote! { Rule::Delegate },
    };
    let transform = match field.transform {
        Some(Transform::Coordinate) => quote! { Some(Transform::Coordinate) },
        Some(Transform::Ipv4) => quote! { Some(Transform::Ipv4) },
        Some(Transform::Ipv6) => quote! { Some(Transform::Ipv6) },
        None => quote! { None },
    };
    let delegate = match field.delegate {
        Some(Delegate::UserFpd) => quote! { Some(Delegate::UserFpd) },
        None => quote! { None },
    };
    let mut domains = direct_domains(field.rule);
    let child = if let Some(object) = &field.child {
        domains = domains.union(*closure.get(object).ok_or("missing derived child domains")?);
        let variant = syn::parse_str::<Ident>(&schema_variant(object))?;
        quote! { Some(SchemaId::#variant) }
    } else {
        quote! { None }
    };
    let id = syn::parse_str::<Ident>(&field_variant(field))?;
    let name = &field.field;
    let domain_bits = Literal::u8_unsuffixed(domains.bits());
    Ok(quote! {
        FieldRule {
            id: FieldId::#id,
            name: #name,
            shape: #shape,
            rule: #rule,
            transform: #transform,
            child: #child,
            delegate: #delegate,
            domains: Domains::from_bits(#domain_bits)
        }
    })
}

#[cfg(test)]
mod tests {
    //! Independent inventory oracle and offline disclosure-compiler mutation tests.

    use super::{CompileOutput, compile, validate_policy_inventory};
    use crate::disclosure_types::Shape;
    use auctioneer_openrtb_catalogue::{Catalogue, extract_catalogue};
    use sha2::{Digest as _, Sha256};
    use std::collections::{BTreeMap, BTreeSet};
    use std::env;
    use std::error::Error;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::sync::atomic::{AtomicU64, Ordering};
    use time::{Date, Duration, Month};
    use toml::Value;

    const STANDARD_SOURCE: &str = "openrtb-2.6-source.md";
    const SOURCE_MANIFEST: &str = "openrtb-2.6-source.toml";
    const CATALOGUE: &str = "openrtb-2.6-catalogue.toml";
    const EXTENSIONS: &str = "supported-extension-catalogue.toml";
    const POLICY: &str = "request-disclosure-policy.toml";
    type TestInventory = BTreeMap<(String, String), Shape>;

    mod compiled_controls {
        use auctioneer_macros::OpenRtbFieldMetadata;
        use auctioneer_openrtb::ext::misc::DeviceExt;
        use auctioneer_openrtb::types::field::JsonField;
        use serde::Serialize;
        use serde_json::value::RawValue;
        use std::ops::Deref;

        #[derive(Serialize, OpenRtbFieldMetadata)]
        #[openrtb_metadata(name = "Foo")]
        pub struct Raw {
            #[serde(rename = "payload")]
            pub stored: Option<JsonField<Box<RawValue>>>,
        }

        #[derive(Serialize, OpenRtbFieldMetadata)]
        #[openrtb_metadata(name = "Foo")]
        pub struct Typed {
            #[serde(rename = "payload")]
            pub stored: Option<DeviceExt>,
        }

        #[derive(Serialize, OpenRtbFieldMetadata)]
        #[openrtb_metadata(name = "Foo")]
        pub struct Json {
            pub payload: Option<serde_json::Value>,
        }

        #[derive(Serialize, OpenRtbFieldMetadata)]
        #[openrtb_metadata(name = "Foo")]
        pub struct CatchAll {
            #[serde(flatten)]
            pub other: serde_json::Map<String, serde_json::Value>,
        }

        #[derive(Serialize, OpenRtbFieldMetadata)]
        #[openrtb_metadata(name = "Foo")]
        pub struct NamedMap {
            pub other: serde_json::Map<String, serde_json::Value>,
        }

        #[derive(Serialize, OpenRtbFieldMetadata)]
        #[openrtb_metadata(name = "Foo")]
        pub struct DifferentField {
            pub different: Option<DeviceExt>,
        }

        pub mod relocated {
            use auctioneer_macros::OpenRtbFieldMetadata;
            use serde::Serialize;
            pub use serde_json::value::RawValue as Opaque;
            pub type Alias<T = Opaque> = Option<Box<T>>;

            #[derive(Serialize, OpenRtbFieldMetadata)]
            #[openrtb_metadata(name = "Foo")]
            pub struct Model {
                pub payload: Alias,
            }
        }

        type Projected = <Box<RawValue> as Deref>::Target;

        #[derive(Serialize, OpenRtbFieldMetadata)]
        #[openrtb_metadata(name = "Foo")]
        pub struct Projection {
            pub payload: Box<Projected>,
        }
    }
    const DATA_FILES: [&str; 5] = [
        STANDARD_SOURCE,
        SOURCE_MANIFEST,
        CATALOGUE,
        EXTENSIONS,
        POLICY,
    ];
    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    // Independently transcribed from the pinned tables and approved dispositions,
    // not obtained by reading policy rows or compiler output.
    const EXPECTED_FIELDS: &str = "\
BrandVersion.brand String Context 0
BrandVersion.ext Object Opaque 3
BrandVersion.version StringArray Context 0
Device.carrier String Context 0
Device.connectiontype Integer Context 0
Device.devicetype Integer Context 0
Device.didmd5 String Id 1
Device.didsha1 String Id 1
Device.dnt Integer Control 0
Device.dpidmd5 String Id 1
Device.dpidsha1 String Id 1
Device.ext Object Child 3 - DeviceExt
Device.flashver String Context 0
Device.geo Object Child 3 - Geo
Device.geofetch Integer Context 0
Device.h Integer Context 0
Device.hwv String Context 0
Device.ifa String Id 1
Device.ip String Transform 2 Ipv4
Device.ipv6 String Transform 2 Ipv6
Device.js Integer Context 0
Device.langb String Context 0
Device.language String Context 0
Device.lmt Integer Control 0
Device.macmd5 String Id 1
Device.macsha1 String Id 1
Device.make String Context 0
Device.mccmnc String Context 0
Device.model String Context 0
Device.os String Context 0
Device.osv String Context 0
Device.ppi Integer Context 0
Device.pxratio Number Context 0
Device.sua Object Child 3 - UserAgent
Device.ua String Context 0
Device.w Integer Context 0
DeviceExt.atts Integer Control 0
DeviceExt.prebid Object Child 3 - DeviceExtPrebid
DeviceExtPrebid.interstitial Object Child 3 - DeviceInterstitial
DeviceInterstitial.minheightperc Integer Context 0
DeviceInterstitial.minwidthperc Integer Context 0
Eid.ext Object Opaque 3
Eid.inserter String Context 0
Eid.matcher String Context 0
Eid.mm Integer Context 0
Eid.source String Context 0
Eid.uids ObjectArray Id 3 - Uid
Geo.accuracy Integer PreciseGeo 2
Geo.city String Context 0
Geo.country String Context 0
Geo.ext Object Opaque 3
Geo.ipservice Integer Context 0
Geo.lastfix Integer Context 0
Geo.lat Number Transform 2 Coordinate
Geo.lon Number Transform 2 Coordinate
Geo.metro String Context 0
Geo.region String Context 0
Geo.regionfips104 String Context 0
Geo.type Integer Context 0
Geo.utcoffset Integer Context 0
Geo.zip String PreciseGeo 2
Uid.atype Integer Context 0
Uid.ext Object Opaque 3
Uid.id String Id 1
User.buyeruid String Id 1
User.consent String Control 0
User.customdata String Id 1
User.data ObjectArray Delegate 4 - - UserFpd
User.eids ObjectArray Id 3 - Eid
User.ext Object Child 7 - UserExt
User.gender String Id 1
User.geo Object Child 3 - Geo
User.id String Id 1
User.keywords String Delegate 4 - - UserFpd
User.kwarray StringArray Delegate 4 - - UserFpd
User.yob Integer Id 1
UserAgent.architecture String Context 0
UserAgent.bitness String Context 0
UserAgent.browsers ObjectArray Child 3 - BrandVersion
UserAgent.ext Object Opaque 3
UserAgent.mobile Integer Context 0
UserAgent.model String Context 0
UserAgent.platform Object Child 3 - BrandVersion
UserAgent.source Integer Context 0
UserExt.data Json Delegate 4 - - UserFpd
UserExt.eids ObjectArray Id 3 - Eid
";

    struct Fixture {
        dir: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let dir = env::temp_dir().join(format!(
                "auctioneer-disclosure-{}-{sequence}",
                process::id()
            ));
            fs::create_dir_all(&dir).unwrap();
            let fixture = Self { dir };
            let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("../auctioneer-config/data");
            for name in DATA_FILES {
                fs::copy(original.join(name), fixture.dir.join(name)).unwrap();
            }
            copy_tree(
                &original.join("disclosure-sources"),
                &fixture.dir.join("disclosure-sources"),
            );
            fixture
        }

        fn read(&self, name: &str) -> String {
            fs::read_to_string(self.dir.join(name)).unwrap()
        }

        fn edit(&self, name: &str, change: impl FnOnce(&mut Value)) {
            let mut document: Value = toml::from_str(&self.read(name)).unwrap();
            change(&mut document);
            fs::write(self.dir.join(name), toml::to_string(&document).unwrap()).unwrap();
        }

        fn field(&self, object: &str, field: &str, change: impl FnOnce(&mut Value)) {
            self.edit(POLICY, |document| {
                change(row_mut(document, "fields", object, Some(field)));
            });
        }

        fn compile(&self, today: Date) -> CompileOutput {
            compile(&self.dir, today).unwrap()
        }

        #[track_caller]
        fn reject(&self, diagnostic: &str) {
            let Err(error) = compile(&self.dir, review_date()) else {
                panic!("invalid disclosure input unexpectedly compiled");
            };
            let diagnostic_message = error.to_string();
            assert!(
                diagnostic_message.to_lowercase().contains(diagnostic),
                "expected {diagnostic} in: {diagnostic_message}"
            );
        }

        fn snapshot(&self) -> Vec<(PathBuf, Vec<u8>)> {
            let mut files = Vec::new();
            snapshot_tree(&self.dir, &self.dir, &mut files);
            files.sort_by(|left, right| left.0.cmp(&right.0));
            files
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _cleanup = fs::remove_dir_all(&self.dir);
        }
    }

    fn copy_tree(source: &Path, destination: &Path) {
        fs::create_dir_all(destination).unwrap();
        for entry_result in fs::read_dir(source).unwrap() {
            let entry = entry_result.unwrap();
            let destination_path = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_tree(&entry.path(), &destination_path);
            } else {
                fs::copy(entry.path(), destination_path).unwrap();
            }
        }
    }

    fn snapshot_tree(root: &Path, directory: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
        for entry_result in fs::read_dir(directory).unwrap() {
            let entry = entry_result.unwrap();
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                snapshot_tree(root, &path, files);
            } else {
                files.push((
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }

    fn review_date() -> Date {
        Date::from_calendar_date(2026_i32, Month::October, 11).unwrap()
    }

    fn rows_mut<'a>(document: &'a mut Value, name: &str) -> &'a mut Vec<Value> {
        document.get_mut(name).unwrap().as_array_mut().unwrap()
    }

    fn row_mut<'a>(
        document: &'a mut Value,
        table: &str,
        object: &str,
        field: Option<&str>,
    ) -> &'a mut Value {
        rows_mut(document, table)
            .iter_mut()
            .find(|row| {
                row.get("object").and_then(Value::as_str) == Some(object)
                    && field
                        .is_none_or(|name| row.get("field").and_then(Value::as_str) == Some(name))
            })
            .unwrap()
    }

    fn source_mut<'a>(document: &'a mut Value, id: &str) -> &'a mut Value {
        rows_mut(document, "sources")
            .iter_mut()
            .find(|row| row.get("id").and_then(Value::as_str) == Some(id))
            .unwrap()
    }

    fn set(row: &mut Value, key: &str, value: &str) {
        row.as_table_mut()
            .unwrap()
            .insert(key.to_owned(), Value::String(value.to_owned()));
    }

    fn remove(row: &mut Value, key: &str) {
        row.as_table_mut().unwrap().remove(key).unwrap();
    }

    fn variant(schema: &str, field: &str) -> String {
        let mut letters = field.chars();
        let initial = letters.next().unwrap().to_ascii_uppercase();
        format!("{schema}{initial}{}", letters.as_str())
    }

    fn option_token(kind: &str, name: Option<&str>) -> String {
        match name {
            Some("-") | None => "None".to_owned(),
            Some(value) => format!("Some({kind}::{value})"),
        }
    }

    fn expected_field(line: &str) -> String {
        let mut columns = line.split_whitespace();
        let (schema, field) = columns.next().unwrap().split_once('.').unwrap();
        let shape = columns.next().unwrap();
        let rule = columns.next().unwrap();
        let domains = columns.next().unwrap();
        let transform = option_token("Transform", columns.next());
        let child = option_token("SchemaId", columns.next());
        let delegate = option_token("Delegate", columns.next());
        assert_eq!(columns.next(), None);
        let id = variant(schema, field);
        format!(
            "FieldRule{{id:FieldId::{id},name:\"{field}\",shape:Shape::{shape},rule:Rule::{rule},transform:{transform},child:{child},delegate:{delegate},domains:Domains::from_bits({domains})}}"
        )
    }

    #[test]
    fn vendored_source_and_independent_membership_have_seventy_nine_plus_seven_rows() {
        let fixture = Fixture::new();
        let source = fixture.read(STANDARD_SOURCE);
        assert_eq!(
            hex::encode(Sha256::digest(source.as_bytes())),
            "1b730796e2776d576cb5749706f6638fd4aa35594c1182e7d0e6aedbce605728"
        );
        let extracted = extract_catalogue(&source).unwrap();
        let committed: Catalogue = toml::from_str(&fixture.read(CATALOGUE)).unwrap();
        assert_eq!(committed, extracted);
        assert_eq!(committed.fields.len(), 79);
        let extensions: Value = toml::from_str(&fixture.read(EXTENSIONS)).unwrap();
        assert_eq!(
            extensions.get("fields").unwrap().as_array().unwrap().len(),
            7
        );
        assert_eq!(EXPECTED_FIELDS.lines().count(), 86);
    }

    #[test]
    fn renders_all_independently_expected_field_dispositions_and_eid_uid_spelling() {
        let fixture = Fixture::new();
        let output = fixture.compile(review_date());
        assert!(output.warnings.is_empty());
        let syntax = syn::parse_file(&output.rust_source).unwrap();
        let descriptors: Vec<_> = syntax
            .items
            .iter()
            .filter_map(|item| {
                if let syn::Item::Static(table) = item
                    && let syn::Expr::Array(array) = &*table.expr
                {
                    Some(&array.elems)
                } else {
                    None
                }
            })
            .flat_map(|entries| entries.iter())
            .filter_map(|entry| {
                if let syn::Expr::Struct(record) = entry {
                    Some(&record.path)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            descriptors
                .iter()
                .filter(|path| path.is_ident("FieldRule"))
                .count(),
            86
        );
        assert_eq!(
            descriptors
                .iter()
                .filter(|path| path.is_ident("SchemaDesc"))
                .count(),
            11
        );
        let compact: String = output.rust_source.split_whitespace().collect();
        for line in EXPECTED_FIELDS.lines() {
            let expected = expected_field(line);
            assert!(
                compact.contains(&expected),
                "missing independently expected descriptor: {line}"
            );
        }
        assert!(compact.contains("pubenumSchemaId{"));
        assert!(compact.contains("pubenumFieldId{"));
        assert!(compact.contains("SchemaId::Eid"));
        assert!(compact.contains("SchemaId::Uid"));
        assert!(!compact.contains("SchemaId::EID"));
        assert!(!compact.contains("SchemaId::UID"));
        assert!(!compact.contains("OnceLock"));
        assert!(!compact.contains("Vec<"));
    }

    #[test]
    fn missing_duplicate_and_extra_dispositions_are_rejected() {
        for (mutation, diagnostic) in [
            ("missing", "missing"),
            ("duplicate", "duplicate"),
            ("extra", "unknown"),
        ] {
            let fixture = Fixture::new();
            fixture.edit(POLICY, |document| {
                let rows = rows_mut(document, "fields");
                let index = rows
                    .iter()
                    .position(|row| row.get("field").and_then(Value::as_str) == Some("didsha1"))
                    .unwrap();
                match mutation {
                    "missing" => {
                        rows.remove(index);
                    }
                    "duplicate" => {
                        let duplicate = rows[index].clone();
                        rows.push(duplicate);
                    }
                    "extra" => {
                        let mut extra = rows[index].clone();
                        set(&mut extra, "field", "ifv");
                        rows.push(extra);
                    }
                    _ => panic!("unexpected disposition mutation {mutation}"),
                }
            });
            fixture.reject(diagnostic);
        }
    }

    #[test]
    fn missing_duplicate_extra_and_nonrestrictive_schemas_are_rejected() {
        for mutation in ["missing", "duplicate", "extra", "unknown"] {
            let fixture = Fixture::new();
            fixture.edit(POLICY, |document| {
                let schemas = rows_mut(document, "schemas");
                let mut schema = schemas.first().unwrap().clone();
                match mutation {
                    "missing" => {
                        schemas.remove(0);
                    }
                    "duplicate" => schemas.push(schema),
                    "extra" => {
                        set(&mut schema, "object", "InventedSchema");
                        schemas.push(schema);
                    }
                    "unknown" => set(schemas.first_mut().unwrap(), "unknown", "preserve"),
                    _ => panic!("unexpected schema mutation {mutation}"),
                }
            });
            fixture.reject(mutation);
        }
    }

    #[test]
    fn invented_standard_ifv_cannot_be_laundered_by_matching_policy() {
        let fixture = Fixture::new();
        fixture.edit(CATALOGUE, |document| {
            set(
                row_mut(document, "fields", "Device", Some("didsha1")),
                "field",
                "ifv",
            );
        });
        fixture.field("Device", "didsha1", |row| set(row, "field", "ifv"));
        fixture.reject("catalogue");
    }

    #[test]
    fn catalogue_rows_must_reproduce_pinned_types_sections_and_membership() {
        for (key, value) in [
            ("type", "integer"),
            ("section", "3.2.19"),
            ("object", "device"),
        ] {
            let fixture = Fixture::new();
            fixture.edit(CATALOGUE, |document| {
                set(
                    row_mut(document, "fields", "Device", Some("didsha1")),
                    key,
                    value,
                );
            });
            fixture.reject("catalogue");
        }
        let fixture = Fixture::new();
        fixture.edit(CATALOGUE, |document| {
            rows_mut(document, "fields").remove(0);
        });
        fixture.reject("catalogue");
        let duplicate_fixture = Fixture::new();
        duplicate_fixture.edit(CATALOGUE, |document| {
            let fields = rows_mut(document, "fields");
            let duplicate = fields.first().unwrap().clone();
            fields.push(duplicate);
        });
        duplicate_fixture.reject("catalogue");
    }

    #[test]
    fn source_digest_checks_the_bytes_not_only_membership() {
        let fixture = Fixture::new();
        fs::write(
            fixture.dir.join(STANDARD_SOURCE),
            format!(
                "{}\n<!-- changed attribution -->\n",
                fixture.read(STANDARD_SOURCE)
            ),
        )
        .unwrap();
        fixture.reject("digest");
        let wrong_digest = Fixture::new();
        wrong_digest.edit(SOURCE_MANIFEST, |document| {
            set(document, "sha256", &"0".repeat(64));
        });
        wrong_digest.reject("digest");
    }

    #[test]
    fn standard_manifest_requires_the_pinned_revision_and_strict_human_metadata() {
        for (key, value) in [
            ("release", "2.6"),
            ("commit", "abc123"),
            (
                "url",
                "https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/main/2.6.md",
            ),
            ("sha256", "not-hex"),
            ("reviewer", " "),
            ("review_date", "2026-02-29"),
        ] {
            let fixture = Fixture::new();
            fixture.edit(SOURCE_MANIFEST, |document| set(document, key, value));
            fixture.reject(key);
        }
    }

    #[test]
    fn every_raw_document_rejects_unknown_keys_including_authored_domains() {
        for file in [SOURCE_MANIFEST, CATALOGUE, EXTENSIONS, POLICY] {
            let fixture = Fixture::new();
            fixture.edit(file, |document| set(document, "invented_key", "no"));
            fixture.reject("unknown field");
        }
        for table in ["schemas", "fields"] {
            let fixture = Fixture::new();
            fixture.edit(POLICY, |document| {
                rows_mut(document, table)
                    .first_mut()
                    .unwrap()
                    .as_table_mut()
                    .unwrap()
                    .insert("domains".to_owned(), Value::Integer(3));
            });
            fixture.reject("unknown field");
        }
        for table in ["sources", "fields"] {
            let fixture = Fixture::new();
            fixture.edit(EXTENSIONS, |document| {
                set(
                    rows_mut(document, table).first_mut().unwrap(),
                    "invented_key",
                    "no",
                );
            });
            fixture.reject("unknown field");
        }
        let ownership = Fixture::new();
        ownership.edit(EXTENSIONS, |document| {
            set(document.get_mut("ownership").unwrap(), "invented_key", "no");
        });
        ownership.reject("unknown field");
    }

    #[test]
    fn enum_typos_and_policy_shape_disagreement_are_rejected() {
        for (object, field, key, value, diagnostic) in [
            ("Device", "carrier", "shape", "str", "unknown variant"),
            (
                "Device",
                "carrier",
                "action",
                "passthrough",
                "unknown variant",
            ),
            ("Device", "ip", "transform", "ip", "unknown variant"),
            (
                "User",
                "keywords",
                "delegate",
                "generic_fpd",
                "unknown variant",
            ),
            ("Geo", "city", "shape", "object", "shape"),
            ("BrandVersion", "version", "shape", "object_array", "shape"),
            ("Device", "pxratio", "shape", "integer", "shape"),
            ("UserExt", "data", "shape", "object", "shape"),
        ] {
            let fixture = Fixture::new();
            fixture.field(object, field, |row| set(row, key, value));
            fixture.reject(diagnostic);
        }
    }

    #[test]
    fn object_context_requires_declared_child_and_json_is_not_generic_context() {
        for (object, field) in [("Geo", "ext"), ("UserAgent", "ext"), ("UserExt", "data")] {
            let fixture = Fixture::new();
            fixture.field(object, field, |row| {
                set(row, "action", "context");
                if row.get("delegate").is_some() {
                    remove(row, "delegate");
                }
            });
            fixture.reject("context");
        }
        let declared = Fixture::new();
        declared.field("Device", "ext", |row| set(row, "action", "context"));
        let output = declared.compile(review_date());
        let compact: String = output.rust_source.split_whitespace().collect();
        let expected = expected_field("Device.ext Object Context 3 - DeviceExt");
        assert!(
            compact.contains(&expected),
            "missing exact descriptor: {expected}"
        );
    }

    #[test]
    fn unscoped_raw_object_extensions_reject_policy_dispositions() {
        // These RawValue owners are outside the pinned membership inventory.
        for object in ["App", "Dooh", "Banner", "Video", "Native", "Audio"] {
            for action in ["child", "context", "id"] {
                let fixture = Fixture::new();
                fixture.edit(POLICY, |document| {
                    let mut field = row_mut(document, "fields", "Device", Some("ext")).clone();
                    set(&mut field, "object", object);
                    set(&mut field, "action", action);
                    rows_mut(document, "fields").push(field);
                });
                fixture.reject(&format!(
                    "unknown policy disposition {}.ext",
                    object.to_ascii_lowercase()
                ));
            }
        }
    }

    fn expanded_ext_policy(object: &str, action: &str) -> Result<(), Box<dyn Error>> {
        let (policy, inventory) = expanded_slot_policy(object, "ext", "object", action);
        validate_policy_inventory(&policy, &inventory)
    }

    fn expanded_slot_policy(
        object: &str,
        field: &str,
        shape: &str,
        action: &str,
    ) -> (String, TestInventory) {
        let mut inventory =
            BTreeMap::from([(("DeviceExt".to_owned(), "atts".to_owned()), Shape::Integer)]);
        // Admit the wire member first: an unknown-disposition error cannot prove
        // that the compiler rejects traversal through an admitted RawValue slot.
        assert_eq!(
            inventory.insert(
                (object.to_owned(), field.to_owned()),
                Value::String(shape.to_owned()).try_into().unwrap(),
            ),
            None
        );
        let child = if action == "opaque" || shape != "object" {
            ""
        } else {
            "child = \"DeviceExt\""
        };
        let policy = format!(
            "version = 1\nreviewer = \"test\"\nreview_date = \"2026-10-11\"\n\
             [[schemas]]\nobject = \"DeviceExt\"\nunknown = \"remove_restricted\"\n\
             [[schemas]]\nobject = \"{object}\"\nunknown = \"remove_restricted\"\n\
             [[fields]]\nobject = \"DeviceExt\"\nfield = \"atts\"\n\
             shape = \"integer\"\naction = \"control\"\nrationale = \"test control\"\n\
             [[fields]]\nobject = \"{object}\"\nfield = \"{field}\"\n\
             shape = \"{shape}\"\naction = \"{action}\"\nrationale = \"test expansion\"\n{child}\n"
        );
        (policy, inventory)
    }

    #[test]
    fn compiled_raw_and_typed_models_keep_policy_dispositions_separate() {
        use auctioneer_openrtb::ext::misc::DeviceExt;
        use auctioneer_openrtb::field_metadata::FieldMetadata as _;
        use compiled_controls::{Projection, Raw, Typed, relocated};

        let control = DeviceExt::MODEL.unwrap();
        for raw in [
            Raw::MODEL.unwrap(),
            relocated::Model::MODEL.unwrap(),
            Projection::MODEL.unwrap(),
        ] {
            for action in ["child", "context", "id"] {
                let (policy, inventory) = expanded_slot_policy("Foo", "payload", "object", action);
                let diagnostic =
                    super::validate_policy_compiled_models(&policy, &inventory, &[raw, control])
                        .unwrap_err()
                        .to_string();
                assert!(
                    diagnostic.contains("Foo.payload")
                        && diagnostic.contains("raw-object")
                        && diagnostic.contains("opaque"),
                    "{action}: {diagnostic}"
                );
                super::validate_policy_compiled_models(
                    &policy,
                    &inventory,
                    &[Typed::MODEL.unwrap(), control],
                )
                .unwrap();
            }
            let (policy, inventory) = expanded_slot_policy("Foo", "payload", "object", "opaque");
            super::validate_policy_compiled_models(&policy, &inventory, &[raw, control]).unwrap();
        }
    }

    #[test]
    fn compiled_raw_storage_refuses_scalar_dispositions_after_inventory_admission() {
        use auctioneer_openrtb::ext::misc::DeviceExt;
        use auctioneer_openrtb::field_metadata::FieldMetadata as _;
        use compiled_controls::Raw;

        for action in ["context", "id", "control"] {
            let (policy, inventory) = expanded_slot_policy("Foo", "payload", "string", action);
            let diagnostic = super::validate_policy_compiled_models(
                &policy,
                &inventory,
                &[Raw::MODEL.unwrap(), DeviceExt::MODEL.unwrap()],
            )
            .unwrap_err()
            .to_string();
            assert!(
                diagnostic.contains("Foo.payload")
                    && diagnostic.contains("raw-object")
                    && diagnostic.contains("opaque"),
                "{action}: {diagnostic}"
            );
        }
    }

    #[test]
    fn compiled_json_parent_requires_the_real_child_model() {
        use auctioneer_openrtb::ext::misc::DeviceExt;
        use auctioneer_openrtb::field_metadata::FieldMetadata as _;
        use compiled_controls::{CatchAll, Json};

        for parent in [CatchAll::MODEL.unwrap(), Json::MODEL.unwrap()] {
            for action in ["child", "context", "id"] {
                let (policy, inventory) = expanded_slot_policy("Foo", "payload", "object", action);
                super::validate_policy_compiled_models(
                    &policy,
                    &inventory,
                    &[parent, DeviceExt::MODEL.unwrap()],
                )
                .unwrap();
                let diagnostic =
                    super::validate_policy_compiled_models(&policy, &inventory, &[parent])
                        .unwrap_err()
                        .to_string();
                assert_eq!(
                    diagnostic,
                    "DeviceExt.atts Rust storage is missing or unclassified"
                );
            }
        }
    }

    #[test]
    fn compiled_missing_root_model_field_and_named_map_fail_closed() {
        use auctioneer_openrtb::ext::misc::DeviceExt;
        use auctioneer_openrtb::field_metadata::FieldMetadata as _;
        use compiled_controls::{DifferentField, NamedMap};

        for (shape, action) in [
            ("object", "child"),
            ("object", "context"),
            ("object", "id"),
            ("object", "opaque"),
            ("string", "context"),
        ] {
            let (policy, inventory) = expanded_slot_policy("Foo", "payload", shape, action);
            let missing_root = super::validate_policy_compiled_models(&policy, &inventory, &[])
                .unwrap_err()
                .to_string();
            assert_eq!(
                missing_root,
                "DeviceExt.atts Rust storage is missing or unclassified"
            );
            for roots in [
                vec![DeviceExt::MODEL.unwrap()],
                vec![DeviceExt::MODEL.unwrap(), DifferentField::MODEL.unwrap()],
                vec![DeviceExt::MODEL.unwrap(), NamedMap::MODEL.unwrap()],
            ] {
                let diagnostic =
                    super::validate_policy_compiled_models(&policy, &inventory, &roots)
                        .unwrap_err()
                        .to_string();
                assert_eq!(
                    diagnostic,
                    "Foo.payload Rust storage is missing or unclassified"
                );
            }
        }
    }

    #[test]
    fn expanded_raw_object_extensions_reject_traversal() {
        let mut admitted = Vec::new();
        for object in ["App", "Dooh", "Banner", "Video", "Native", "Audio"] {
            for action in ["child", "context", "id"] {
                match expanded_ext_policy(object, action) {
                    Ok(()) => admitted.push(format!("{object}.ext ({action})")),
                    Err(error) => {
                        let diagnostic = error.to_string();
                        assert!(
                            diagnostic.contains(&format!("{object}.ext"))
                                && diagnostic.contains("raw-object")
                                && diagnostic.contains("opaque")
                                && diagnostic.contains("unsupported"),
                            "expected raw-slot rejection, not an inventory error: {diagnostic}"
                        );
                    }
                }
            }
        }
        assert!(
            admitted.is_empty(),
            "admitted RawValue slots unexpectedly accepted traversal: {admitted:?}"
        );
    }

    #[test]
    fn expanded_raw_object_extensions_accept_opaque_dispositions() {
        for object in ["App", "Dooh", "Banner", "Video", "Native", "Audio"] {
            expanded_ext_policy(object, "opaque").unwrap();
        }
    }

    #[test]
    fn compiled_request_graph_has_the_independent_raw_extension_inventory() {
        use auctioneer_openrtb::field_metadata::REQUEST;
        let models = super::compiled_model_graph(REQUEST).unwrap();
        let slots = super::compiled_model_slots(&models);
        let raw: BTreeSet<_> = slots
            .fields
            .iter()
            .filter(|(_, storage)| **storage == super::SlotRepresentation::RawValue)
            .map(|(slot, _)| slot.clone())
            .collect();
        let expected = [
            "App",
            "Audio",
            "Banner",
            "BrandVersion",
            "Dooh",
            "Geo",
            "Native",
            "UID",
            "UserAgent",
            "Video",
        ]
        .map(|object| (object.to_owned(), "ext".to_owned()))
        .into_iter()
        .collect();
        assert_eq!(raw, expected);
    }

    #[test]
    fn compiled_graph_rejects_ancestor_wire_name_conflicts() {
        use auctioneer_openrtb::field_metadata::{Field, Model, Storage};
        struct Parent;
        struct Child;
        const CHILD: &Model = &Model::new::<Child>("Slot", "child", &[]);
        const PARENT: &Model = &Model::new::<Parent>(
            "Slot",
            "parent",
            &[Field {
                name: "nested",
                storage: Storage::Typed,
                flattened: false,
                child: Some(CHILD),
            }],
        );
        let error = super::compiled_model_graph(PARENT).unwrap_err().to_string();
        assert!(
            error.contains("conflicting declarations for wire model Slot"),
            "{error}"
        );
    }

    #[test]
    fn compiled_graph_refuses_conflicting_declarations_and_cycles() {
        use auctioneer_openrtb::field_metadata::{Field, Model, Storage};
        struct Root;
        struct First;
        struct Second;
        const FIRST: &Model = &Model::new::<First>("Slot", "same diagnostic", &[]);
        const SECOND: &Model = &Model::new::<Second>("Slot", "same diagnostic", &[]);
        const CONFLICT: &Model = &Model::new::<Root>(
            "Root",
            "root",
            &[
                Field {
                    name: "first",
                    storage: Storage::Typed,
                    flattened: false,
                    child: Some(FIRST),
                },
                Field {
                    name: "second",
                    storage: Storage::Typed,
                    flattened: false,
                    child: Some(SECOND),
                },
            ],
        );
        static CYCLE: Model = Model::new::<Root>(
            "Cycle",
            "cycle",
            &[Field {
                name: "again",
                storage: Storage::Typed,
                flattened: false,
                child: Some(&CYCLE),
            }],
        );
        const REPEATED: &Model = &Model::new::<Root>(
            "Root",
            "root",
            &[
                Field {
                    name: "first",
                    storage: Storage::Typed,
                    flattened: false,
                    child: Some(FIRST),
                },
                Field {
                    name: "second",
                    storage: Storage::Typed,
                    flattened: false,
                    child: Some(FIRST),
                },
            ],
        );
        let conflict_error = super::compiled_model_graph(CONFLICT)
            .unwrap_err()
            .to_string();
        assert!(
            conflict_error.contains("conflicting declarations for wire model Slot"),
            "{conflict_error}",
        );
        let cycle_error = super::compiled_model_graph(&CYCLE).unwrap_err().to_string();
        assert!(
            cycle_error.contains("recursive compiled model Cycle"),
            "{cycle_error}"
        );
        assert_eq!(super::compiled_model_graph(REPEATED).unwrap().len(), 2);
    }

    #[test]
    fn raw_object_extensions_accept_opaque_dispositions_in_full_compile() {
        let fixture = Fixture::new();
        let output = fixture.compile(review_date());
        let compact: String = output.rust_source.split_whitespace().collect();
        for object in ["UserAgent", "BrandVersion", "Geo", "Uid"] {
            let expected = expected_field(&format!("{object}.ext Object Opaque 3"));
            assert!(
                compact.contains(&expected),
                "missing descriptor: {expected}"
            );
        }
    }

    fn reject_raw_object_extension_traversal(action: &str) {
        let mut admitted = Vec::new();
        for object in ["UserAgent", "BrandVersion", "Geo", "UID"] {
            let fixture = Fixture::new();
            fixture.field(object, "ext", |row| {
                set(row, "action", action);
                set(row, "child", "DeviceExt");
            });
            let label = format!("{object}.ext");
            match compile(&fixture.dir, review_date()) {
                Ok(_) => admitted.push(label),
                Err(error) => {
                    let diagnostic = error.to_string();
                    assert!(
                        diagnostic.contains(&label)
                            && diagnostic.contains("raw-object")
                            && diagnostic.contains("opaque"),
                        "expected raw-object opaque-rule rejection for {label} ({action}): {diagnostic}"
                    );
                }
            }
        }
        assert!(
            admitted.is_empty(),
            "raw-object extensions unexpectedly compiled with {action} traversal: {admitted:?}"
        );
    }

    #[test]
    fn raw_object_extensions_reject_child_traversal() {
        reject_raw_object_extension_traversal("child");
    }

    #[test]
    fn raw_object_extensions_reject_context_traversal() {
        reject_raw_object_extension_traversal("context");
    }

    #[test]
    fn raw_object_extensions_reject_id_traversal() {
        reject_raw_object_extension_traversal("id");
    }

    #[test]
    fn json_backed_eid_extension_still_accepts_traversal_rules() {
        for (action, rule) in [("child", "Child"), ("context", "Context"), ("id", "Id")] {
            let fixture = Fixture::new();
            fixture.field("EID", "ext", |row| {
                set(row, "action", action);
                set(row, "child", "DeviceExt");
            });
            let output = fixture.compile(review_date());
            let compact: String = output.rust_source.split_whitespace().collect();
            let expected = expected_field(&format!("Eid.ext Object {rule} 3 - DeviceExt"));
            assert!(
                compact.contains(&expected),
                "missing exact descriptor: {expected}"
            );
        }
    }

    #[test]
    fn typed_object_extensions_still_accept_traversal_rules() {
        for (object, child, domains) in [("Device", "DeviceExt", 3_u8), ("User", "UserExt", 7_u8)] {
            for (action, rule) in [("child", "Child"), ("context", "Context"), ("id", "Id")] {
                let fixture = Fixture::new();
                fixture.field(object, "ext", |row| set(row, "action", action));
                let output = fixture.compile(review_date());
                let compact: String = output.rust_source.split_whitespace().collect();
                let expected =
                    expected_field(&format!("{object}.ext Object {rule} {domains} - {child}"));
                assert!(
                    compact.contains(&expected),
                    "missing exact descriptor: {expected}"
                );
            }
        }
    }

    #[test]
    fn child_links_must_exist_and_belong_to_object_shapes() {
        for mutation in ["missing", "absent schema", "scalar", "opaque"] {
            let fixture = Fixture::new();
            match mutation {
                "missing" => fixture.field("Device", "geo", |row| remove(row, "child")),
                "absent schema" => {
                    fixture.field("Device", "geo", |row| set(row, "child", "MissingSchema"));
                }
                "scalar" => fixture.field("Device", "carrier", |row| set(row, "child", "Geo")),
                "opaque" => fixture.field("Geo", "ext", |row| set(row, "child", "Geo")),
                _ => panic!("unexpected child-link mutation {mutation}"),
            }
            fixture.reject("child");
        }
    }

    #[test]
    fn all_eid_uid_collections_are_id_gated_and_have_element_schemas() {
        for (object, field) in [("User", "eids"), ("UserExt", "eids"), ("EID", "uids")] {
            let fixture = Fixture::new();
            fixture.field(object, field, |row| remove(row, "child"));
            fixture.reject("child");
            let unguarded = Fixture::new();
            unguarded.field(object, field, |row| set(row, "action", "child"));
            unguarded.reject("id");
        }
    }

    #[test]
    fn transforms_require_the_exact_family_shape_and_no_other_rule_attachment() {
        for (object, field, mutation, diagnostic) in [
            ("Device", "ip", "missing", "transform"),
            ("Geo", "lat", "ipv4", "transform"),
            ("Device", "ipv6", "ipv4", "transform"),
            ("Device", "carrier", "coordinate", "transform"),
        ] {
            let fixture = Fixture::new();
            fixture.field(object, field, |row| {
                if mutation == "missing" {
                    remove(row, "transform");
                } else {
                    set(row, "transform", mutation);
                }
            });
            fixture.reject(diagnostic);
        }
    }

    #[test]
    fn delegation_requires_exactly_the_four_owned_fpd_branches() {
        let missing = Fixture::new();
        missing.field("User", "keywords", |row| remove(row, "delegate"));
        missing.reject("delegate");
        let unowned = Fixture::new();
        unowned.field("User", "customdata", |row| {
            set(row, "action", "delegate");
            set(row, "delegate", "user_fpd");
        });
        unowned.reject("fpd");
        let reclassified = Fixture::new();
        reclassified.field("User", "kwarray", |row| {
            remove(row, "delegate");
            set(row, "action", "context");
        });
        reclassified.reject("fpd");
        let attached = Fixture::new();
        attached.field("Device", "carrier", |row| set(row, "delegate", "user_fpd"));
        attached.reject("delegate");
    }

    #[test]
    fn self_and_indirect_schema_cycles_are_rejected() {
        let direct = Fixture::new();
        direct.field("Device", "ext", |row| set(row, "child", "Device"));
        direct.reject("cycle");
        let indirect = Fixture::new();
        indirect.field("DeviceExtPrebid", "interstitial", |row| {
            set(row, "child", "Device");
        });
        indirect.reject("cycle");
    }

    #[test]
    fn extension_artifact_and_license_digests_are_enforced() {
        for file in [
            "disclosure-sources/device-extension.source",
            "disclosure-sources/user-extension.source",
            "disclosure-sources/ownership-classification.md",
            "disclosure-sources/LICENSE-extensions",
        ] {
            let fixture = Fixture::new();
            fs::write(
                fixture.dir.join(file),
                format!("{}\nchanged\n", fixture.read(file)),
            )
            .unwrap();
            fixture.reject("digest");
        }
        for key in ["sha256", "license_sha256"] {
            let fixture = Fixture::new();
            fixture.edit(EXTENSIONS, |document| {
                set(source_mut(document, "device-extension"), key, "not-hex");
            });
            fixture.reject("digest");
        }
    }

    #[test]
    fn extension_metadata_requires_full_immutable_references_and_review() {
        for (key, value, diagnostic) in [
            ("commit", "d7f8515", "commit"),
            (
                "url",
                "https://github.com/prebid/prebid-server/blob/main/openrtb_ext/device.go",
                "url",
            ),
            (
                "url",
                "https://github.com/prebid/prebid-server/blob/0000000000000000000000000000000000000000/openrtb_ext/device.go",
                "commit",
            ),
            ("reviewer", "", "reviewer"),
            ("review_date", "2026-13-11", "review_date"),
        ] {
            let fixture = Fixture::new();
            fixture.edit(EXTENSIONS, |document| {
                set(source_mut(document, "device-extension"), key, value);
            });
            fixture.reject(diagnostic);
        }
    }

    #[test]
    fn extension_artifact_paths_cannot_be_absolute_or_escape_the_fixture() {
        for key in ["artifact", "license"] {
            for path in [
                "/etc/passwd",
                "../disclosure-outside",
                "disclosure-sources/../../disclosure-outside",
            ] {
                let fixture = Fixture::new();
                fixture.edit(EXTENSIONS, |document| {
                    set(source_mut(document, "device-extension"), key, path);
                });
                fixture.reject("path");
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn extension_symlink_cannot_escape_even_with_matching_digest() {
        let fixture = Fixture::new();
        let outside = Fixture::new();
        let artifact = "disclosure-sources/device-extension.source";
        fs::remove_file(fixture.dir.join(artifact)).unwrap();
        symlink(outside.dir.join(artifact), fixture.dir.join(artifact)).unwrap();
        fixture.reject("path");
    }

    #[test]
    fn extension_source_ids_and_member_provenance_must_be_unique_and_known() {
        let duplicate_source = Fixture::new();
        duplicate_source.edit(EXTENSIONS, |document| {
            let sources = rows_mut(document, "sources");
            let duplicate = sources.first().unwrap().clone();
            sources.push(duplicate);
        });
        duplicate_source.reject("duplicate");
        let duplicate_field = Fixture::new();
        duplicate_field.edit(EXTENSIONS, |document| {
            let fields = rows_mut(document, "fields");
            let duplicate = fields.first().unwrap().clone();
            fields.push(duplicate);
        });
        duplicate_field.reject("duplicate");
        let unknown_source = Fixture::new();
        unknown_source.edit(EXTENSIONS, |document| {
            set(
                row_mut(document, "fields", "DeviceExt", Some("atts")),
                "source",
                "absent-source",
            );
        });
        unknown_source.reject("source");
        let empty_reference = Fixture::new();
        empty_reference.edit(EXTENSIONS, |document| {
            set(
                row_mut(document, "fields", "DeviceExt", Some("atts")),
                "reference",
                " ",
            );
        });
        empty_reference.reject("reference");
    }

    #[test]
    fn reviewed_binding_rejects_known_but_wrong_extension_source() {
        let fixture = Fixture::new();
        fixture.edit(EXTENSIONS, |document| {
            set(
                row_mut(document, "fields", "UserExt", Some("eids")),
                "source",
                "device-extension",
            );
        });
        fixture.reject("binding");
    }

    #[test]
    fn reviewed_binding_rejects_misspelled_declaration_reference() {
        let fixture = Fixture::new();
        fixture.edit(EXTENSIONS, |document| {
            set(
                row_mut(document, "fields", "DeviceExt", Some("atts")),
                "reference",
                "ExtDevice.Typo",
            );
        });
        fixture.reject("binding");
    }

    #[test]
    fn reviewed_binding_rejects_wrong_type_even_when_policy_agrees() {
        let fixture = Fixture::new();
        fixture.edit(EXTENSIONS, |document| {
            set(
                row_mut(document, "fields", "DeviceExt", Some("atts")),
                "type",
                "string",
            );
        });
        fixture.field("DeviceExt", "atts", |row| set(row, "shape", "string"));
        fixture.reject("binding");
    }

    #[test]
    fn reviewed_binding_rejects_matching_digest_artifact_and_license_rewrites() {
        for (source_id, artifact) in [
            (
                Some("user-extension"),
                "disclosure-sources/user-extension.source",
            ),
            (
                Some("device-extension"),
                "disclosure-sources/device-extension.source",
            ),
            (
                Some("ownership"),
                "disclosure-sources/ownership-classification.md",
            ),
            (None, "disclosure-sources/LICENSE-extensions"),
        ] {
            let fixture = Fixture::new();
            let mut bytes = fs::read(fixture.dir.join(artifact)).unwrap();
            *bytes.first_mut().unwrap() = b'X';
            fs::write(fixture.dir.join(artifact), &bytes).unwrap();
            let changed_digest = hex::encode(Sha256::digest(&bytes));
            fixture.edit(EXTENSIONS, |document| match source_id {
                Some(id) => set(source_mut(document, id), "sha256", &changed_digest),
                None => {
                    for id in ["user-extension", "device-extension"] {
                        set(source_mut(document, id), "license_sha256", &changed_digest);
                    }
                }
            });
            fixture.reject("binding");
        }
    }

    #[test]
    fn reviewed_binding_rejects_origin_and_excerpt_range_drift() {
        for mutation in ["repository", "remote file", "commit", "source id", "range"] {
            let fixture = Fixture::new();
            fixture.edit(EXTENSIONS, |document| match mutation {
                "repository" | "remote file" | "commit" => {
                    let source = source_mut(document, "device-extension");
                    let current_url = source.get("url").unwrap().as_str().unwrap();
                    let changed_url = match mutation {
                        "repository" => {
                            current_url.replace("prebid/prebid-server", "other/repository")
                        }
                        "remote file" => {
                            current_url.replace("openrtb_ext/device.go", "openrtb_ext/other.go")
                        }
                        "commit" => {
                            let changed_commit = "0000000000000000000000000000000000000000";
                            let previous_commit = source.get("commit").unwrap().as_str().unwrap();
                            let url = current_url.replace(previous_commit, changed_commit);
                            set(source, "commit", changed_commit);
                            url
                        }
                        _ => panic!("unexpected source-origin mutation {mutation}"),
                    };
                    set(source, "url", &changed_url);
                }
                "source id" => {
                    set(
                        source_mut(document, "device-extension"),
                        "id",
                        "alternate-device",
                    );
                    for row in rows_mut(document, "fields") {
                        if row.get("source").and_then(Value::as_str) == Some("device-extension") {
                            set(row, "source", "alternate-device");
                        }
                    }
                }
                "range" => {
                    let source = source_mut(document, "ownership").as_table_mut().unwrap();
                    source.insert("first_line".to_owned(), Value::Integer(196));
                    source.insert("last_line".to_owned(), Value::Integer(201));
                }
                _ => panic!("unexpected provenance mutation {mutation}"),
            });
            fixture.reject("binding");
        }
    }

    #[test]
    fn reviewed_binding_rejects_missing_or_extra_members_with_matching_policy_edits() {
        for mutation in ["missing", "extra"] {
            let fixture = Fixture::new();
            for artifact in [EXTENSIONS, POLICY] {
                fixture.edit(artifact, |document| {
                    let rows = rows_mut(document, "fields");
                    let index = rows
                        .iter()
                        .position(|row| {
                            row.get("object").and_then(Value::as_str) == Some("DeviceExt")
                                && row.get("field").and_then(Value::as_str) == Some("atts")
                        })
                        .unwrap();
                    match mutation {
                        "missing" => {
                            rows.remove(index);
                        }
                        "extra" => {
                            let mut row = rows[index].clone();
                            set(&mut row, "field", "unreviewed");
                            rows.push(row);
                        }
                        _ => panic!("unexpected membership mutation {mutation}"),
                    }
                });
            }
            fixture.reject("binding");
        }
    }

    #[test]
    fn external_sources_need_complete_license_pairs_and_owned_excerpt_ranges() {
        for key in ["license", "license_sha256"] {
            let fixture = Fixture::new();
            fixture.edit(EXTENSIONS, |document| {
                remove(source_mut(document, "device-extension"), key);
            });
            fixture.reject("license");
        }
        for key in ["first_line", "last_line"] {
            let fixture = Fixture::new();
            fixture.edit(EXTENSIONS, |document| {
                source_mut(document, "ownership")
                    .as_table_mut()
                    .unwrap()
                    .insert(key.to_owned(), Value::Integer(0));
            });
            fixture.reject("line");
        }
    }

    #[test]
    fn fpd_and_tid_ownership_sets_cannot_be_shortened_extended_or_duplicated() {
        for key in ["user_fpd", "transaction_ids"] {
            for mutation in ["missing", "extra", "duplicate"] {
                let fixture = Fixture::new();
                fixture.edit(EXTENSIONS, |document| {
                    let entries = document
                        .get_mut("ownership")
                        .unwrap()
                        .get_mut(key)
                        .unwrap()
                        .as_array_mut()
                        .unwrap();
                    match mutation {
                        "missing" => {
                            entries.remove(0);
                        }
                        "extra" => entries.push(Value::String("user.customdata".to_owned())),
                        "duplicate" => {
                            let duplicate = entries.first().unwrap().clone();
                            entries.push(duplicate);
                        }
                        _ => panic!("unexpected ownership mutation {mutation}"),
                    }
                });
                fixture.reject("ownership");
            }
        }
        let missing_source = Fixture::new();
        missing_source.edit(EXTENSIONS, |document| {
            set(
                document.get_mut("ownership").unwrap(),
                "source",
                "missing-source",
            );
        });
        missing_source.reject("ownership");
    }

    #[test]
    fn policy_metadata_rationale_and_versions_are_not_optional() {
        for (key, value) in [("reviewer", " "), ("review_date", "2026-02-29")] {
            let fixture = Fixture::new();
            fixture.edit(POLICY, |document| set(document, key, value));
            fixture.reject(key);
        }
        let rationale = Fixture::new();
        rationale.field("Device", "carrier", |row| set(row, "rationale", " "));
        rationale.reject("rationale");
        for file in [POLICY, EXTENSIONS] {
            let fixture = Fixture::new();
            fixture.edit(file, |document| {
                document
                    .as_table_mut()
                    .unwrap()
                    .insert("version".to_owned(), Value::Integer(2));
            });
            fixture.reject("version");
        }
    }

    #[test]
    fn review_age_warns_after_one_hundred_eighty_days_without_changing_output_or_inputs() {
        let fixture = Fixture::new();
        let before = fixture.snapshot();
        let fresh = fixture.compile(review_date());
        for days in [-1_i64, 179, 180, 181] {
            let today = review_date().checked_add(Duration::days(days)).unwrap();
            let output = fixture.compile(today);
            assert_eq!(output.rust_source, fresh.rust_source);
            if days > 180 {
                assert_eq!(output.warnings.len(), 5);
                assert!(
                    output
                        .warnings
                        .iter()
                        .all(|warning| warning.contains("180"))
                );
            } else {
                assert!(output.warnings.is_empty());
            }
        }
        assert_eq!(fixture.snapshot(), before);
    }

    #[test]
    fn repeated_generation_is_byte_identical_across_roots_and_input_row_order() {
        let first = Fixture::new();
        let second = Fixture::new();
        second.edit(POLICY, |document| {
            rows_mut(document, "fields").reverse();
            rows_mut(document, "schemas").reverse();
        });
        second.edit(EXTENSIONS, |document| {
            rows_mut(document, "fields").reverse();
            rows_mut(document, "sources").reverse();
        });
        let first_snapshot = first.snapshot();
        let second_snapshot = second.snapshot();
        let initial = first.compile(review_date());
        let repeated = first.compile(review_date());
        let reordered = second.compile(review_date().checked_add(Duration::days(179)).unwrap());
        assert_eq!(initial, repeated);
        assert_eq!(initial, reordered);
        assert_eq!(first.snapshot(), first_snapshot);
        assert_eq!(second.snapshot(), second_snapshot);
    }
}
