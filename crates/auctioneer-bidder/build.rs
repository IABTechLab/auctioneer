//! Discover implementations only in the crate's committed `src/bidders` root.

use auctioneer_config::bidder_discovery as build_discovery;
use auctioneer_config::schema_staging::BuildContext;
use auctioneer_config::schema_staging_build::{BuildRequest, generate, write_snapshots};
use auctioneer_config::{bidder_endpoint_codegen, bidder_settings_codegen};

use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Command;

fn record_layout_build(out_dir: &Path) -> Result<BTreeMap<&'static str, String>, Box<dyn Error>> {
    for key in [
        "RUSTC",
        "TARGET",
        "CARGO_CFG_TARGET_ARCH",
        "CARGO_CFG_TARGET_POINTER_WIDTH",
        "CARGO_CFG_TARGET_ENDIAN",
    ] {
        println!("cargo::rerun-if-env-changed={key}");
    }
    let compiler = Command::new(env::var("RUSTC")?).arg("-vV").output()?;
    if !compiler.status.success() {
        return Err("build compiler identity query failed".into());
    }
    let identity = BTreeMap::from([
        ("compiler", String::from_utf8(compiler.stdout)?),
        ("target", env::var("TARGET")?),
        ("arch", env::var("CARGO_CFG_TARGET_ARCH")?),
        ("pointer_width", env::var("CARGO_CFG_TARGET_POINTER_WIDTH")?),
        ("endian", env::var("CARGO_CFG_TARGET_ENDIAN")?),
    ]);
    // Data for native conformance only; deployed code never includes this file.
    fs::write(
        out_dir.join("schema-layout-build.toml"),
        toml::to_string(&identity)?,
    )?;
    Ok(identity)
}

fn stage_schemas(
    manifest_dir: &Path,
    out_dir: &Path,
    discovery: &build_discovery::Discovery,
    compiler: &str,
) -> Result<(), Box<dyn Error>> {
    let mut environment = BTreeMap::new();
    for key in [
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
    ] {
        println!("cargo::rerun-if-env-changed={key}");
    }
    for (raw_key, raw_value) in env::vars_os() {
        let Some(key) = raw_key.to_str() else {
            continue;
        };
        if key.starts_with("CARGO_CFG_")
            || key.starts_with("CARGO_FEATURE_")
            || key.starts_with("CARGO_PROFILE_")
            || matches!(
                key,
                "PROFILE"
                    | "OPT_LEVEL"
                    | "DEBUG"
                    | "HOST"
                    | "TARGET"
                    | "OUT_DIR"
                    | "RUSTC"
                    | "RUSTFLAGS"
                    | "CARGO_ENCODED_RUSTFLAGS"
                    | "RUSTC_WRAPPER"
                    | "RUSTC_WORKSPACE_WRAPPER"
            )
        {
            let value = raw_value
                .into_string()
                .map_err(|_invalid| format!("non UTF-8 staging build variable: {key}"))?;
            environment.insert(key.to_owned(), value);
        }
    }
    for required in ["PROFILE", "OPT_LEVEL", "DEBUG", "HOST", "TARGET"] {
        if !environment.contains_key(required) {
            return Err(format!("missing staging build variable: {required}").into());
        }
    }
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string(manifest_dir.join("Cargo.toml"))?)?;
    let enabled: Vec<_> = manifest
        .get("features")
        .and_then(toml::Value::as_table)
        .ok_or("missing bidder features")?
        .keys()
        .filter(|feature| {
            env::var_os(format!(
                "CARGO_FEATURE_{}",
                feature.replace('-', "_").to_ascii_uppercase()
            ))
            .is_some()
        })
        .cloned()
        .collect();
    let enabled_refs: Vec<_> = enabled.iter().map(String::as_str).collect();
    // PROFILE may collapse custom release profiles to `release`; Cargo's output
    // directory retains the actual profile name used for this artifact.
    let profile = out_dir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .ok_or("missing actual staging build profile")?;
    let target = env::var("TARGET")?;
    let flags = match env::var("CARGO_ENCODED_RUSTFLAGS") {
        Ok(value) => value,
        Err(env::VarError::NotPresent) => String::new(),
        Err(error) => return Err(error.into()),
    };
    let generated = generate(BuildRequest {
        crate_dir: manifest_dir,
        out_dir,
        discovery,
        metadata: auctioneer_config_data::bidder_metadata_declarations(),
        context: BuildContext {
            target: &target,
            profile,
            enabled_features: &enabled_refs,
            compiler_verbose: compiler,
            encoded_rustflags: &flags,
        },
        build_environment: &environment,
    })?;
    for path in generated.rerun_paths {
        println!("cargo::rerun-if-changed={}", path.display());
    }
    write_snapshots(out_dir, &generated.snapshots)?;
    fs::write(
        out_dir.join("schema-staging.rs"),
        generated.rust.to_string(),
    )?;
    fs::write(out_dir.join("schema-staging.json"), generated.manifest_json)?;
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let out_dir = env::var("OUT_DIR")?;
    let layout = record_layout_build(Path::new(&out_dir))?;
    let discovery = build_discovery::scan(Path::new(&manifest_dir))?;
    discovery.validate_metadata(auctioneer_config_data::bidder_metadata_declarations())?;
    let rendered = discovery.render()?;
    let registry =
        discovery.render_registry(auctioneer_config_data::bidder_metadata_declarations())?;
    let uid_policies = discovery.render_uid_policies()?;
    let allocation_conformance = discovery.render_allocation_conformance()?;
    let response_conformance = discovery.render_response_conformance()?;
    let settings = bidder_settings_codegen::render(discovery.manifests())?;
    let endpoints = bidder_endpoint_codegen::render(discovery.manifests())?;

    for path in discovery.rerun_paths() {
        println!("cargo::rerun-if-changed={}", path.display());
    }
    fs::write(Path::new(&out_dir).join("discovery.rs"), rendered)?;
    fs::write(Path::new(&out_dir).join("registry.rs"), registry)?;
    fs::write(Path::new(&out_dir).join("bidder_settings.rs"), settings)?;
    fs::write(Path::new(&out_dir).join("bidder_endpoint.rs"), endpoints)?;
    fs::write(
        Path::new(&out_dir).join("sync_uid_policies.rs"),
        uid_policies,
    )?;
    fs::write(
        Path::new(&out_dir).join("bidder_allocation_conformance.rs"),
        allocation_conformance,
    )?;
    fs::write(
        Path::new(&out_dir).join("bidder_response_conformance.rs"),
        response_conformance,
    )?;
    if env::var_os("CARGO_FEATURE_SCHEMA_ARTIFACT_STAGING").is_some()
        || env::var("CARGO_CFG_TARGET_ARCH")? != "wasm32"
    {
        stage_schemas(
            Path::new(&manifest_dir),
            Path::new(&out_dir),
            &discovery,
            layout
                .get("compiler")
                .ok_or("missing staging compiler identity")?,
        )?;
    } else {
        for relative in ["schema-staging.rs", "schema-staging.json"] {
            match fs::remove_file(Path::new(&out_dir).join(relative)) {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
    }
    Ok(())
}
