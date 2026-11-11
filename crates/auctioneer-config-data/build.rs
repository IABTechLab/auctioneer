//! Reads bidder-owned manifests and `data/*`, then emits `OUT_DIR/generated.rs`.
//!
//! Compiled development URL defaults require the owning manifest's
//! `insecure-bidders` gate; publication follows the active Cargo features.
//! A default-disabled factory with typed endpoint settings may instead omit
//! its compiled destination and require explicit operator configuration.
//!
//! Pure rendering logic (TOML types + `render_*` helpers) lives in
//! `auctioneer-config`'s `build-support` library modules. This file keeps
//! the coordinators that read TOML off disk, validate URLs / hostAllowlist entries, and
//! stitch imports + module boilerplate around the rendered struct
//! literals.
use auctioneer_config::{
    bidder_manifest::SettingKind, build_catalog, build_disclosure as disclosure, codegen,
};

use auctioneer_gvl_schema::{Manifest, ManifestEntry, validate_gvl_json, validate_manifest};
use codegen::build_support::{
    EndpointAuthorityKind, ValidationResult, classify_endpoint_authority, validate_endpoint_url,
    validate_syncer_template,
};
use codegen::{
    AccountRaw, BidderInfoRaw, BuildResult, HostDefaultRaw, account_default_type_imports,
    host_default_type_imports, render_account_config, render_bidder_info,
    render_bidder_type_imports, render_host_config,
};
use fs2::FileExt as _;
use proc_macro2::TokenStream;
use quote::quote;
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::str;
use time::{Duration, OffsetDateTime};
use validator::Validate as _;

fn main() -> BuildResult {
    let out_dir = env::var("OUT_DIR")?;
    let out_path = Path::new(&out_dir).join("generated.rs");

    let mut out = TokenStream::new();
    // The names `emit_bidders` actually loaded are the registry this
    // build ships, and `emit_host_default` validates its `[aliases]`
    // rows against exactly that list rather than re-reading manifests —
    // a second read could disagree under the `insecure-bidders` feature.
    let bidder_names = emit_bidders(&mut out)?;
    emit_product_defaults(&mut out, &bidder_names)?;
    emit_eea_set(&mut out)?;
    emit_disclosure(&out_dir)?;

    fs::write(&out_path, out.to_string())?;

    emit_gvl_archive(&out_dir)?;

    println!("cargo:rerun-if-changed=data");
    Ok(())
}

fn emit_product_defaults(out: &mut TokenStream, bidder_names: &[String]) -> BuildResult {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct ProductDefaults {
        host: HostDefaultRaw,
        account: AccountRaw,
    }

    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let committed = format!("{manifest_dir}/data/product-defaults.toml");
    let harness_enabled = env::var_os("CARGO_FEATURE_TEST_HARNESS_OVERRIDE").is_some();
    let path = if harness_enabled {
        println!("cargo:rerun-if-env-changed=AUCTIONEER__DEFAULTS__PATH_OVERRIDE");
        env::var("AUCTIONEER__DEFAULTS__PATH_OVERRIDE").unwrap_or(committed)
    } else {
        committed
    };
    println!("cargo:rerun-if-changed={path}");
    let raw = fs::read_to_string(&path).map_err(|err| format!("read {path}: {err}"))?;
    let source: ProductDefaults =
        toml::from_str(&raw).map_err(|err| format!("parse {path}: {err}"))?;
    // Explicit harness fixtures exercise non-neutral renderer/privacy branches.
    // Production never reads their redirect and must ship no operator service.
    if !harness_enabled
        && (source.host.user_sync.is_some()
            || !source.host.aliases.is_empty()
            || source.host.gdpr.host_vendor_id != 0
            || !source.host.gdpr.non_standard_publishers.is_empty()
            || source.host.host_cookie.domain.is_some()
            || source.host.host_cookie.optout_cookie.is_some()
            || !source.host.status_response.is_empty())
    {
        return Err("product defaults must not configure operator identities or services".into());
    }
    let imports = emit_host_default(out, &path, &source.host, bidder_names)?;
    emit_account_default(out, &path, &source.account, &imports)?;
    out.extend(quote! {
        pub use self::{
            account_default as product_account_default, host_default as product_host_default,
        };
    });
    Ok(())
}

fn emit_disclosure(out_dir: &str) -> BuildResult {
    let data_dir = if env::var("CARGO_FEATURE_TEST_HARNESS_OVERRIDE").is_ok() {
        println!("cargo:rerun-if-env-changed=AUCTIONEER__DISCLOSURE__DIR");
        env::var("AUCTIONEER__DISCLOSURE__DIR").unwrap_or_else(|_error| "data".to_owned())
    } else {
        "data".to_owned()
    };
    println!("cargo:rerun-if-changed={data_dir}");
    let compiled = disclosure::compile(Path::new(&data_dir), OffsetDateTime::now_utc().date())?;
    for warning in compiled.warnings {
        println!("cargo:warning=disclosure: {warning}");
    }
    fs::write(
        Path::new(out_dir).join("disclosure.rs"),
        compiled.rust_source,
    )?;
    Ok(())
}

