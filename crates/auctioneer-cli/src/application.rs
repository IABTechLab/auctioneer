//! Application commands are excluded from archive-independent maintenance builds.

use crate::qualification;
use auctioneer_core::config::SdkAuctioneerAppConfig;
use clap::Subcommand;
use edgezero_cli::DiffExit;
use edgezero_cli::args::{
    ActiveVersionArgs, AuthArgs, BuildArgs, ConfigDiffArgs, ConfigGcArgs, ConfigPushArgs,
    ConfigValidateArgs, DeployArgs, HealthcheckArgs, NewArgs, ProvisionArgs, RollbackArgs,
    ServeArgs,
};

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
    /// Resolve and print the currently-active service version.
    ActiveVersion(ActiveVersionArgs),
    /// Sign in, out, or query status using the hosting platform's native CLI.
    Auth(AuthArgs),
    /// Build the project for a target edge.
    Build(BuildArgs),
    /// Inspect or mutate the typed app config.
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Deploy to a target edge, including a staging draft.
    Deploy(DeployArgs),
    /// Probe a deployed or staged version's health.
    Healthcheck(HealthcheckArgs),
    /// Create a new `EdgeZero` app skeleton.
    New(NewArgs),
    /// Create platform resources backing the declared stores.
    Provision(ProvisionArgs),
    /// Validate and evaluate deployment resource qualification offline.
    #[command(subcommand)]
    Qualify(qualification::Command),
    /// Roll a service back or deactivate a staging version.
    Rollback(RollbackArgs),
    /// Run a hosting-platform-specific local simulation.
    Serve(ServeArgs),
}

#[derive(Subcommand, Debug)]
pub(crate) enum ConfigCommand {
    /// Diff typed app config against the live store.
    Diff(ConfigDiffArgs),
    /// Reclaim config-store chunks that no live pointer references.
    Gc(ConfigGcArgs),
    /// Push typed app config as a blob envelope.
    Push(ConfigPushArgs),
    /// Validate the manifest and typed app config using the push contract.
    Validate(ConfigValidateArgs),
}

pub(crate) fn run(command: Command) -> i32 {
    let result: Result<(), String> = match command {
        Command::ActiveVersion(args) => edgezero_cli::run_active_version(&args),
        Command::Auth(args) => edgezero_cli::run_auth(&args),
        Command::Build(args) => edgezero_cli::run_build(&args),
        Command::Config(ConfigCommand::Push(args)) => {
            edgezero_cli::run_config_push_typed::<SdkAuctioneerAppConfig>(&args)
        }
        Command::Config(ConfigCommand::Diff(args)) => {
            match edgezero_cli::run_config_diff_typed::<SdkAuctioneerAppConfig>(&args) {
                Ok(DiffExit { code: 0 }) => Ok(()),
                Ok(DiffExit { code }) => return code,
                Err(error) => Err(error),
            }
        }
        Command::Config(ConfigCommand::Gc(args)) => edgezero_cli::run_config_gc(&args),
        Command::Config(ConfigCommand::Validate(args)) => {
            edgezero_cli::run_config_validate_typed::<SdkAuctioneerAppConfig>(&args)
        }
        Command::Deploy(args) => edgezero_cli::run_deploy(&args),
        Command::Healthcheck(args) => edgezero_cli::run_healthcheck(&args),
        Command::New(args) => edgezero_cli::run_new(&args),
        Command::Provision(args) => edgezero_cli::run_provision(&args),
        Command::Qualify(qualification_command) => {
            return qualification::run(qualification_command);
        }
        Command::Rollback(args) => edgezero_cli::run_rollback(&args),
        Command::Serve(args) => edgezero_cli::run_serve(&args),
    };
    if let Err(error) = result {
        log::error!("[auctioneer] {error}");
        2
    } else {
        0
    }
}

