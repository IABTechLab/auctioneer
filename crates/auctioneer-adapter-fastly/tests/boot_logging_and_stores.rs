//! Host-visible hooks and source guards for the wasm boot path.
//! Actual logger output and SDK store reads run separately in Viceroy.

#[cfg(test)]
mod tests {
    use auctioneer_core::App;
    use edgezero_core::app::Hooks;
    use edgezero_core::manifest::LogLevel;

    fn source() -> &'static str {
        include_str!("../src/lib.rs")
    }

    #[test]
    fn auctioneer_declares_that_it_owns_logging() {
        assert!(
            <App as Hooks>::owns_logging(),
            "the SDK must not install a second global sink after Auctioneer's boot logger"
        );
    }

    #[test]
    fn fastly_logging_is_baked_into_the_generated_hooks() {
        let logging = <App as Hooks>::logging_for("fastly");
        assert_eq!(logging.endpoint.as_deref(), Some("auctioneer_log"));
        assert_eq!(logging.level, LogLevel::Info);
        assert_eq!(logging.echo_stdout, Some(true));
    }

    #[test]
    fn the_health_bootstrap_has_both_logical_aliases_declared() {
        let stores = <App as Hooks>::stores();
        for declared in [stores.config, stores.secrets] {
            let store = declared.expect("health requires declared config and secret stores");
            assert!(!store.default.is_empty());
            assert!(store.ids.contains(&store.default));
        }
    }

    #[test]
    fn boot_logging_uses_baked_hooks_without_runtime_environment_reads() {
        let source = source();
        assert!(
            source.contains("FastlyLogging::from(A::logging_for(\"fastly\"))"),
            "the boot sink must use the same baked settings as the SDK"
        );
        for removed in ["runtime_env_config", "EnvConfig", "env.store_name("] {
            assert!(
                !source.contains(removed),
                "runtime boot must not depend on removed runtime settings: {removed}"
            );
        }
    }

    #[test]
    fn the_sink_precedes_the_envelope_and_sdk_dispatch_owns_the_lifecycle() {
        let source = source();
        let sink = source.find("install_boot_log::<A>();").expect("boot sink");
        let envelope = source
            .find("bootstrap_adapter_with_envelope(")
            .expect("boot envelope");
        let health = source.find("install_health()?").expect("health setup");
        let dispatch = source
            .find("run_app::<A>()")
            .expect("SDK manifest-aware lifecycle helper");
        assert!(sink < envelope && envelope < health && health < dispatch);
        assert!(source.contains("run_with_hooks::<AuctioneerApp>()"));
        // SDK run_app owns fallible assembly, baked registries and terminal
        // delivery. Auctioneer must not reintroduce the retired partial lifecycle.
        assert!(!source.contains("EdgeApp::build::<A>"));
        assert!(!source.contains("send_with_registries_and_hooks("));
    }

    #[test]
    fn the_health_bootstrap_opens_the_same_logical_aliases_as_sdk_registries() {
        let source = source();
        assert!(source.contains("let stores = <AuctioneerApp as Hooks>::stores();"));
        for direct in [
            "ConfigStore::try_open(config_id)?",
            "SecretStore::open(secrets_id)?",
        ] {
            assert!(
                source.contains(direct),
                "health and SDK registries must open baked resource-link aliases: {direct}"
            );
        }
    }

    #[test]
    fn a_store_fault_leaves_health_closed_rather_than_stopping_the_router() {
        let source = source();
        let install_body = source
            .split("fn install_health(")
            .nth(1)
            .and_then(|rest| rest.split("fn resolve_health_policy(").next())
            .expect("health installation precedes fallible policy resolution");
        assert!(install_body.contains("match resolve_health_policy()"));
        assert!(install_body.contains("install_health_primitives(HealthAccessPolicy::Closed"));
        assert!(install_body.contains("health policy unresolved"));
        assert!(!install_body.contains("ConfigStore::try_open"));
        assert!(!install_body.contains("SecretStore::open"));
        assert!(!source.contains("ConfigStore::open("));
        assert!(!source.contains(".plaintext()"));
    }
}
