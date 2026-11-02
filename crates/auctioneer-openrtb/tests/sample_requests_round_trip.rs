//! Round-trips sample-request fixtures through the auctioneer types.
//!
//! ## Fixture roots
//!
//! Both committed roots are always required; a configured upstream root is too:
//! - `crates/auctioneer-openrtb/tests/fixtures/` — generic model fixtures.
//! - `crates/auctioneer-bidder/tests/conformance/fixtures/openrtb/` — the unchanged
//!   curated upstream captures, committed under the upstream Apache-2.0
//!   license. These remain available to CI without the upstream fixture clone.
//! - `$PREBID_SERVER_DIR/endpoints/openrtb2/sample-requests/valid-whole/`
//!   — the full upstream fixture set, walked when the operator sets
//!   the env var to a local clone. CI configures its pinned checkout here.
//!   A missing, empty, or unreadable configured corpus fails the test instead
//!   of silently falling back to the curated subset.
//!
//! ## Contract
//!
//! Every walked fixture must DESERIALIZE into `BidRequest` and SERIALIZE
//! back to a `serde_json::Value` whose contents are EQUAL to the input.
//! Unknown fields use flattened catch-alls; classified device context preserves
//! explicit nulls, empty containers, and JSON number forms with typed fields.
//! This is JSON-value equality, not preservation of whitespace or key order.
//! Privacy scrubbing is a separate transformation after this round trip.
#[cfg(test)]
mod tests {
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use serde_json::Value;
    use std::env;
    use std::fs;
    use std::panic::catch_unwind;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    /// Each skipped filename must document its model divergence.
    const SKIP: &[&str] = &[];

    fn collect_fixture_roots() -> Vec<PathBuf> {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut roots = vec![
            manifest.join("tests/fixtures"),
            manifest.join("../auctioneer-bidder/tests/conformance/fixtures/openrtb"),
        ];
        for root in &roots {
            assert!(
                root.is_dir(),
                "vendored fixture root is missing: {}",
                root.display()
            );
        }
        if let Some(dir) = env::var_os("PREBID_SERVER_DIR") {
            let full = Path::new(&dir).join("endpoints/openrtb2/sample-requests/valid-whole");
            assert!(
                full.is_dir(),
                "configured fixture root is missing: {}",
                full.display()
            );
            roots.push(full);
        }
        roots
    }

    fn collect_fixture_paths(roots: &[PathBuf]) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        for root in roots {
            let before = paths.len();
            walk(root, &mut paths);
            assert!(
                paths.len() > before,
                "fixture root contains no JSON fixtures: {}",
                root.display()
            );
        }
        paths.sort();
        paths.dedup();
        paths
    }

    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries = fs::read_dir(dir).unwrap_or_else(|error| {
            panic!(
                "cannot traverse fixture directory {}: {error}",
                dir.display()
            )
        });
        for result in entries {
            let entry = result.unwrap_or_else(|error| {
                panic!("cannot read fixture entry in {}: {error}", dir.display())
            });
            let path = entry.path();
            let metadata = fs::metadata(&path).unwrap_or_else(|error| {
                panic!("cannot inspect fixture path {}: {error}", path.display())
            });
            if metadata.is_dir() {
                walk(&path, out);
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                out.push(path);
            }
        }
    }

    #[test]
    fn configured_missing_corpus_is_not_silently_replaced_by_vendored_fixtures() {
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "tests::every_walked_fixture_round_trips_json_value_parity",
            ])
            .env(
                "PREBID_SERVER_DIR",
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("tests/fixtures/a-corpus-that-does-not-exist"),
            )
            .output()
            .unwrap();
        assert!(
            !output.status.success(),
            "an explicitly configured missing corpus must fail, not run the vendored subset"
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("configured fixture root"),
            "failure must identify the configured root: {output:?}"
        );
    }

    #[test]
    fn unreadable_fixture_root_is_not_silently_skipped() {
        let roots = [Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")];
        let result = catch_unwind(|| collect_fixture_paths(&roots));
        assert!(result.is_err(), "a non-directory fixture root must fail");
    }

    #[test]
    fn committed_fixture_population_retains_local_and_captured_inputs() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let local = manifest.join("tests/fixtures");
        let captured = manifest.join("../auctioneer-bidder/tests/conformance/fixtures/openrtb");
        let mut expected = vec![
            local.join("device-context-number-and-null.json"),
            captured.join("dooh.json"),
            captured.join("prebid-test-ad.json"),
            captured.join("simple.json"),
            captured.join("targeting-optional-all-false.json"),
            captured.join("user-eids-uids-missing.json"),
            captured.join("user-ext-eids-empty.json"),
        ];
        expected.sort();
        let actual: Vec<_> = collect_fixture_paths(&collect_fixture_roots())
            .into_iter()
            .filter(|path| path.starts_with(&local) || path.starts_with(&captured))
            .collect();
        assert_eq!(
            actual, expected,
            "committed fixture coverage must not shrink"
        );
    }

    #[test]
    fn every_walked_fixture_round_trips_json_value_parity() {
        let roots = collect_fixture_roots();
        assert!(
            !roots.is_empty(),
            "no fixture roots: both committed fixture roots must exist",
        );
        let paths = collect_fixture_paths(&roots);
        assert!(
            !paths.is_empty(),
            "no fixtures found under {roots:?} — vendored subset went missing?",
        );

        let mut checked = 0_usize;
        let mut skipped = 0_usize;
        let mut failed: Vec<String> = Vec::new();

        for path in &paths {
            let filename = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            if SKIP.contains(&filename) {
                skipped += 1;
                continue;
            }

            let raw = fs::read_to_string(path).unwrap();
            let envelope: Value = serde_json::from_str(&raw)
                .unwrap_or_else(|err| panic!("invalid json in {path:?}: {err}"));

            // Most fixtures wrap the bid request under mockBidRequest;
            // some are themselves the bid request.
            let bid_request_json = envelope.get("mockBidRequest").cloned().unwrap_or(envelope);

            let parsed: BidRequest = match serde_json::from_value(bid_request_json.clone()) {
                Ok(value) => value,
                Err(err) => {
                    failed.push(format!("DESERIALIZE {filename}: {err}"));
                    continue;
                }
            };
            let back: Value = match serde_json::to_value(&parsed) {
                Ok(value) => value,
                Err(err) => {
                    failed.push(format!("SERIALIZE {filename}: {err}"));
                    continue;
                }
            };
            if back != bid_request_json {
                failed.push(format!(
                    "ROUND-TRIP DRIFT {filename}: serialize output differs from input"
                ));
                continue;
            }
            checked += 1;
        }

        assert!(
            failed.is_empty(),
            "{} fixture failures (checked {}, skipped {}):\n  {}",
            failed.len(),
            checked,
            skipped,
            failed.join("\n  "),
        );
        // Every successfully discovered non-skipped fixture must be checked.
        // Root and traversal validation above prevents silently reduced coverage.
        let expected = paths.len() - skipped;
        assert_eq!(
            checked,
            expected,
            "drift: walked {paths_len} fixtures, skipped {skipped}, but only \
         {checked} were checked (expected {expected})",
            paths_len = paths.len(),
        );
    }
}