#[cfg(test)]
mod config_contract {
    use super::ConfigValidateArgs;
    use auctioneer_config::build_catalog;
    use auctioneer_core::App;
    use auctioneer_core::config::resolve::{
        AccountSelection, resolve_host, resolve_selected_account, resolved_account,
    };
    use auctioneer_core::config::{
        AuctioneerAppConfig, EffectiveConfigError, SdkAuctioneerAppConfig,
        validate_effective_defaults,
    };
    use clap::Parser as _;
    use edgezero_core::app::Hooks;
    use edgezero_core::app_config::{
        AppConfigLoadOptions, AppConfigMeta as _, load_app_config_with_options,
    };
    use edgezero_core::blob_envelope::BlobEnvelope;
    use serde::Deserialize as _;
    use serde_json::json;
    use std::collections::BTreeSet;
    use std::iter;
    use std::path::PathBuf;
    use std::process::Command;
    use std::{env, fs};
    use validator::Validate as _;

    const REFERENCE: &str = include_str!("../../../auctioneer.toml");
    const TMAX_ENV_VAR: &str = "AUCTIONEER__HOST__RESOURCE_LIMITS__DEFAULT_TMAX_MS";
    const PROBE_OUT_VAR: &str = "AUCTIONEER__TEST__PROBE_OUT";
    const LIVE_KEY_DOC: &str =
        "schema_version = 1\n[host.resource_limits]\ndefault_tmax_ms = 1000\n";
    const ABSENT_KEY_DOC: &str =
        "schema_version = 1\n[host.resource_limits]\n# default_tmax_ms = 1000\n";

