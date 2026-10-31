//! The shipped `spin.toml` can configure health authentication.
//!
//! Spin resolves a component variable only when the APPLICATION
//! declares it AND the component maps it. The manifest shipped
//! neither, so `variables::get` always errored, the adapter read both
//! inputs as absent, and the decision table fail-closed to 401 with no
//! way for an operator to supply a token — `spin up --variable` and
//! runtime config providers can only set a DECLARED variable.
//!
//! # Why the manifest text
//!
//! The failure is a deployment-configuration one, so the check belongs
//! on the artifact an operator deploys. Two measurements set the shape
//! of this file:
//!
//! - `spin up` DOES reject the missing declaration, with
//!   `invalid variable template: unknown variable
//!   "allow_unauthenticated_health"` — but reaching it needs a built
//!   `.wasm` and a listening server, which this crate's tests have
//!   neither of;
//! - `spin doctor` does NOT catch it. With the declaration removed it
//!   still reports "No problems found", so it cannot stand in.
//!
//! The cookie-key manifest target also drives the pinned Spin loader;
//! these assertions pin the health defaults and component mappings.

#[cfg(test)]
mod tests {
    use std::fs::read_to_string;

    /// The two inputs `install_health` reads on this target.
    const HEALTH_VARIABLES: [&str; 2] = ["allow_unauthenticated_health", "health_gvl_token"];

    fn manifest() -> String {
        read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/spin.toml"))
            .expect("the adapter's own manifest is readable from its test")
    }

    /// Both variables are declared, and both are mapped into the
    /// component that reads them.
    ///
    /// Asserted as two separate facts because satisfying one alone
    /// does not work: a declaration with no mapping leaves the reads
    /// failing exactly as if nothing were declared, and a mapping with
    /// no declaration makes Spin refuse the app outright.
    #[test]
    fn both_health_variables_are_declared_and_mapped() {
        let manifest = manifest();

        let (app_section, component_section) = manifest
            .split_once("[component.auctioneer-adapter-spin.variables]")
            .expect(
                "the manifest must carry a [component.auctioneer-adapter-spin.variables] \
                 table: a declared-but-unmapped variable does not resolve",
            );

        assert!(
            app_section.contains("[variables]"),
            "the manifest must declare an application [variables] table",
        );
        for name in HEALTH_VARIABLES {
            assert!(
                app_section.contains(name),
                "[variables] must declare {name}: the adapter reads it through \
                 `variables::get`, and an undeclared variable can be set by \
                 nothing -- not `spin up --variable`, not a runtime config provider",
            );
            assert!(
                component_section.contains(name),
                "the component must map {name} for the read to resolve",
            );
        }
    }

    /// The token is a credential, and the flag fails closed.
    ///
    /// Both are properties of the DEFAULTS rather than of the wiring: a
    /// token not marked `secret` sits in the deployed manifest surface,
    /// and a flag with no default makes Spin refuse the app instead of
    /// applying the fail-closed policy an operator who configures
    /// nothing should get.
    #[test]
    fn the_token_is_secret_and_the_flag_defaults_closed() {
        let manifest = manifest();
        let declaration = manifest
            .lines()
            .find(|line| line.trim_start().starts_with("health_gvl_token ="))
            .expect("health_gvl_token is declared");
        assert!(
            declaration.contains("secret = true"),
            "health_gvl_token is a credential, so it must be declared `secret`; \
             found {declaration:?}",
        );
        let parsed: toml::Value = toml::from_str(&manifest).unwrap();
        assert_eq!(
            parsed["variables"]["health_gvl_token"]
                .get("default")
                .and_then(toml::Value::as_str),
            Some(""),
            "an empty default makes the optional variable valid without a credential"
        );

        let flag = manifest
            .lines()
            .find(|line| {
                line.trim_start()
                    .starts_with("allow_unauthenticated_health =")
            })
            .expect("allow_unauthenticated_health is declared");
        assert!(
            flag.contains(r#"default = "false""#),
            "the flag must default to \"false\" so an operator who sets nothing \
             gets the fail-closed policy rather than an unresolved variable; \
             found {flag:?}",
        );
    }
}
