//! The two consumers of [`AuctioneerConfigSource`] instantiate it with
//! different generic arguments, so nothing but a shared test proves
//! they still agree on the ROOT shape. One fixture document must
//! deserialize under both instantiations — mismatched namespace names,
//! a required field turning optional on one side, or a
//! `deny_unknown_fields` slipping off would show up here first.

#[cfg(test)]
mod tests {
    use auctioneer_config_schema::AuctioneerConfigSource;
    use serde::Deserialize;

    /// A minimal document with both namespaces present. The baked
    /// tables carry a couple of keys each so a `toml::Value`
    /// instantiation has something to hold; the real strict shapes
    /// live in `auctioneer-config`'s build script.
    const FIXTURE: &str = r#"
[baked.host]
status_response = ""

[baked.account]
disabled = false

[runtime]
[runtime.auction]
"#;

    /// Stand-in for the build script's `HostDefaultRaw` / `AccountRaw`:
    /// a STRICT subtree type, which is what that side instantiates.
    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct StrictHost {
        status_response: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct StrictAccount {
        disabled: bool,
    }

    /// Stand-in for `AuctioneerAppConfig`: an all-`Option` overlay type,
    /// which is what the CLI side instantiates.
    #[derive(Debug, Default, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct OverlayRuntime {
        #[serde(default)]
        auction: Option<toml::Value>,
    }

    #[test]
    fn build_and_cli_agree_on_the_root_shape() {
        // The build script's instantiation: baked strict, runtime opaque.
        let build_side: AuctioneerConfigSource<StrictHost, StrictAccount, toml::Value> =
            toml::from_str(FIXTURE).expect("the build-side instantiation must accept the document");
        assert_eq!(build_side.baked.host.status_response, "");
        assert!(!build_side.baked.account.disabled);
        assert!(
            build_side.runtime.is_table(),
            "the build side needs `[runtime]` only to prove it exists",
        );

        // The CLI's instantiation: baked opaque, runtime typed.
        let cli_side: AuctioneerConfigSource<toml::Value, toml::Value, OverlayRuntime> =
            toml::from_str(FIXTURE).expect("the CLI-side instantiation must accept the document");
        assert!(
            cli_side.baked.host.is_table() && cli_side.baked.account.is_table(),
            "the CLI carries the baked tables as opaque values it never re-validates",
        );
        assert!(
            cli_side.runtime.auction.is_some(),
            "the CLI side is the one that types `[runtime]`",
        );
    }

    #[test]
    fn root_rejects_an_unknown_namespace() {
        // A `[runtim]` typo must not sail past as an unread table.
        let typo = FIXTURE.replace("[runtime]", "[runtim]\n[runtime]");
        let err =
            toml::from_str::<AuctioneerConfigSource<StrictHost, StrictAccount, toml::Value>>(&typo)
                .expect_err("an unknown root namespace must be rejected");
        assert!(
            err.to_string().contains("unknown field") && err.to_string().contains("runtim"),
            "the error should name the unknown root key. Got: {err}",
        );
    }

    #[test]
    fn baked_namespace_rejects_an_unknown_table() {
        let stray = format!("{FIXTURE}\n[baked.other]\nx = 1\n");
        let err = toml::from_str::<AuctioneerConfigSource<StrictHost, StrictAccount, toml::Value>>(
            &stray,
        )
        .expect_err("an unknown `[baked.*]` table must be rejected");
        assert!(
            err.to_string().contains("unknown field") && err.to_string().contains("other"),
            "the error should name the unknown baked namespace. Got: {err}",
        );
    }

    #[test]
    fn both_namespaces_are_required() {
        let no_runtime = FIXTURE.replace("[runtime]\n[runtime.auction]\n", "");
        let err = toml::from_str::<AuctioneerConfigSource<StrictHost, StrictAccount, toml::Value>>(
            &no_runtime,
        )
        .expect_err("`[runtime]` is required, not defaulted");
        assert!(
            err.to_string().contains("runtime"),
            "the error should name the missing `runtime` field. Got: {err}",
        );

        let no_baked = "[runtime]\n";
        let baked_err = toml::from_str::<
            AuctioneerConfigSource<StrictHost, StrictAccount, toml::Value>,
        >(no_baked)
        .expect_err("`[baked]` is required, not defaulted");
        assert!(
            baked_err.to_string().contains("baked"),
            "the error should name the missing `baked` field. Got: {baked_err}",
        );
    }
}