/// Emits `bidders()` / `bidder()` and returns the bidder names it
/// loaded, in the order they were emitted. The caller passes that list
/// on to [`emit_host_default`] so host-declared aliases are checked
/// against the SAME registry this build bakes in.
fn emit_bidders(out: &mut TokenStream) -> BuildResult<Vec<String>> {
    // Only the harness may replace the committed sibling bidder tree.
    // Construct the sibling path without lexical `..` components so the
    // shared reader can validate every ancestor without following links.
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let default_root = Path::new(&manifest_dir)
        .parent()
        .ok_or("metadata crate must have a parent directory")?
        .join("auctioneer-bidder/src/bidders");
    let root = if env::var("CARGO_FEATURE_TEST_HARNESS_OVERRIDE").is_ok() {
        println!("cargo:rerun-if-env-changed=AUCTIONEER__BIDDERS__DIR");
        env::var_os("AUCTIONEER__BIDDERS__DIR").map_or(default_root, PathBuf::from)
    } else {
        default_root
    };
    let features = metadata_features()?;
    let catalog = build_catalog::scan(&root, &features)
        .map_err(|error| -> Box<dyn Error> { error.to_string().into() })?;
    for path in &catalog.input_paths {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let mut bidders = load_bidders_from(&root, catalog)?;
    let generated_bindings = codegen::render_bidder_bindings(&bidders, &features)?;
    let declaration_count = bidders.len();
    let binding_items = syn::parse_file(&generated_bindings)?;
    bidders.retain(|(_name, info)| {
        info.required_feature.as_ref().is_none_or(|feature| {
            let cargo_name = feature.to_uppercase().replace('-', "_");
            env::var_os(format!("CARGO_FEATURE_{cargo_name}")).is_some()
        })
    });
    codegen::usersync::validate_catalog_size(bidders.len(), 0)?;
    bidders.sort_by(|left, right| left.0.cmp(&right.0));

    let type_imports = syn::parse_file(&render_bidder_type_imports(&bidders))?;
    // Hash the original constructor text, not a reserialized syntax tree.
    let mut constructors = Vec::with_capacity(bidders.len());
    let mut constructor_exprs = Vec::with_capacity(bidders.len());
    for (name, info) in &bidders {
        // Surface the typed `RenderError` through `Display`, as the
        // host and account emitters do: cargo prints a build script's
        // boxed error with `Debug`, and the struct-literal dump is not
        // a message an operator can act on.
        let literal = render_bidder_info(name, info)
            .map_err(|err| -> Box<dyn Error> { format!("bidder {name:?}: {err}").into() })?;
        let expression = syn::parse_str::<syn::Expr>(&literal)?;
        let gate = codegen::publication_gate(info.native_only, info.required_feature.as_deref());
        constructor_exprs.push(quote!(#gate #expression));
        constructors.push((info.native_only, literal));
    }
    let storage = codegen::storage::render_target_metadata_storage(
        &constructors,
        &generated_bindings,
        declaration_count,
    )
    .map_err(|error| -> Box<dyn Error> { error.to_string().into() })?;
    let storage_items = syn::parse_file(&storage)?;
    out.extend(quote! {
        #binding_items
        #type_imports
        use std::sync::OnceLock;

        #[must_use]
        pub fn bidders() -> &'static [BidderInfo] {
            static CELL: OnceLock<Vec<BidderInfo>> = OnceLock::new();
            CELL.get_or_init(|| vec![#(#constructor_exprs,)*])
        }

        #storage_items

        #[must_use]
        pub fn bidder(name: &str) -> Option<&'static BidderInfo> {
            let index = bidders().binary_search_by(|info| info.name.as_str().cmp(name)).ok()?;
            bidders().get(index)
        }
    });
    let guest = env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32");
    Ok(bidders
        .into_iter()
        .filter(|(_, info)| !guest || !info.native_only)
        .map(|(name, _info)| name)
        .collect())
}

fn load_bidders_from(
    root: &Path,
    catalog: build_catalog::Catalog,
) -> BuildResult<Vec<(String, BidderInfoRaw)>> {
    let mut bidders = Vec::new();
    for manifest in catalog.manifests.into_values() {
        let path = root.join(&manifest.key).join("bidder.toml");
        let allow_insecure = manifest.required_feature.as_deref() == Some("insecure-bidders");
        let supported_endpoint_macros: Vec<String> = manifest
            .endpoint
            .as_ref()
            .map(|endpoint| endpoint.fields.values().cloned().collect())
            .unwrap_or_default();
        for (name, definition) in manifest.definitions {
            let mut parsed = parse_bidder_metadata(
                &path,
                definition,
                &manifest.key,
                manifest.required_feature.as_deref(),
            )?;
            parsed
                .supported_endpoint_macros
                .clone_from(&supported_endpoint_macros);
            parsed.native_only = manifest.native_only;

            match parsed.endpoint.as_deref() {
                Some(endpoint) => {
                    validate_endpoint_authority(&name, endpoint, allow_insecure)?;
                    validate_host_allowlist(
                        &name,
                        endpoint,
                        parsed.host_allowlist.as_deref(),
                        allow_insecure,
                    )?;
                }
                None if !manifest.factory
                    || parsed.default_enabled != Some(false)
                    || !manifest.settings.as_ref().is_some_and(|settings| {
                        matches!(
                            settings.fields.get("endpoint"),
                            Some(SettingKind::OptionalString)
                        )
                    })
                    || parsed.host_allowlist.is_some() =>
                {
                    return Err(format!("bidder {name:?}: missing endpoint requires a default-disabled factory with typed endpoint settings and no hostAllowlist").into());
                }
                None => {}
            }
            validate_bidder_enums(&name, &parsed)?;
            codegen::usersync::validate_skip_when(&parsed.user_sync.skip_when)?;
            codegen::usersync::validate_syncer_key(parsed.user_sync.key.as_deref())?;
            for endpoint in [
                parsed.user_sync.iframe.as_ref(),
                parsed.user_sync.redirect.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                codegen::usersync::validate_user_macro(endpoint.user_macro.as_deref())?;
            }

            if let Some(iframe) = parsed.user_sync.iframe.as_ref() {
                translate_syncer_result(&name, "userSync.iframe.url", &iframe.url, allow_insecure)?;
            }
            if let Some(redirect) = parsed.user_sync.redirect.as_ref() {
                translate_syncer_result(
                    &name,
                    "userSync.redirect.url",
                    &redirect.url,
                    allow_insecure,
                )?;
            }

            // Inactive definitions must pass the complete renderer too.
            drop(
                render_bidder_info(&name, &parsed).map_err(|error| -> Box<dyn Error> {
                    format!("bidder {name:?}: {error}").into()
                })?,
            );

            bidders.push((name, parsed));
        }
    }
    Ok(bidders)
}

fn parse_bidder_metadata(
    path: &Path,
    mut definition: toml::Table,
    implementation: &str,
    required_feature: Option<&str>,
) -> BuildResult<BidderInfoRaw> {
    definition.insert("implementation".into(), implementation.into());
    if let Some(feature) = required_feature {
        definition.insert("required_feature".into(), feature.into());
    }
    definition.try_into().map_err(|_error| {
        format!(
            "parse bidder metadata {}: invalid bidder metadata fields",
            path.display()
        )
        .into()
    })
}

fn metadata_features() -> BuildResult<BTreeSet<String>> {
    println!("cargo:rerun-if-changed=Cargo.toml");
    let manifest: toml::Value = toml::from_str(&fs::read_to_string("Cargo.toml")?)?;
    let features = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or("metadata Cargo manifest must declare its features")?;
    Ok(features.keys().cloned().collect())
}

/// Codegen-time validation for the bidder `endpoint` field. Splits
/// out the authority classification (only the literal authority + the
/// exact `{{.Host}}` token are allowed in the scheme/host slot)
/// from the URL-shape / scheme / IP-class checks that
/// `validate_endpoint_url` runs on the fully-resolved authority.
fn validate_endpoint_authority(name: &str, endpoint: &str, allow_insecure: bool) -> BuildResult {
    match classify_endpoint_authority(endpoint) {
        EndpointAuthorityKind::Fixed => {
            translate_endpoint_result(name, "endpoint", endpoint, allow_insecure)
        }
        EndpointAuthorityKind::HostVarying => {
            // Substitute a placeholder for `{{.Host}}` so url::Url
            // can parse the rest of the URL. The actual host is
            // validated per-allow-list-entry by
            // validate_host_allowlist; here we only verify the
            // scheme + path shape are legal.
            let stand_in =
                endpoint.replace("{{.Host}}", "auctioneer-host-varying-stand-in.invalid");
            translate_endpoint_result(name, "endpoint", &stand_in, allow_insecure)
        }
        EndpointAuthorityKind::UnsupportedMacro(_) => Err(format!(
            "bidder {name:?} endpoint authority contains an \
             unsupported template macro. Scheme and host are taken verbatim from the \
             endpoint (or, when the authority is exactly `{{{{.Host}}}}`, from a member \
             of `hostAllowlist`); no other macros may appear in scheme or host.",
        )
        .into()),
    }
}

/// Allow-list of recognized `endpointCompression` values: only `gzip`
/// is recognized; everything else (typos, future upstream additions we
/// haven't audited) MUST fail codegen rather than silently downgrade
/// to `None`.
const ALLOWED_ENDPOINT_COMPRESSIONS: &[&str] = &["gzip"];

/// Allow-list of recognized capability `mediaTypes` values per
/// `OpenRTB` 2.6 section 3.2.2. `BidderInfo` only knows these four shapes;
/// anything else (typos, future formats we haven't wired) MUST fail
/// codegen rather than silently drop.
const ALLOWED_MEDIA_TYPES: &[&str] = &["banner", "video", "native", "audio"];

/// Every TOML-supplied enum value the codegen routes through a
/// `match ... _ => default` MUST be allow-listed at validation time.
/// Otherwise a typo or upstream addition silently downgrades
/// (`endpointCompression: gizp` -> `None`, `mediaTypes: [vide]` ->
/// empty).
fn validate_bidder_enums(name: &str, info: &BidderInfoRaw) -> BuildResult {
    if let Some(compression) = info.endpoint_compression.as_deref()
        && !ALLOWED_ENDPOINT_COMPRESSIONS.contains(&compression)
    {
        return Err(format!(
            "bidder {name:?} endpointCompression is not on the \
                 recognized allow-list ({ALLOWED_ENDPOINT_COMPRESSIONS:?}). The codegen \
                 would silently downgrade to no-compression; fix the TOML or extend \
                 the allow-list."
        )
        .into());
    }
    for (label, shape_opt) in [
        (
            "capabilities.site.mediaTypes",
            info.capabilities.site.as_ref(),
        ),
        (
            "capabilities.app.mediaTypes",
            info.capabilities.app.as_ref(),
        ),
        (
            "capabilities.dooh.mediaTypes",
            info.capabilities.dooh.as_ref(),
        ),
    ] {
        let Some(shape) = shape_opt else {
            continue;
        };
        for media_type in &shape.media_types {
            if !ALLOWED_MEDIA_TYPES.contains(&media_type.as_str()) {
                return Err(format!(
                    "bidder {name:?} {label} entry is not on the recognized \
                     allow-list ({ALLOWED_MEDIA_TYPES:?}). The codegen would silently drop \
                     it; fix the TOML or extend the allow-list."
                )
                .into());
            }
        }
    }
    Ok(())
}

/// Translate the typed `ValidationResult` into a build-time error
/// message the cargo runtime surfaces verbatim. The rule set itself
/// lives in `codegen::build_support::validate_endpoint_url` so it can
/// be unit-tested from `tests/url_safety.rs` without spinning up a
/// child cargo build.
fn translate_endpoint_result(
    name: &str,
    field: &str,
    url: &str,
    allow_insecure: bool,
) -> BuildResult {
    match validate_endpoint_url(url, allow_insecure) {
        ValidationResult::Ok => Ok(()),
        ValidationResult::MalformedUrl => Err(format!(
            "bidder {name:?} {field} is malformed (parse failed or empty host). \
             Provide a fully-qualified URL with a non-empty host.",
        )
        .into()),
        ValidationResult::UnsupportedScheme => Err(format!(
            "bidder {name:?} {field} has unsupported scheme -- must be http:// or https://."
        )
        .into()),
        ValidationResult::HttpInDefaultBuild => Err(format!(
            "bidder {name:?} {field} uses http:// without an insecure-bidders requirement. \
             Use https:// or require insecure-bidders on the owning bidder manifest \
             (only compiled with --features insecure-bidders).",
        )
        .into()),
        ValidationResult::NonLocalhostHttpUnderInsecure => Err(format!(
            "bidder {name:?} {field} uses http:// to a host other than the three \
             loopback literals (127.0.0.1, ::1, localhost). The insecure-bidders relaxation \
             is exact-literal only -- anywhere else in 127.0.0.0/8 is rejected.",
        )
        .into()),
        ValidationResult::NonLiteralLoopbackUnderInsecure => Err(format!(
            "bidder {name:?} {field} targets a loopback host that is NOT one of the \
             three exact-literal allow-list entries (127.0.0.1, ::1, localhost). The \
             insecure-bidders feature narrows the loopback rejection to those three \
             literals -- every other host in 127.0.0.0/8 (e.g. 127.0.0.2) and every alias \
             that resolves to loopback stays rejected even under the feature, regardless \
             of scheme.",
        )
        .into()),
        ValidationResult::LocalhostAlias => Err(format!(
            "bidder {name:?} {field} targets a DNS-trick loopback alias (e.g. \
             127.0.0.1.nip.io, *.localhost, *.nip.io, *.xip.io, *.lvh.me, *.localtest.me, \
             *.vcap.me). Default builds reject them, and the insecure-bidders feature-on \
             allow-list is exact-literal only (127.0.0.1, ::1, localhost) so aliases are \
             not allowed under the feature either.",
        )
        .into()),
        ValidationResult::LoopbackInDefaultBuild => Err(format!(
            "bidder {name:?} {field} targets a loopback authority without an \
             insecure-bidders requirement. Loopback (127.0.0.0/8 / [::1] / localhost) \
             requires insecure-bidders on the owning bidder manifest and \
             --features insecure-bidders, even over HTTPS.",
        )
        .into()),
        ValidationResult::PrivateIpAuthority => Err(format!(
            "bidder {name:?} {field} has a hard-coded private-IP authority. Use a \
             hostname or move behind a reverse proxy.",
        )
        .into()),
        ValidationResult::AuthorityMacro
        | ValidationResult::TemplateTooLong
        | ValidationResult::SyncerUserinfoOrFragment
        | ValidationResult::UnknownMacro(_)
        | ValidationResult::UnclosedMacro(_) => {
            // AuthorityMacro / UnknownMacro / UnclosedMacro are only
            // returned by validate_syncer_template; seeing them here
            // is an internal build_support bug, not bad input.
            Err(
                "syncer-only validation result returned by validate_endpoint_url -- internal error"
                    .into(),
            )
        }
    }
}

/// Codegen-time syncer-URL validation. Delegates to the
/// `codegen::validate_syncer_template` helper; same rule set as
/// `validate_endpoint_url` plus authority-macro rejection.
fn translate_syncer_result(
    name: &str,
    field: &str,
    url: &str,
    allow_insecure: bool,
) -> BuildResult {
    match validate_syncer_template(url, allow_insecure) {
        ValidationResult::Ok => Ok(()),
        ValidationResult::TemplateTooLong => {
            Err(format!("bidder {name:?} {field} exceeds the 8 KiB syncer-template limit").into())
        }
        ValidationResult::AuthorityMacro => Err(format!(
            "bidder {name:?} {field} contains a template macro in its authority. \
             Macros (e.g. {{{{.Host}}}}) are only allowed in the path or query, never in the \
             scheme or host.",
        )
        .into()),
        ValidationResult::SyncerUserinfoOrFragment => Err(format!(
            "bidder {name:?} {field} contains userinfo or fragment; syncer URLs must have neither."
        )
        .into()),
        ValidationResult::UnknownMacro(_) => Err(format!(
            "bidder {name:?} {field} contains an unknown template macro. \
             Bidder syncer URLs may only reference the allow-listed macros: \
             {{{{.RedirectURL}}}}, {{{{.GDPR}}}}, {{{{.GDPRConsent}}}}, {{{{.USPrivacy}}}}, \
             {{{{.GPP}}}}, {{{{.GPPSID}}}}. {{{{.ExternalURL}}}} / {{{{.SyncerKey}}}} / \
             {{{{.BidderName}}}} / {{{{.SyncType}}}} / {{{{.UserMacro}}}} belong in the \
             redirect_url template, not in a bidder syncer URL.",
        )
        .into()),
        ValidationResult::UnclosedMacro(_) => Err(format!(
            "bidder {name:?} {field} contains an unclosed template macro \
             (no matching `}}}}` before end-of-input). Close the macro or \
             remove the stray `{{{{`.",
        )
        .into()),
        // Everything else delegates to the endpoint-URL message.
        ValidationResult::MalformedUrl
        | ValidationResult::UnsupportedScheme
        | ValidationResult::HttpInDefaultBuild
        | ValidationResult::LoopbackInDefaultBuild
        | ValidationResult::NonLocalhostHttpUnderInsecure
        | ValidationResult::NonLiteralLoopbackUnderInsecure
        | ValidationResult::LocalhostAlias
        | ValidationResult::PrivateIpAuthority => {
            translate_endpoint_result(name, field, url, allow_insecure)
        }
    }
}

/// Codegen-time validation for the per-bidder `hostAllowlist` TOML
/// list. Each entry is a bare host literal (no scheme, no path, no
/// port) -- i.e. the `{{.Host}}` substitution the runtime will splice
/// into the endpoint template. We synthesise `https://<entry>/` (or
/// `http://<entry>/` under `insecure-bidders` for loopback literals)
/// and re-use `validate_endpoint_url` so the same private-IP /
/// loopback / scheme rules apply: a host-varying bidder cannot
/// declare `10.0.0.1` or `127.0.0.2` in its allow-list any more than
/// it can hard-code those in its endpoint.
///
/// Also enforces the closed-host-set guarantee -- a `{{.Host}}`
/// endpoint MUST declare a non-empty `hostAllowlist`, and conversely
/// declaring an allowlist on a fixed-host endpoint is a misconfig
/// (the runtime equality check would ignore the allowlist).
fn validate_host_allowlist(
    name: &str,
    endpoint: &str,
    allowlist: Option<&[String]>,
    allow_insecure: bool,
) -> BuildResult {
    // Authority-strict: a bidder is host-varying IFF the endpoint's
    // authority slot (between `://` and the first path separator) is
    // exactly `{{.Host}}`. A substring scan would misclassify
    // `https://fixed.example/rtb/{{.Host}}` as host-varying even
    // though the host is fixed; the `render_endpoint_macros` gate
    // already rejects that shape but the belt-and-braces check here
    // makes the invariant local to this validator.
    let host_varying = matches!(
        classify_endpoint_authority(endpoint),
        EndpointAuthorityKind::HostVarying,
    );
    match (host_varying, allowlist) {
        (true, None) => {
            return Err(format!(
                "bidder {name:?} endpoint contains `{{{{.Host}}}}` but \
                 declares no `hostAllowlist`. Host-varying endpoints MUST enumerate \
                 every host `{{{{.Host}}}}` may resolve to at build time."
            )
            .into());
        }
        (false, Some(_)) => {
            return Err(format!(
                "bidder {name:?} endpoint has no `{{{{.Host}}}}` macro but \
                 declares a `hostAllowlist`. The runtime check for a fixed-host endpoint \
                 is host equality with the endpoint authority; `hostAllowlist` is only \
                 meaningful for host-varying endpoints."
            )
            .into());
        }
        (true, Some(_)) | (false, None) => {}
    }
    let Some(entries) = allowlist else {
        return Ok(());
    };
    if entries.is_empty() {
        return Err(format!(
            "bidder {name:?} declares an empty hostAllowlist. Either remove the field \
             (host-fixed bidder) or list at least one host."
        )
        .into());
    }
    for entry in entries {
        validate_host_allowlist_entry(name, entry, allow_insecure)?;
    }
    Ok(())
}

/// Per-entry rules for `hostAllowlist`. Each entry must be a bare
/// host literal (no scheme, no path, no userinfo, no fragment, no
/// port, no template macros). The synthesized `https://{entry}/`
/// must round-trip through `url::Url` with the parsed host matching
/// the entry byte-for-byte after IPv6-bracket stripping.
fn validate_host_allowlist_entry(name: &str, entry: &str, allow_insecure: bool) -> BuildResult {
    if entry.is_empty() || entry.trim() != entry {
        return Err(format!(
            "bidder {name:?} hostAllowlist entry is empty or surrounded by \
             whitespace. Use a bare host literal (e.g. \"eu.example.test\")."
        )
        .into());
    }
    if entry.contains("://") || entry.contains('/') || entry.contains('?') {
        return Err(format!(
            "bidder {name:?} hostAllowlist entry looks like a URL. List bare \
             hosts (no scheme, no path, no port) -- the runtime substitutes them into \
             the endpoint template's {{{{.Host}}}} slot."
        )
        .into());
    }
    if entry.contains("{{") {
        return Err(format!(
            "bidder {name:?} hostAllowlist entry contains a template macro. \
             Entries must be concrete authorities, not macros."
        )
        .into());
    }
    // Reject characters that change the authority's shape on URL
    // synthesis. `@` introduces userinfo,
    // `#` introduces a fragment, `\` is path-traversal in some URL
    // parsers -- any of them lets the stored string and the parsed
    // host diverge, making later host-equality checks ambiguous.
    if let Some(bad) = entry.chars().find(|chr| matches!(*chr, '@' | '#' | '\\')) {
        return Err(format!(
            "bidder {name:?} hostAllowlist entry contains forbidden \
             character {bad:?}. Entries must be bare host literals -- userinfo \
             (`@`), fragments (`#`), and backslashes are not allowed."
        )
        .into());
    }
    let synthesized = format!("https://{entry}/");
    translate_endpoint_result(name, "hostAllowlist entry", &synthesized, allow_insecure)?;
    // After URL validation, confirm round-trip: the parsed URL must
    // contain no userinfo, fragment, query, or non-root path. Catches
    // sneaky authority-splitting chars the explicit blacklist would miss.
    let parsed = url::Url::parse(&synthesized).map_err(|err| {
        format!(
            "bidder {name:?} hostAllowlist entry did not round-trip through \
             url::Url after validation: {err}",
        )
    })?;
    if !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
        || parsed.query().is_some()
        || parsed.path() != "/"
    {
        return Err(format!(
            "bidder {name:?} hostAllowlist entry parses as something other \
             than a bare host. Entries must contain no \
             userinfo, fragment, query, or path."
        )
        .into());
    }
    // The final URL's host must be a member of `host_allowlist` --
    // the runtime comparison is host-only (no port). An entry like
    // "eu.example.test:8443" would either silently never match
    // `Url::host_str()` or force the comparison side to grow a port
    // slot. Pick one representation up front: HOST-ONLY. Reject
    // port-bearing entries; encode any non-default port in the
    // endpoint template's path/query slot.
    if parsed.port().is_some() {
        return Err(format!(
            "bidder {name:?} hostAllowlist entry includes a port. The \
             host-allowlist set is host-only -- the runtime compares \
             `Url::host_str()` against the stored entries. Encode any non-default \
             port in the endpoint template instead."
        )
        .into());
    }
    // Final check: require the parsed host to match the entry byte-
    // for-byte (after the IPv6-bracket normalization url::Url applies).
    // Catches edge cases the explicit char blacklist would miss --
    // percent-encoded host chars, IDN punycoding, mixed-case
    // confusables, etc.
    let parsed_host = parsed.host_str().unwrap_or("");
    let entry_unbracketed = entry
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or(entry);
    if !parsed_host.eq_ignore_ascii_case(entry_unbracketed) {
        return Err(format!(
            "bidder {name:?} hostAllowlist entry does not round-trip through \
             url::Url. Use a plain hostname, IPv4 \
             literal, or `[ipv6-literal]` form."
        )
        .into());
    }
    Ok(())
}

/// Reject invalid baked alias names, ambiguous folded namespaces, and
/// base bidders absent from this build's registry.
///
/// Product defaults carry no aliases. Explicit harness fixtures still exercise
/// namespace and registry checks before generation; operator policy is resolved
/// and validated by the runtime owner.
///
/// `bidder_names` is what `emit_bidders` loaded, which is deliberately
/// build-shape-dependent: definitions requiring `insecure-bidders` are
/// emitted only under that feature, so an alias onto such a bidder
/// fails a default build and passes a `--features
/// insecure-bidders` one. That asymmetry is the truth — the alias
/// genuinely cannot resolve in a build that has no such bidder — so the
/// message says as much rather than leaving an operator guessing.
fn validate_host_aliases_against_registry(
    path: &str,
    parsed: &HostDefaultRaw,
    bidder_names: &[String],
) -> BuildResult {
    let mut folded_aliases = BTreeSet::new();
    for (alias, base) in &parsed.aliases {
        if alias.is_empty() {
            return Err(format!("{path}: host alias name must be nonempty").into());
        }
        if alias.len() > 128 {
            return Err(format!("{path}: host alias name exceeds 128 UTF-8 bytes").into());
        }
        if !folded_aliases.insert(alias.to_ascii_lowercase()) {
            return Err(format!("{path}: alias namespace collides at {alias:?}").into());
        }
        if !bidder_names.iter().any(|name| name == base) {
            return Err(format!(
                "{path}: host alias {alias:?} names base bidder {base:?}, which this build's \
                 bidder registry does not carry. Known bidders: [{known}]. A bidder that ships \
                 with required_feature = insecure-bidders is absent unless that \
                 feature is enabled, so an alias onto one resolves only in a build that \
                 enables it.",
                known = bidder_names.join(", "),
            )
            .into());
        }
        if bidder_names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(alias) && name != alias)
        {
            return Err(
                format!("{path}: alias {alias:?} shadows a differently cased base bidder").into(),
            );
        }
    }
    Ok(())
}

/// Returns the `crate::types` import list it emitted so
/// `emit_account_default` can skip already-imported names (the
/// generated file is one module; duplicates would be E0252).
fn emit_host_default(
    out: &mut TokenStream,
    path: &str,
    parsed: &HostDefaultRaw,
    bidder_names: &[String],
) -> BuildResult<Vec<&'static str>> {
    // `#[derive(Validate)]` on `ResourceLimitsRaw` enforces the
    // positivity rule for every cap. Surface the field-level error
    // with a build-script-friendly message.
    parsed
        .resource_limits
        .validate()
        .map_err(|errs| -> Box<dyn Error> { format!("{path} failed validation: {errs}").into() })?;
    validate_host_aliases_against_registry(path, parsed, bidder_names)?;
    codegen::usersync::validate_catalog_size(bidder_names.len(), parsed.aliases.len())?;
    // Same surfacing for the typed `RenderError`s (tri-state /
    // enforce_algo / default_value / vendor-id / key-range checks):
    // fail the build with a message pointing at the bad TOML key.
    let rendered = render_host_config(parsed)
        .map_err(|err| -> Box<dyn Error> { format!("{path} failed codegen: {err}").into() })?;
    let host_expression = syn::parse_str::<syn::Expr>(&rendered)?;

    let type_imports = host_default_type_imports(parsed);
    let imports = type_imports
        .iter()
        .map(|name| syn::parse_str::<syn::Ident>(name))
        .collect::<Result<Vec<_>, _>>()?;
    out.extend(quote! {
        use crate::types::{#(#imports),*};
        use std::collections::BTreeMap;

        #[must_use]
        pub fn host_default() -> &'static HostConfig {
            static CELL: OnceLock<HostConfig> = OnceLock::new();
            CELL.get_or_init(|| #host_expression)
        }
    });
    Ok(type_imports)
}

fn emit_account_default(
    out: &mut TokenStream,
    path: &str,
    parsed: &AccountRaw,
    host_type_imports: &[&'static str],
) -> BuildResult {
    let rendered = render_account_config(parsed)
        .map_err(|err| -> Box<dyn Error> { format!("{path} failed codegen: {err}").into() })?;
    let account_expression = syn::parse_str::<syn::Expr>(&rendered)?;

    let imports = account_default_type_imports(parsed, host_type_imports)
        .into_iter()
        .map(syn::parse_str::<syn::Ident>)
        .collect::<Result<Vec<_>, _>>()?;
    out.extend(quote! {
        use crate::types::{#(#imports),*};

        #[must_use]
        pub fn account_default() -> &'static AccountConfig {
            static CELL: OnceLock<AccountConfig> = OnceLock::new();
            CELL.get_or_init(|| #account_expression)
        }
    });
    Ok(())
}

fn emit_eea_set(out: &mut TokenStream) -> BuildResult {
    let txt = fs::read_to_string("data/eea-countries.txt")?;
    let countries: Vec<&str> = txt.lines().filter(|line| !line.is_empty()).collect();
    out.extend(quote! {
        #[must_use]
        pub fn eea_set() -> &'static [&'static str] {
            &[#(#countries,)*]
        }
    });
    println!("cargo:rerun-if-changed=data/eea-countries.txt");
    Ok(())
}

/// Read the archive under the persistent `fs2` advisory lock on the
/// checked-in `data/gvl_archive.lock` sentinel — there is no unlocked
/// read window. Common case (`live` manifest present): the SHARED lock
/// is held across every read; the returned handle keeps it held until
/// `emit_gvl_archive` returns. Recovery case (`live` absent — a refresh
/// swap crashed between its two renames): `fs2` has no portable
/// shared→exclusive upgrade, so the shared lock is DROPPED, the lock is
/// re-acquired EXCLUSIVE, state is re-checked and the four-case
/// `.old → live` recovery runs (shared with the refresh tool's pre-swap
/// step), and the read then happens while the exclusive is still held.
fn acquire_archive_read_lock(
    lock_path: &str,
    manifest_path: &str,
    archive_dir: &str,
) -> BuildResult<fs::File> {
    let shared =
        fs::File::open(lock_path).map_err(|err| format!("open archive lock {lock_path}: {err}"))?;
    shared
        .lock_shared()
        .map_err(|err| format!("lock_shared {lock_path}: {err}"))?;
    if Path::new(manifest_path).exists() {
        return Ok(shared);
    }
    drop(shared);
    let exclusive =
        fs::File::open(lock_path).map_err(|err| format!("open archive lock {lock_path}: {err}"))?;
    exclusive
        .lock_exclusive()
        .map_err(|err| format!("lock_exclusive {lock_path}: {err}"))?;
    // Re-check under the EXCLUSIVE lock (another process may have
    // recovered or swapped in the gap) and heal only if still needed.
    auctioneer_gvl_schema::recover_gvl_archive(Path::new(archive_dir))
        .map_err(|err| format!("recover_gvl_archive {archive_dir}: {err}"))?;
    Ok(exclusive)
}

fn verify_sha256(bytes: &[u8], expected: &str, label: &str) -> BuildResult {
    let observed = hex::encode(Sha256::digest(bytes));
    if observed == expected {
        Ok(())
    } else {
        Err(format!("SHA mismatch for {label}: manifest={expected}, on-disk={observed}").into())
    }
}

/// Whole days from `then` to `now`, via checked unix-timestamp
/// subtraction (conservatively saturating on the unreachable overflow).
fn days_between(now: OffsetDateTime, then: OffsetDateTime) -> i64 {
    now.unix_timestamp()
        .checked_sub(then.unix_timestamp())
        .map_or(i64::MAX, |secs| Duration::seconds(secs).whole_days())
}

/// Validate one manifest entry against its on-disk blob (SHA pin,
/// UTF-8, per-spec schema, manifest/blob version + lastUpdated
/// cross-checks, deleted-vendor aggregate warning) and render its
/// `BakedGvlEntry` literal. Returns the literal plus the raw byte
/// count.
fn check_archive_entry(
    archive_dir: &str,
    entry: &ManifestEntry,
) -> BuildResult<(TokenStream, u64)> {
    let raw_path = format!("{archive_dir}/{}", entry.path);
    println!("cargo:rerun-if-changed={raw_path}");
    let raw = fs::read(&raw_path).map_err(|err| format!("read {raw_path}: {err}"))?;
    verify_sha256(&raw, &entry.sha256, &entry.path)?;
    // The runtime uses lazy parse — but build.rs still validates every
    // entry up front so a malformed bundled blob fails the build, not
    // the first request that touches it.
    let raw_str = str::from_utf8(&raw).map_err(|err| format!("{} not UTF-8: {err}", entry.path))?;
    let validated = validate_gvl_json(raw_str, Some(entry.spec))
        .map_err(|err| format!("{} failed schema: {err:?}", entry.path))?;
    if validated.vendor_list_version != entry.vendor_list_version {
        return Err(format!(
            "manifest vendor_list_version {} disagrees with parsed {} in {}",
            entry.vendor_list_version, validated.vendor_list_version, entry.path
        )
        .into());
    }
    if validated.tcf_policy_version != entry.tcf_policy_version {
        return Err(format!(
            "manifest tcf_policy_version {} disagrees with parsed {} in {}",
            entry.tcf_policy_version, validated.tcf_policy_version, entry.path
        )
        .into());
    }
    // The manifest metadata may not drift from the file contents: the
    // blob's own `lastUpdated` must match the manifest string exactly.
    let parsed: serde_json::Value = serde_json::from_slice(&raw)
        .map_err(|err| format!("{}: archive JSON parse: {err}", entry.path))?;
    let embedded_last_updated = parsed
        .get("lastUpdated")
        .and_then(|value| value.as_str())
        .ok_or_else(|| format!("{}: archive JSON missing lastUpdated", entry.path))?;
    if embedded_last_updated != entry.last_updated_rfc3339 {
        return Err(format!(
            "manifest last_updated {} disagrees with archive JSON lastUpdated {} in {}",
            entry.last_updated_rfc3339, embedded_last_updated, entry.path
        )
        .into());
    }
    // Deleted-vendor observability is a BUILD-TIME aggregate (one line
    // per entry). The deleted set is static here (the archive is
    // `include_bytes!`), so this is the non-amplifying place; the
    // runtime lazy-parse path stays silent, and deleted vendors are
    // excluded from authorization at enforcement time. The count covers
    // every vendor CARRYING deletedDate, past- or future-dated.
    let deleted_count = validated
        .vendors
        .values()
        .filter(|vendor| vendor.deleted_date.is_some())
        .count();
    if deleted_count > 0 {
        println!(
            "cargo:warning=GVL (spec {}, list {}): {deleted_count} vendors marked deleted",
            entry.spec, entry.vendor_list_version
        );
    }
    let spec = proc_macro2::Literal::u32_unsuffixed(entry.spec);
    let vendor_list_version = proc_macro2::Literal::u32_unsuffixed(entry.vendor_list_version);
    let tcf_policy_version = proc_macro2::Literal::u32_unsuffixed(entry.tcf_policy_version);
    let sha256 = &entry.sha256;
    let last_updated = &entry.last_updated_rfc3339;
    let literal = quote! {
        BakedGvlEntry {
            spec: #spec,
            vendor_list_version: #vendor_list_version,
            tcf_policy_version: #tcf_policy_version,
            raw: include_bytes!(#raw_path),
            sha256: #sha256,
            last_updated: #last_updated,
        }
    };
    let len = u64::try_from(raw.len()).map_err(|err| format!("{}: size: {err}", entry.path))?;
    Ok((literal, len))
}

/// Whether to skip the compressed-bundle estimate entirely.
///
/// `build_failure_harness` spawns one child `cargo build -p
/// auctioneer-config-data` per case. Every one of those cases asserts something
/// about CONFIG validation — bad bidder manifests, bad baked defaults,
/// a malformed archive manifest — and not one of them asserts anything
/// about how the corpus compresses. Running the estimate in each child
/// re-paid its cost once per case for no coverage.
///
/// # Why a real build cannot inherit this
///
/// The env var is read ONLY when the `test-harness-override` cargo
/// feature is on, the same gate the archive-dir and bidder manifest
/// redirects already sit behind. Production, release and CI builds do
/// not enable that feature, so for them this function does not consult
/// the environment at all and returns `false` — exporting
/// `AUCTIONEER__GVL__ARCHIVE__SKIP_BUNDLE_CHECK=1` in a shell, a CI job or
/// a Dockerfile cannot switch the gate off. Both conditions must hold,
/// and only the harness supplies either: it passes `--features
/// auctioneer-config-data/test-harness-override` on the child's command line and
/// sets the var through `Command::env` on that one child process.
fn skip_compressed_bundle_check() -> bool {
    if env::var("CARGO_FEATURE_TEST_HARNESS_OVERRIDE").is_err() {
        return false;
    }
    println!("cargo:rerun-if-env-changed=AUCTIONEER__GVL__ARCHIVE__SKIP_BUNDLE_CHECK");
    env::var("AUCTIONEER__GVL__ARCHIVE__SKIP_BUNDLE_CHECK").is_ok_and(|value| value == "1")
}

/// Brotli quality for the compressed-bundle ESTIMATE.
///
/// This pass answers one question — does the corpus still fit under the
/// platform upload ceiling — so it needs a trustworthy size estimate,
/// not the smallest achievable artifact. Quality 11 is tuned for
/// compress-once-serve-many and costs two orders of magnitude more time
/// for a few percent of size.
///
/// Measured on the real 19.62 MiB corpus (24 blobs: 23 vendor lists +
/// `gcl.json`), window 22, against the 9 MiB cap:
///
/// | quality | total       | % of cap | time    |
/// |---------|-------------|----------|---------|
/// | 11      | 1,759,631 B | 18.65 %  | 47.3 s  |
/// | 10      | 1,804,475 B | 19.12 %  |  7.3 s  |
/// | 9       | 1,986,407 B | 21.05 %  |  0.37 s |
///
/// Quality 9 is 128x faster than 11 and overstates the bundle by 12.9 %,
/// which the cap's 4.7x headroom absorbs: the corpus would have to grow
/// past 4.75x its present size before the q9 figure reached the cap, and
/// it crosses the cap EARLIER than the q11 figure would, so the estimate
/// errs toward failing the build rather than passing a bundle that will
/// not upload. Raise this back toward 11 if the measured total ever
/// climbs near the cap, where the few percent would start to matter.
const BUNDLE_ESTIMATE_BROTLI_QUALITY: u32 = 9;

/// Compressed-bundle gate: every archive entry PLUS gcl.json, summed
/// against the compressed cap. Catches deployments whose corpus no
/// longer compresses under the platform upload ceiling.
fn check_compressed_bundle(archive_dir: &str, manifest: &Manifest) -> BuildResult {
    if skip_compressed_bundle_check() {
        println!("cargo:warning=GVL compressed-bundle check skipped (test-harness-override)");
        return Ok(());
    }
    let bundle_max_compressed: u64 =
        env::var("AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_COMPRESSED_MIB")
            .ok()
            .and_then(|mib| mib.parse::<u64>().ok())
            .unwrap_or(9)
            .checked_mul(0x0010_0000)
            .ok_or("compressed cap overflows u64")?;
    let mut blob_paths: Vec<String> = manifest
        .entries
        .iter()
        .map(|entry| format!("{archive_dir}/{}", entry.path))
        .collect();
    blob_paths.push(format!("{archive_dir}/{}", manifest.gcl.path));
    let mut compressed_bytes: u64 = 0;
    for raw_path in &blob_paths {
        let raw = fs::read(raw_path).map_err(|err| format!("read {raw_path}: {err}"))?;
        let mut compressed = Vec::new();
        brotli::CompressorReader::new(raw.as_slice(), 4096, BUNDLE_ESTIMATE_BROTLI_QUALITY, 22)
            .read_to_end(&mut compressed)
            .map_err(|err| format!("brotli {raw_path}: {err}"))?;
        compressed_bytes = compressed_bytes
            .checked_add(u64::try_from(compressed.len())?)
            .ok_or("compressed byte total overflowed u64")?;
    }
    if compressed_bytes > bundle_max_compressed {
        return Err(format!(
            "GVL bundle compressed size exceeds \
             AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_COMPRESSED_MIB: \
             {compressed_bytes} > {bundle_max_compressed} bytes"
        )
        .into());
    }
    Ok(())
}

/// Freshness SLA on the manifest's own `generated_at` (default 45
/// days; `AUCTIONEER__GVL__ARCHIVE__MAX_AGE_DAYS` overrides). Kept separate
/// from `validate_manifest` because the cap is a build policy, not a
/// manifest-schema invariant. Returns the effective cap for codegen.
fn check_manifest_freshness(now: OffsetDateTime, generated_at: OffsetDateTime) -> BuildResult<u64> {
    let max_days: u64 = env::var("AUCTIONEER__GVL__ARCHIVE__MAX_AGE_DAYS")
        .ok()
        .and_then(|days| days.parse().ok())
        .unwrap_or(45);
    let age_days = days_between(now, generated_at);
    let max_days_i64 = i64::try_from(max_days).unwrap_or(i64::MAX);
    if age_days > max_days_i64 {
        return Err(format!(
            "GVL archive is {age_days} days old (cap: {max_days}). \
             Re-run: cargo run --locked -p auctioneer-cli --no-default-features -- gvl refresh"
        )
        .into());
    }
    Ok(max_days)
}

/// Validate the baked GVL archive against `manifest.json` and emit
/// `OUT_DIR/gvl_archive.rs`: `gvl_archive()`, the manifest metadata
/// accessors, and the baked GCL accessors. Every failure surfaces
/// through the `?`/`Result` channel per this build script's contract.
fn emit_gvl_archive(out_dir: &str) -> BuildResult {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let default_dir = format!("{manifest_dir}/data/gvl_archive");
    // The env override bypasses the checked-in archive, so production
    // builds ignore it: only the failure harness enables the
    // `test-harness-override` feature.
    let archive_dir = if env::var("CARGO_FEATURE_TEST_HARNESS_OVERRIDE").is_ok() {
        println!("cargo:rerun-if-env-changed=AUCTIONEER__GVL__ARCHIVE__DIR");
        env::var("AUCTIONEER__GVL__ARCHIVE__DIR").unwrap_or(default_dir)
    } else {
        default_dir
    };
    let manifest_path = format!("{archive_dir}/manifest.json");
    let lock_path = format!("{manifest_dir}/data/gvl_archive.lock");
    println!("cargo:rerun-if-changed={lock_path}");
    println!("cargo:rerun-if-changed={manifest_path}");
    println!("cargo:rerun-if-env-changed=AUCTIONEER__GVL__ARCHIVE__MAX_AGE_DAYS");
    println!("cargo:rerun-if-env-changed=AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_RAW_MIB");
    println!("cargo:rerun-if-env-changed=AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_COMPRESSED_MIB");

    // The read ALWAYS runs under the held lock (shared in the common
    // case, exclusive after recovery); `_lock_file` lives until this fn
    // returns, so `Drop` releases it only after the last read.
    let _lock_file = acquire_archive_read_lock(&lock_path, &manifest_path, &archive_dir)?;
    let raw_manifest =
        fs::read_to_string(&manifest_path).map_err(|err| format!("read {manifest_path}: {err}"))?;

    // ONE strict validator shared with the refresh tool. It already
    // enforced MissingGcl + GclStale (the GCL's own RFC 9111 lifetime,
    // never the manifest-age window below, and never overrideable).
    let now = OffsetDateTime::now_utc();
    let manifest = validate_manifest(&raw_manifest, now)
        .map_err(|err| format!("manifest validation: {err}"))?;

    // GCL: sha-pinned, schema-validated, manifest/blob cross-checked.
    let gcl_path = format!("{archive_dir}/{}", manifest.gcl.path);
    println!("cargo:rerun-if-changed={gcl_path}");
    let gcl_raw = fs::read_to_string(&gcl_path).map_err(|err| format!("read {gcl_path}: {err}"))?;
    verify_sha256(gcl_raw.as_bytes(), &manifest.gcl.sha256, "gcl.json")?;
    let validated_gcl = auctioneer_gvl_schema::validate_gcl_json(&gcl_raw)
        .map_err(|err| format!("gcl validation: {err}"))?;
    auctioneer_gvl_schema::check_gcl_consistency(&manifest, &validated_gcl)
        .map_err(|err| format!("gcl consistency: {err}"))?;

    let max_days = check_manifest_freshness(now, manifest.generated_at)?;

    // Raw bundle-size gate, per-target default: the wasm adapters share
    // a 128 MB isolate with code + parse cache + fanout streams, so
    // their GVL budget is 24 MB; native detects its ceiling at boot and
    // can afford 60 MB.
    let default_raw_mib: u64 = if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        24
    } else {
        60
    };
    let bundle_max_raw: u64 = env::var("AUCTIONEER__GVL__ARCHIVE__BUNDLE_MAX_RAW_MIB")
        .ok()
        .and_then(|mib| mib.parse().ok())
        .unwrap_or(default_raw_mib)
        .checked_mul(0x0010_0000)
        .ok_or("raw cap overflows u64")?;

    let max_age_days = proc_macro2::Literal::u32_unsuffixed(u32::try_from(max_days)?);
    let mut entries = Vec::with_capacity(manifest.entries.len());
    let mut bundle_bytes: u64 = 0;
    for entry in &manifest.entries {
        let (literal, len) = check_archive_entry(&archive_dir, entry)?;
        bundle_bytes = bundle_bytes
            .checked_add(len)
            .ok_or("bundle byte total overflowed u64")?;
        entries.push(literal);
    }

    // GCL emission — the blob participates in the bundle accounting and
    // the compressed pass alongside the entries.
    bundle_bytes = bundle_bytes
        .checked_add(u64::try_from(gcl_raw.len())?)
        .ok_or("bundle size overflow")?;
    let gcl_expires_unix = manifest
        .gcl
        .expires_at()
        .map_err(|err| format!("gcl expiry: {err}"))?
        .unix_timestamp();
    let gcl_expiry_expression =
        syn::parse_str::<syn::Expr>(&codegen::render_int_i64(gcl_expires_unix))?;
    let generated_at = &manifest.generated_at_rfc3339;
    let generated = quote! {
        #[must_use]
        pub fn gvl_archive() -> &'static [BakedGvlEntry] { ENTRIES }
        #[must_use]
        pub fn gvl_archive_generated_at() -> &'static str { GENERATED_AT }
        #[must_use]
        pub fn gvl_archive_max_age_days() -> u32 { MAX_AGE_DAYS }

        static GENERATED_AT: &str = #generated_at;
        static MAX_AGE_DAYS: u32 = #max_age_days;
        static ENTRIES: &[BakedGvlEntry] = &[#(#entries,)*];

        #[must_use]
        pub fn baked_gcl_json() -> &'static str { include_str!(#gcl_path) }

        #[doc = "Unix seconds; the runtime `Gcl` boot init performs the"]
        #[doc = "fallible `OffsetDateTime` conversion."]
        #[must_use]
        pub fn gcl_expires_at_unix_secs() -> i64 { #gcl_expiry_expression }
    };

    // The raw cap is ALWAYS enforced — `oversize_bundle_ack` relaxes
    // only the refresh tool's age filter, never this gate. The decision
    // itself is the pure, shared `validate_bundle_limits` (build.rs
    // cannot host unit tests, so the schema crate's suite exercises
    // every branch); this site only gathers inputs and surfaces the
    // error string.
    auctioneer_gvl_schema::validate_bundle_limits(
        env::var("CARGO_CFG_TARGET_ARCH").as_deref().unwrap_or(""),
        bundle_max_raw,
        bundle_bytes,
    )
    .map_err(|err| err.to_string())?;

    check_compressed_bundle(&archive_dir, &manifest)?;

    fs::write(format!("{out_dir}/gvl_archive.rs"), generated.to_string())?;
    Ok(())
}