    fn commented_example(name: &str) -> String {
        let start = format!("# BEGIN {name}");
        let end = format!("# END {name}");
        let lines: Vec<_> = REFERENCE.lines().collect();
        assert_eq!(lines.iter().filter(|line| **line == start).count(), 1);
        assert_eq!(lines.iter().filter(|line| **line == end).count(), 1);
        let first = lines.iter().position(|line| *line == start).unwrap();
        let last = lines.iter().position(|line| *line == end).unwrap();
        assert!(first < last);
        lines[first.checked_add(1).unwrap()..last]
            .iter()
            .map(|line| {
                if *line == "#" {
                    ""
                } else {
                    line.strip_prefix("# ").expect("example stays commented")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn assert_reference_fields(document: &toml::Value, path: &str, fields: &[&str]) {
        let table = path
            .split('.')
            .try_fold(document, |value, key| value.get(key))
            .and_then(toml::Value::as_table)
            .unwrap_or_else(|| panic!("missing reference table {path}"));
        assert_eq!(
            table.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            fields.iter().copied().collect::<BTreeSet<_>>(),
            "reference fields for {path}",
        );
    }

    #[test]
    fn shipped_reference_commands_use_the_actual_cli_arguments() {
        let commands: Vec<_> = REFERENCE
            .lines()
            .filter_map(|line| line.strip_prefix("# auctioneer-cli "))
            .collect();
        assert_eq!(commands.len(), 3);
        for command in commands {
            crate::Args::try_parse_from(
                iter::once("auctioneer-cli").chain(command.split_ascii_whitespace()),
            )
            .unwrap_or_else(|error| panic!("invalid reference command {command}: {error}"));
        }
    }

    const REFERENCE_FIELDS: &[(&str, &[&str])] = &[
        (
            "host",
            &[
                "status_response",
                "resource_limits",
                "gdpr",
                "ccpa",
                "lmt",
                "host_cookie",
                "user_sync",
                "aliases",
                "gvl",
            ],
        ),
        (
            "host.resource_limits",
            &[
                "default_tmax_ms",
                "max_tmax_ms",
                "max_bidder_response_bytes",
                "max_bidders_per_auction",
                "max_outbound_requests",
                "max_outbound_raw_bytes",
                "max_outbound_request_bytes",
                "max_transport_bytes",
                "max_decoded_bytes",
                "max_parsed_nodes",
            ],
        ),
        (
            "host.gdpr",
            &[
                "enabled",
                "default_value",
                "host_vendor_id",
                "non_standard_publishers",
                "tcf2",
            ],
        ),
        (
            "host.gdpr.tcf2",
            &[
                "enabled",
                "purpose_one_treatment",
                "purposes",
                "special_features",
            ],
        ),
        (
            "host.gdpr.tcf2.purpose_one_treatment",
            &["enabled", "access_allowed"],
        ),
        (
            "host.gdpr.tcf2.purposes.2",
            &[
                "enforce_algo",
                "enforce_purpose",
                "enforce_vendors",
                "vendor_exceptions",
            ],
        ),
        (
            "host.gdpr.tcf2.special_features.1",
            &["enforce", "vendor_exceptions"],
        ),
        ("host.ccpa", &["enforce"]),
        ("host.lmt", &["enforce"]),
        (
            "host.host_cookie",
            &[
                "cookie_name",
                "family",
                "domain",
                "ttl_days",
                "max_cookie_size_bytes",
                "optout_cookie",
            ],
        ),
        ("host.host_cookie.optout_cookie", &["name", "value"]),
        (
            "host.user_sync",
            &[
                "external_url",
                "redirect_url",
                "cooperative",
                "priority_groups",
                "allowed_origins",
            ],
        ),
        ("host.user_sync.cooperative", &["enabled_by_default"]),
        (
            "account_defaults",
            &[
                "max_transport_bytes",
                "max_decoded_bytes",
                "disabled",
                "debug_allow",
                "bid_rounding",
                "targeting_prefix",
                "truncate_target_attribute",
                "gdpr",
                "ccpa",
                "privacy",
                "cookie_sync",
            ],
        ),
        (
            "account_defaults.cookie_sync",
            &[
                "default_limit",
                "max_limit",
                "default_coop_sync",
                "priority_groups",
            ],
        ),
        (
            "account_defaults.gdpr",
            &[
                "enabled",
                "purpose_one_treatment",
                "channel_enabled",
                "purposes",
                "special_features",
                "basic_enforcement_vendors",
            ],
        ),
        (
            "account_defaults.gdpr.purpose_one_treatment",
            &["enabled", "access_allowed"],
        ),
        (
            "account_defaults.gdpr.channel_enabled",
            &["web", "app", "amp", "video", "dooh"],
        ),
        (
            "account_defaults.gdpr.purposes.2",
            &[
                "enforce_algo",
                "enforce_purpose",
                "enforce_vendors",
                "vendor_exceptions",
            ],
        ),
        (
            "account_defaults.gdpr.special_features.1",
            &["enforce", "vendor_exceptions"],
        ),
        ("account_defaults.ccpa", &["enabled", "channel_enabled"]),
        (
            "account_defaults.ccpa.channel_enabled",
            &["web", "app", "amp", "video", "dooh"],
        ),
        (
            "account_defaults.privacy",
            &["ipv4", "ipv6", "allowactivities"],
        ),
        ("account_defaults.privacy.ipv4", &["anon_keep_bits"]),
        ("account_defaults.privacy.ipv6", &["anon_keep_bits"]),
        (
            "account_defaults.privacy.allowactivities",
            &[
                "fetchBids",
                "transmitUfpd",
                "transmitPreciseGeo",
                "transmitTid",
                "syncUser",
                "reportAnalytics",
                "enrichUfpd",
                "transmitUniqueRequestIds",
            ],
        ),
        ("features", &["gzip_outbound", "returnallbidstatus_default"]),
        (
            "host.gvl",
            &["authenticate_health", "authenticate_health_token_secret"],
        ),
    ];

    #[test]
    fn reference_includes_the_complete_common_policy_surface() {
        let document: toml::Value = toml::from_str(REFERENCE).unwrap();
        for &(path, fields) in REFERENCE_FIELDS {
            assert_reference_fields(&document, path, fields);
        }
        for activity in document["account_defaults"]["privacy"]["allowactivities"]
            .as_table()
            .unwrap()
            .keys()
        {
            assert_reference_fields(
                &document,
                &format!("account_defaults.privacy.allowactivities.{activity}"),
                &["default", "rules"],
            );
        }
        let floor_example = commented_example("FLOOR CURRENCY EXAMPLE");
        let floor: toml::Value = toml::from_str(&floor_example).unwrap();
        assert_reference_fields(&floor, "floor_currency", &["expires_at", "usd_per_unit"]);
        let policy: SdkAuctioneerAppConfig =
            toml::from_str(&format!("{REFERENCE}\n{floor_example}")).unwrap();
        policy.validate().unwrap();
    }

    #[test]
    fn reference_clear_and_activity_alternatives_are_valid_policy() {
        for (example, paths) in [
            (
                "CLEAR EXAMPLES",
                &[
                    "/host/user_sync",
                    "/host/host_cookie/domain",
                    "/host/host_cookie/optout_cookie",
                    "/account_defaults/truncate_target_attribute",
                    "/account_defaults/cookie_sync/default_limit",
                    "/account_defaults/cookie_sync/max_limit",
                    "/account_defaults/cookie_sync/default_coop_sync",
                    "/account_defaults/cookie_sync/priority_groups",
                    "/account_defaults/gdpr/purpose_one_treatment/enabled",
                    "/account_defaults/gdpr/purpose_one_treatment/access_allowed",
                ][..],
            ),
            (
                "SYNC REDIRECT CLEAR EXAMPLE",
                &["/host/user_sync/redirect_url"],
            ),
        ] {
            let policy: SdkAuctioneerAppConfig =
                toml::from_str(&commented_example(example)).unwrap();
            policy.validate().unwrap();
            let value = serde_json::to_value(policy).unwrap();
            for path in paths {
                assert_eq!(value.pointer(path), Some(&json!({"clear": true})), "{path}");
            }
        }
        let policy: SdkAuctioneerAppConfig = toml::from_str(&format!(
            "{REFERENCE}\n{}",
            commented_example("ACTIVITY RULE EXAMPLE")
        ))
        .unwrap();
        policy.validate().unwrap();
        let value = serde_json::to_value(policy).unwrap();
        assert_eq!(
            value["accounts"]["fake_publisher"]["privacy"]["allowactivities"]["transmitUfpd"]["rules"]
                [0],
            json!({"allow": false, "condition": {"componentName": ["fake_alias"], "componentType": ["bidder"]}})
        );
    }

    #[test]
    fn reference_bidder_settings_cover_discovered_fields_without_a_bidder_list() {
        let example: toml::Value = toml::from_str(&commented_example("BIDDER EXAMPLE")).unwrap();
        assert_reference_fields(
            &example,
            "bidders.fake_bidder",
            &[
                "enabled",
                "usersync_enabled",
                "timeout_ms",
                "sample_rate",
                "max_transport_bytes",
                "max_decoded_bytes",
                "implementation",
                "settings",
            ],
        );
        let values = example["bidders"]["fake_bidder"]["settings"]
            .as_table()
            .unwrap();
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("auctioneer-bidder");
        let manifest: toml::Value =
            toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
        let features = manifest["features"]
            .as_table()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let catalog = build_catalog::scan(&root.join("src/bidders"), &features).unwrap();
        let owners: Vec<_> = catalog
            .manifests
            .values()
            .filter_map(|owner| owner.settings.as_ref().map(|settings| (owner, settings)))
            .collect();
        assert!(!owners.is_empty());
        let declared: BTreeSet<_> = owners
            .iter()
            .flat_map(|(_, settings)| settings.fields.keys().map(String::as_str))
            .collect();
        assert_eq!(
            values.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            declared
        );
        let expected_roundtrips = owners.len();
        let mut roundtrips = 0_usize;
        let mut checked = 0_usize;
        for (owner, settings) in owners {
            let compiled_name = owner
                .definitions
                .keys()
                .find(|name| auctioneer_config_data::bidder(name).is_some());
            let name = compiled_name.unwrap_or(&owner.key);
            let selected: toml::Table = settings
                .fields
                .keys()
                .map(|field| (field.clone(), values[field].clone()))
                .collect();
            let mut overrides = serde_json::to_value(&example["bidders"]["fake_bidder"]).unwrap();
            overrides["implementation"] = json!(owner.key);
            overrides["settings"] = json!(selected);
            let wire = json!({"schema_version": 1_u8, "bidders": {(name): overrides}});
            let policy: SdkAuctioneerAppConfig = serde_json::from_value(wire.clone()).unwrap();
            let toml_policy: SdkAuctioneerAppConfig =
                toml::from_str(&toml::to_string(&wire).unwrap()).unwrap();
            // Reference schemas cover every discovered owner. Admission
            // additionally requires the owner in this feature lane's build.
            if compiled_name.is_some() {
                policy.validate().unwrap();
                toml_policy.validate().unwrap();
                checked = checked.checked_add(1).unwrap();
            }
            let serialized = serde_json::to_value(&policy).unwrap();
            assert_eq!(serialized, serde_json::to_value(toml_policy).unwrap());
            assert_eq!(serialized["bidders"][name]["implementation"], owner.key);
            assert_eq!(serialized["bidders"][name]["settings"], json!(selected));
            roundtrips = roundtrips.checked_add(1).unwrap();
        }
        assert_eq!(roundtrips, expected_roundtrips);
        assert!(checked > 0, "compiled settings examples must execute");
    }

    fn fixture_file(body: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temporary fixture directory");
        let path = dir.path().join("auctioneer.toml");
        fs::write(&path, body).expect("write fixture");
        (dir, path)
    }

    fn envelope_of<C: serde::Serialize>(typed: &C) -> BlobEnvelope {
        BlobEnvelope::new(
            serde_json::to_value(typed).expect("typed config serializes"),
            "2026-01-01T00:00:00Z".to_owned(),
        )
    }

    #[test]
    fn config_fixtures_are_independent_of_cargo_output_paths() {
        let (_dir, path) = fixture_file(LIVE_KEY_DOC);
        assert!(path.starts_with(env::temp_dir()));
        assert_eq!(fs::read_to_string(path).unwrap(), LIVE_KEY_DOC);
    }

    #[test]
    fn typed_validate_command_accepts_direct_policy_and_refuses_invalid_inputs_without_writes() {
        let cases = [
            (
                "schema_version = 1\n[host.resource_limits]\ndefault_tmax_ms = 1000\n",
                0_i32,
            ),
            (
                "schema_version = 1\n[host.resource_limits]\ndefault_tmax_ms = 0\n",
                2_i32,
            ),
            (
                "schema_version = 1\n[host.aliases]\nfake_private_alias = 'fake_unknown_base'\n",
                2_i32,
            ),
            ("[runtime]\n", 2_i32),
        ];
        assert_eq!(cases.len(), 4);
        for (body, expected) in cases {
            let (directory, config) = fixture_file(body);
            let mut manifest: toml::Value =
                toml::from_str(include_str!("../../../edgezero.toml")).unwrap();
            manifest["adapters"]
                .as_table_mut()
                .unwrap()
                .retain(|name, _| name == "axum");
            manifest["adapters"]["axum"]["adapter"]
                .as_table_mut()
                .unwrap()
                .remove("manifest");
            let manifest_path = directory.path().join("edgezero.toml");
            fs::write(&manifest_path, toml::to_string(&manifest).unwrap()).unwrap();
            let mut args = ConfigValidateArgs::default();
            args.app_config = Some(config);
            args.manifest = manifest_path;
            args.no_env = true;
            let result = super::run(super::Command::Config(super::ConfigCommand::Validate(args)));
            assert_eq!(result, expected, "{body}");
            let mut names = fs::read_dir(directory.path())
                .unwrap()
                .map(|entry| entry.unwrap().file_name())
                .collect::<Vec<_>>();
            names.sort();
            assert_eq!(names, ["auctioneer.toml", "edgezero.toml"]);
        }
    }

    #[test]
    fn pushed_blob_is_the_authored_runtime_model_without_projection() {
        let source: SdkAuctioneerAppConfig = toml::from_str(REFERENCE).unwrap();
        source.validate().expect("shipped runtime policy is valid");
        let data = serde_json::to_value(&source).unwrap();
        let object = data.as_object().unwrap();
        for retired in [
            "baked",
            "runtime",
            "startup",
            "auction",
            "targeting",
            "status",
        ] {
            assert!(!object.contains_key(retired), "unexpected root: {retired}");
        }
        for expected in [
            "schema_version",
            "host",
            "account_defaults",
            "accounts",
            "bidders",
            "features",
        ] {
            assert!(object.contains_key(expected), "missing root: {expected}");
        }
        assert_eq!(data["schema_version"], 1_u32);
        assert_eq!(data["host"]["resource_limits"]["default_tmax_ms"], 1000_u32);
        assert_eq!(
            data["host"]["user_sync"]["external_url"],
            "https://sync.example.test"
        );
        assert_eq!(data["account_defaults"]["debug_allow"], false);
        assert_eq!(data["accounts"]["fake_publisher"]["debug_allow"], true);
    }

    #[test]
    fn pushed_envelope_data_and_digest_match_the_direct_engine_model() {
        let sdk: SdkAuctioneerAppConfig = toml::from_str(REFERENCE).unwrap();
        let direct: AuctioneerAppConfig = toml::from_str(REFERENCE).unwrap();
        let pushed = envelope_of(&sdk);
        let engine = envelope_of(&direct);
        assert_eq!(pushed.data, engine.data);
        assert_eq!(pushed.sha256, engine.sha256);
        pushed.verify().unwrap();
        let restored: SdkAuctioneerAppConfig = serde_json::from_value(pushed.data.clone()).unwrap();
        assert_eq!(serde_json::to_value(restored).unwrap(), pushed.data);
    }

    #[test]
    fn direct_model_requires_a_supported_schema_and_rejects_retired_wrappers() {
        for invalid in [
            "",
            "[host]\n",
            "schema_version = 0\n",
            "schema_version = 2\n",
            "schema_version = '1'\n",
            "[runtime]\n",
            "[baked.host]\n[baked.account]\n",
            "schema_version = 1\n[runtime]\n",
            "schema_version = 1\n[baked.host]\n",
            "schema_version = 1\n[startup]\n",
        ] {
            toml::from_str::<SdkAuctioneerAppConfig>(invalid)
                .expect_err("unsupported policy must fail before push");
        }
        toml::from_str::<SdkAuctioneerAppConfig>("schema_version = 1\n").unwrap();
    }

    #[test]
    fn a_typo_in_a_root_namespace_is_rejected_at_push() {
        let typo = REFERENCE.replace("\n[host]\n", "\n[hots]\n\n[host]\n");
        assert_ne!(typo, REFERENCE);
        let error = toml::from_str::<SdkAuctioneerAppConfig>(&typo).unwrap_err();
        assert!(error.to_string().contains("unknown field"), "{error}");
    }

    #[test]
    fn push_validates_effective_sync_config_without_a_cli_validator() {
        let valid = "schema_version = 1\n[host.host_cookie]\nmax_cookie_size_bytes = 0\n[host.user_sync]\nexternal_url = 'https://sync.example.test'\npriority_groups = []\n";
        let source: SdkAuctioneerAppConfig = toml::from_str(valid).unwrap();
        source.validate().expect("valid effective sync policy");
        let data = serde_json::to_value(source).unwrap();
        assert_eq!(data["host"]["host_cookie"]["max_cookie_size_bytes"], 0_u32);
        assert_eq!(data["host"]["user_sync"]["priority_groups"], json!([]));
        assert!(data.get("runtime").is_none());
        assert!(data.get("baked").is_none());

        let invalid: SdkAuctioneerAppConfig =
            toml::from_str("schema_version = 1\n[host.host_cookie]\ncookie_name = 'uids'\n")
                .unwrap();
        assert!(
            invalid.validate().is_err(),
            "reject merged cookie invariant"
        );
        assert!(
            matches!(
                validate_effective_defaults(&invalid),
                Err(EffectiveConfigError::Malformed(_))
            ),
            "invalid operator data is not an immutable registry failure"
        );
    }

    #[test]
    fn valid_earlier_account_layers_survive_a_partial_selected_account() {
        let source: SdkAuctioneerAppConfig = serde_json::from_value(json!({
            "schema_version": 1_u8,
            "host": {"user_sync": {"external_url": "https://sync.example.test"}},
            "account_defaults": {"cookie_sync": {"default_limit": 5_i64, "max_limit": 10_i64}},
            "accounts": {"fake_publisher": {"debug_allow": true}},
        }))
        .unwrap();
        source
            .validate()
            .expect("validate complete inherited account layers");
        let host = resolve_host(&source, auctioneer_config_data::product_host_default());
        assert_eq!(
            host.user_sync.as_ref().unwrap().external_url,
            "https://sync.example.test"
        );
        let account = resolved_account(&source, auctioneer_config_data::product_account_default());
        assert_eq!(account.cookie_sync.default_limit, Some(5));
        assert_eq!(account.cookie_sync.max_limit, Some(10));
        let named = resolve_selected_account(
            AccountSelection::Named("fake_publisher".to_owned()),
            &source,
            auctioneer_config_data::product_account_default(),
        );
        assert!(named.config.debug_allow);
        assert_eq!(named.config.cookie_sync.default_limit, Some(5));
        assert_eq!(named.config.cookie_sync.max_limit, Some(10));
    }

    #[test]
    fn a_later_account_cannot_hide_invalid_earlier_defaults() {
        let source: SdkAuctioneerAppConfig = serde_json::from_value(json!({
            "schema_version": 1_u8,
            "account_defaults": {"cookie_sync": {"priority_groups": [[""]]}},
            "accounts": {"fake_publisher": {"cookie_sync": {"priority_groups": []}}},
        }))
        .unwrap();
        assert!(
            source.validate().is_err(),
            "default layer must independently validate"
        );
    }

    #[test]
    fn sdk_secret_metadata_stays_root_relative_without_a_source_wrapper() {
        assert!(SdkAuctioneerAppConfig::secret_fields().is_empty());
    }

    #[test]
    fn sdk_in_place_deserialization_matches_a_fresh_direct_model() {
        let mut sdk: SdkAuctioneerAppConfig = toml::from_str(REFERENCE).unwrap();
        assert!(sdk.host.resource_limits.default_tmax_ms.is_some());
        let document =
            json!({"schema_version": 1_u8, "account_defaults": {"targeting_prefix": "ad"}});
        let wire = serde_json::to_string(&document).unwrap();
        let mut deserializer = serde_json::Deserializer::from_str(&wire);
        SdkAuctioneerAppConfig::deserialize_in_place(&mut deserializer, &mut sdk).unwrap();
        let direct: AuctioneerAppConfig = serde_json::from_value(document).unwrap();
        assert_eq!(
            serde_json::to_value(sdk).unwrap(),
            serde_json::to_value(direct).unwrap()
        );
    }

    #[test]
    fn fastly_local_config_starts_empty_until_push() {
        let manifest: toml::Value = toml::from_str(include_str!("../../../fastly.toml")).unwrap();
        let config_id = <App as Hooks>::stores()
            .config
            .expect("the app declares a config store")
            .default;
        let wire = manifest["local_server"]["config_stores"][config_id]["contents"][config_id]
            .as_str()
            .unwrap();
        assert_eq!(wire, "");
    }

    #[test]
    fn runtime_envelope_fixture_uses_the_same_schema_and_actual_integrity_helper() {
        // Retain the independently hashed neutral fixture without shipping a
        // policy in the local manifest. The recorded digest is not recomputed.
        let mut envelope: BlobEnvelope = serde_json::from_value(json!({
            "data": {
                "account_defaults": {}, "accounts": {}, "bidders": {},
                "features": {}, "host": {"resource_limits": {}}, "schema_version": 1_u64,
            },
            "generated_at": "2026-10-07T04:35:15Z",
            "sha256": "6aa4c01132d4dd3425c13e28cab5c2fb1fec177d6303f1b3e3a863b08adc7778",
            "version": 1_u32,
        }))
        .unwrap();
        envelope.verify().unwrap();
        assert_eq!(envelope.data["schema_version"], 1_u32);
        assert!(envelope.data.get("runtime").is_none());
        assert!(envelope.data.get("baked").is_none());
        let source: SdkAuctioneerAppConfig = serde_json::from_value(envelope.data.clone()).unwrap();
        source.validate().unwrap();
        assert_eq!(serde_json::to_value(source).unwrap(), envelope.data);
        assert!(envelope.data["host"].get("status_response").is_none());
        envelope.data["host"]["status_response"] = json!("fake_changed_banner");
        assert!(
            envelope.verify().is_err(),
            "a changed payload must fail integrity"
        );
    }

    // The SDK reads the process environment; children preserve libtest isolation
    // without unsafe global env mutations. Ignored probes run only via parents.
    fn env_overlay_probe(inner_test: &str) -> String {
        let (_dir, fixture) = fixture_file("");
        let answer = fixture.with_file_name("probe_answer.txt");
        let output = Command::new(env::current_exe().unwrap())
            .args(["--exact", inner_test, "--ignored"])
            .env(TMAX_ENV_VAR, "1500")
            .env(PROBE_OUT_VAR, &answer)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child failed: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        fs::read_to_string(answer).expect("child probe answer")
    }

    fn record_resolved_tmax(body: &str, env_overlay: bool) {
        let (_dir, path) = fixture_file(body);
        let mut options = AppConfigLoadOptions::default();
        options.env_overlay = env_overlay;
        let source =
            load_app_config_with_options::<SdkAuctioneerAppConfig>(&path, "auctioneer", &options)
                .unwrap();
        let answer = format!(
            "RESOLVED_TMAX={:?}",
            source.host.resource_limits.default_tmax_ms
        );
        fs::write(env::var(PROBE_OUT_VAR).unwrap(), answer).unwrap();
    }

    #[test]
    fn env_overlay_reaches_a_live_host_resource_key() {
        assert_eq!(
            env_overlay_probe("application::config_contract::live_runtime_key_probe"),
            "RESOLVED_TMAX=Some(1500)"
        );
    }

    #[test]
    fn absent_runtime_key_is_not_env_overridable() {
        assert_eq!(
            env_overlay_probe("application::config_contract::absent_runtime_key_probe"),
            "RESOLVED_TMAX=None"
        );
    }

    #[test]
    fn no_env_keeps_the_authored_runtime_value() {
        assert_eq!(
            env_overlay_probe("application::config_contract::no_env_runtime_key_probe"),
            "RESOLVED_TMAX=Some(1000)"
        );
    }

    #[test]
    #[ignore = "spawned by env_overlay_reaches_a_live_host_resource_key"]
    fn live_runtime_key_probe() {
        record_resolved_tmax(LIVE_KEY_DOC, true);
    }

    #[test]
    #[ignore = "spawned by absent_runtime_key_is_not_env_overridable"]
    fn absent_runtime_key_probe() {
        record_resolved_tmax(ABSENT_KEY_DOC, true);
    }

    #[test]
    #[ignore = "spawned by no_env_keeps_the_authored_runtime_value"]
    fn no_env_runtime_key_probe() {
        record_resolved_tmax(LIVE_KEY_DOC, false);
    }
}
