//! Every non-auction route registered in the route table runs inside
//! the aux-egress wrapper.
//!
//! This is a SOURCE-LEVEL assertion, and deliberately so. The property
//! is "no aux handler is registered without the wrapper", which is a
//! statement about the whole set of registered handlers — and a
//! behavioural test can only ever exercise the handlers someone
//! remembered to write a test for. The leak this guards against is
//! precisely a handler nobody remembered: it would acquire an aux
//! permit at admission with nothing installing it, and the permit would
//! be released at handler return rather than at terminal egress.
//!
//! It reads the route table rather than a hand-maintained list, so
//! adding a route to `edgezero.toml` without wrapping its handler fails
//! here rather than shipping.

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    /// The one route that is NOT an aux route.
    ///
    /// The auction handler installs its own egress permit inline,
    /// because it acquires AFTER the auction resolves rather than at
    /// admission — there is nothing to egress until the auction has
    /// produced something. It is named here explicitly rather than
    /// matched by a pattern, so a new route cannot quietly inherit the
    /// exemption by being called something auction-ish.
    const NOT_AUX: &[&str] = &["auction"];

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("the crate sits two levels under the workspace root")
            .to_path_buf()
    }

    /// The registered handler function names, keyed by route id, read
    /// out of the route table.
    ///
    /// A minimal scan rather than a TOML parse: the file's `[[triggers.http]]`
    /// blocks are flat `key = "value"` pairs, and depending on a parser
    /// here would let a parser upgrade change what this test reads.
    fn registered_handlers() -> BTreeMap<String, String> {
        let manifest = fs::read_to_string(repo_root().join("edgezero.toml"))
            .expect("the route table is readable from the workspace root");
        let mut routes = BTreeMap::new();
        let mut id: Option<String> = None;
        for raw in manifest.lines() {
            let line = raw.trim();
            if line == "[[triggers.http]]" {
                id = None;
            } else if let Some(value) = field(line, "id") {
                id = Some(value);
            } else if let Some(value) = field(line, "handler") {
                if let Some(route_id) = id.take() {
                    let function = value
                        .rsplit("::")
                        .next()
                        .expect("a handler path has a last segment")
                        .to_owned();
                    routes.insert(route_id, function);
                }
            } else {
                // Every other key in the block — path, methods,
                // adapters, description — is irrelevant to which
                // handler serves the route.
            }
        }
        assert!(
            !routes.is_empty(),
            "the route table must declare at least one handler",
        );
        routes
    }

    /// `key = "value"` on one line, or `None`.
    fn field(line: &str, key: &str) -> Option<String> {
        let rest = line.strip_prefix(key)?.trim_start();
        let value = rest.strip_prefix('=')?.trim();
        Some(value.trim_matches('"').to_owned())
    }

    /// Every handler source in the handlers directory, concatenated.
    fn handler_sources() -> String {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/handlers");
        let mut all = String::new();
        for entry in fs::read_dir(&dir).expect("the handlers directory is readable") {
            let path = entry.expect("a readable directory entry").path();
            if path.extension().is_some_and(|ext| ext == "rs") {
                all.push_str(&fs::read_to_string(&path).expect("a readable handler source"));
            }
        }
        all
    }

    /// Every registered aux route's handler calls the wrapper.
    ///
    /// A test that only exercised the three aux routes someone thought
    /// to test would pass against a fourth, unwrapped one — which is
    /// exactly the leak this exists to catch.
    #[test]
    fn every_aux_route_is_wrapped() {
        let sources = handler_sources();
        let mut unwrapped = Vec::new();

        for (id, function) in registered_handlers() {
            if NOT_AUX.contains(&id.as_str()) {
                continue;
            }
            // Only sync protocols defer body reading until their own
            // policy checks. Every other route uses the shared bounded drain.
            let wrapper = if matches!(
                function.as_str(),
                "cookie_sync" | "cookie_sync_preflight" | "setuid" | "getuids"
            ) {
                "with_aux_egress_preserving_body"
            } else {
                "with_aux_egress"
            };
            let expected = format!("{wrapper}(ctx, {function}_body)");
            if !sources.contains(&expected) {
                unwrapped.push(format!("{id} (fn {function})"));
            }
        }

        assert!(
            unwrapped.is_empty(),
            "these registered aux routes do not run inside `with_aux_egress`, so their \
             admission permits would be released at handler return instead of at terminal \
             egress: {unwrapped:?}",
        );
    }

    /// The exemption list names only routes that actually exist, so a
    /// renamed or deleted route cannot leave a stale exemption behind
    /// that silently covers a future route of the same name.
    #[test]
    fn the_non_aux_exemption_names_only_live_routes() {
        let registered = registered_handlers();
        for exempt in NOT_AUX {
            assert!(
                registered.contains_key(*exempt),
                "`{exempt}` is exempted from the aux wrapper but is not a registered route",
            );
        }
    }

    /// The scan found the routes the table actually declares, so a
    /// parsing mistake cannot make `every_aux_route_is_wrapped` pass by
    /// finding nothing to check.
    #[test]
    fn the_route_scan_sees_every_declared_route() {
        let routes = registered_handlers();
        for expected in [
            "auction",
            "status",
            "info_bidders",
            "info_bidder",
            "health_gvl",
            "cookie_sync",
            "cookie_sync_preflight",
            "setuid",
            "getuids",
        ] {
            assert!(
                routes.contains_key(expected),
                "the route scan missed `{expected}`; it would then check nothing",
            );
        }
    }
}
