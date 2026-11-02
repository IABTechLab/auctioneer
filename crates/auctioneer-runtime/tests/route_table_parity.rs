//! `RouteTable` ↔ `edgezero.toml` parity. The ingress hook rejects on
//! the `RouteTable`, but the handlers are wired by `edgezero.toml`'s
//! `[[triggers.http]]` blocks — two hand-maintained lists. If they
//! drift, the hook can 404 a route that IS wired, or apply the wrong
//! cap. There is no codegen from the manifest, so this test pins a
//! bijection between the two on `(id, method, path)` — `id` included,
//! because `match_route` returns `route_id` and the ingress hook
//! selects the route-specific rejection body from it, so a table whose
//! id disagrees with the manifest would emit the wrong 400 body even
//! when method / path / cap line up.

#[cfg(test)]
mod tests {
    use auctioneer_runtime::body_caps::{
        AUCTION_CAP_BYTES, COOKIESYNC_CAP_BYTES, DEFAULT_CAP_BYTES, HEALTH_GVL_CAP_BYTES, table,
    };

    type RouteTuple = (String, String, String);

    fn assert_no_duplicates(sorted: &[RouteTuple], side: &str) {
        for pair in sorted.windows(2) {
            // `windows(2)` always yields two elements; the `else`
            // arm is unreachable but keeps the pattern irrefutable
            // without indexing.
            let [first, second] = pair else { continue };
            assert_ne!(
                first, second,
                "{side} lists the same (id, method, path) twice"
            );
        }
    }

    #[test]
    fn route_table_matches_edgezero_toml() {
        let manifest: toml::Value =
            toml::from_str(include_str!("../../../edgezero.toml")).expect("edgezero.toml parses");
        let entries = manifest
            .get("triggers")
            .and_then(|triggers| triggers.get("http"))
            .and_then(toml::Value::as_array)
            .expect("[[triggers.http]] entries");
        assert!(
            !entries.is_empty(),
            "edgezero.toml wires no HTTP triggers - the parity test has nothing to pin"
        );

        // `methods` is a TOML ARRAY (`methods = ["GET"]`), so flatten
        // each entry to one (id, method, path) tuple per method.
        let mut manifest_tuples: Vec<RouteTuple> = Vec::new();
        for entry in entries {
            let id = entry
                .get("id")
                .and_then(toml::Value::as_str)
                .expect("trigger id");
            let path = entry
                .get("path")
                .and_then(toml::Value::as_str)
                .expect("trigger path");
            let methods = entry
                .get("methods")
                .and_then(toml::Value::as_array)
                .expect("trigger methods array");
            for method in methods {
                let method_name = method.as_str().expect("method is a string");
                manifest_tuples.push((id.to_owned(), method_name.to_owned(), path.to_owned()));
            }
        }

        let mut table_tuples: Vec<RouteTuple> = table()
            .routes()
            .iter()
            .map(|route| {
                (
                    route.id.to_owned(),
                    route.method.as_str().to_owned(),
                    route.path.to_owned(),
                )
            })
            .collect();

        manifest_tuples.sort();
        table_tuples.sort();
        // Duplicate-free on both sides + equal as sorted sequences ⇒
        // every manifest tuple has exactly one table Route (no wired
        // route missing → no spurious 404) AND every table Route has
        // exactly one manifest tuple (no phantom entry advertising a
        // cap for an unwired route).
        assert_no_duplicates(&manifest_tuples, "edgezero.toml");
        assert_no_duplicates(&table_tuples, "body_caps::table()");
        assert_eq!(
            manifest_tuples, table_tuples,
            "body_caps::table() must mirror edgezero.toml's [[triggers.http]] set exactly"
        );

        // Per-route cap classes: 256 KiB auction / 32 KiB cookie_sync
        // / 4 KiB for the no-body routes including health.
        for route in table().routes() {
            let expected = match route.id {
                "auction" => AUCTION_CAP_BYTES,
                "cookie_sync" => COOKIESYNC_CAP_BYTES,
                "health_gvl" => HEALTH_GVL_CAP_BYTES,
                _ => DEFAULT_CAP_BYTES,
            };
            assert_eq!(
                route.cap_bytes, expected,
                "route {} carries the wrong body cap",
                route.id
            );
        }
    }
}
