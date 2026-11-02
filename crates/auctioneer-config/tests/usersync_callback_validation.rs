use auctioneer_config_schema::codegen;

#[cfg(test)]
mod tests {
    use super::codegen::{HostDefaultRaw, RenderError, render_host_config};

    fn source_host() -> HostDefaultRaw {
        let source: toml::Value = toml::from_str(include_str!("../../../auctioneer.toml"))
            .expect("committed config source");
        source["baked"]["host"]
            .clone()
            .try_into()
            .expect("baked host config")
    }

    fn assert_user_sync_error(host: &HostDefaultRaw, expected: &str) {
        let error = render_host_config(host).unwrap_err();
        let RenderError::UserSync(message) = error else {
            panic!("expected user-sync validation error, got {error:?}");
        };
        assert_eq!(message, expected);
    }

    #[test]
    fn baked_callback_codegen_rejects_ambiguous_bidder_identity() {
        let mut host = source_host();
        for query in [
            "uid={{.UserMacro}}",
            "bidder=fake_fixture_base&uid={{.UserMacro}}",
            "bidder=fake_fixture_base&bidder={{.SyncerKey}}&uid={{.UserMacro}}",
            "%62idder=fake_fixture_base&bidder={{.SyncerKey}}&uid={{.UserMacro}}",
        ] {
            host.user_sync.redirect_url = Some(format!("{{{{.ExternalURL}}}}/setuid?{query}"));
            assert_user_sync_error(
                &host,
                "user_sync.redirect_url must contain one bound bidder macro field",
            );
        }
    }

    #[test]
    fn baked_callback_codegen_rejects_repaired_base_urls() {
        let mut host = source_host();
        for (base, expected) in [
            (
                "https:sync.example.com",
                "sync URL must have an explicit scheme and authority",
            ),
            (
                "https:/sync.example.com",
                "sync URL must have an explicit scheme and authority",
            ),
            (
                "https://sync.example.com ",
                "sync URL must have a literal authority and no whitespace or backslash",
            ),
        ] {
            host.user_sync.external_url = base.to_owned();
            assert_user_sync_error(&host, expected);
        }
    }

    #[test]
    fn baked_callback_codegen_rejects_embedded_scheme_delimiter() {
        let base = "https:sync.example.test://unused";
        let mut host = source_host();
        host.user_sync.external_url = base.to_owned();
        assert_user_sync_error(&host, "sync URL must have an explicit scheme and authority");

        host.user_sync.external_url = "https://sync.example.test".to_owned();
        host.user_sync.redirect_url = Some(format!(
            "{base}/setuid?bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
        ));
        assert_user_sync_error(
            &host,
            "user_sync.redirect_url must resolve to an absolute URL",
        );
    }

    #[test]
    fn baked_callback_codegen_rejects_shared_nonpublic_hosts() {
        let mut host = source_host();
        for base in [
            "https://0.1.2.3",
            "https://[fec0::1]",
            "https://[64:ff9b::a00:1]",
            "https://[100::1]",
            "https://[3fff::1]",
        ] {
            host.user_sync.external_url = base.to_owned();
            assert_user_sync_error(&host, "sync URL cannot use a private literal host");
        }
    }

    #[test]
    fn baked_callback_codegen_rejects_root_dotted_loopback_names() {
        for name in [
            "localhost",
            "sub.localhost",
            "nip.io",
            "xip.io",
            "lvh.me",
            "localtest.me",
            "vcap.me",
            "127.0.0.1.nip.io",
            "app.xip.io",
            "app.lvh.me",
            "app.localtest.me",
            "app.vcap.me",
        ] {
            let base = format!("https://{name}.:8443");
            let mut host = source_host();
            host.user_sync.external_url = format!("{base}/base/");
            assert_user_sync_error(&host, "sync URL cannot use a private literal host");

            host = source_host();
            host.user_sync.redirect_url = Some(format!(
                "{base}/setuid?bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
            ));
            assert_user_sync_error(
                &host,
                "user_sync.redirect_url cannot use a private literal host",
            );

            host = source_host();
            host.user_sync.allowed_origins = vec![base.clone()];
            assert_user_sync_error(&host, "sync URL cannot use a private literal host");
        }
    }

    #[test]
    fn baked_callback_codegen_preserves_public_dns_and_normalized_ip_controls() {
        for name in [
            "sync.example.com.",
            "snip.io.",
            "8.8.8.8",
            "8.8.8.8.",
            "0x08080808",
            "[2606:4700:4700::1111]",
        ] {
            let mut host = source_host();
            let base = format!("https://{name}:8443/base%20path");
            host.user_sync.external_url = base.clone();
            host.user_sync.redirect_url = Some(format!(
                "{base}/setuid?bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
            ));
            assert!(render_host_config(&host).is_ok(), "{base}");
        }
        for name in [
            "127.0.0.2.",
            "0x7f000002",
            "10.0.0.1.",
            "0x0a000001",
            "[fc00::1]",
        ] {
            let mut host = source_host();
            host.user_sync.external_url = format!("https://{name}");
            assert_user_sync_error(&host, "sync URL cannot use a private literal host");
        }
    }
}
