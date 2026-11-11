//! Boot installs only the immutable observation scope. Policy belongs to config.

#[cfg(test)]
mod tests {
    #[test]
    fn serving_and_baseline_boot_install_only_the_counter_scope() {
        for source in [
            include_str!("../src/main.rs"),
            include_str!("../src/baseline_probe.rs"),
        ] {
            assert!(source.contains("install_health_counter_scope(\"process\")?"));
            for removed in [
                "decide_health_policy",
                "install_health_primitives",
                "read_bool_flag",
                "HEALTH_TOKEN_VAR",
                "ALLOW_UNAUTH_VAR",
            ] {
                assert!(
                    !source.contains(removed),
                    "boot must not read health policy: {removed}"
                );
            }
        }
    }
}
