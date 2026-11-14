//! The resolved decoder tuple, asserted against `Cargo.lock`.
//!
//! Four of the six decoder crates are not auctioneer dependencies at all — they
//! reach the build through `edgezero-core`, which pins them exactly.
//! This test is therefore the ONLY witness that the decoder auctioneer's
//! memory budget is measured against is the decoder that actually runs:
//! a transitive bump, or a second resolved copy of any of them, would
//! otherwise invalidate the bound silently.
//!
//! It is a pure lockfile parse. It links no codec crate, enables no
//! feature and allocates no decoder, so it stays valid on every target
//! and costs nothing to run.
//!
//! The same parse also anchors the `EdgeZero` revision, because the
//! decoder tuple and the revision that pins it are one fact: a
//! lockfile carrying the tuple at a different revision would prove
//! nothing.
//!
//! Also anchor the JSON and native parser dependencies whose exact versions
//! the SDK's config and ingress allocation audits require.

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs::read_to_string;
    use std::path::PathBuf;

    /// The `EdgeZero` revision every `edgezero-*` package must resolve
    /// to. Asserted structurally — over `source =` lines only — so a
    /// renamed dependency cannot make the check vacuous.
    const EDGEZERO_REV: &str = "a7d7fd0f2face027e3810ed67e80f6341fcf68b6";
    const EDGEZERO_REPOSITORY: &str = "https://github.com/IABTechLab/edgezero.git";

    /// The tuple `Cargo.lock` must resolve. Every exact version here is
    /// upstream's: auctioneer asserts them, it does not declare them.
    const ASSERTED: &[(&str, &str)] = &[
        ("async-compression", "0.4.43"),
        ("brotli", "8.0.4"),
        ("brotli-decompressor", "5.0.1"),
        ("compression-codecs", "0.4.38"),
        ("compression-core", "0.4.32"),
        ("flate2", "1.1.9"),
        ("serde_json", "1.0.150"),
        ("hyper", "1.12.0"),
        ("http", "1.4.2"),
        ("bytes", "1.12.1"),
    ];

    fn lockfile_text() -> String {
        let lockfile = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("Cargo.lock");
        read_to_string(&lockfile)
            .unwrap_or_else(|error| panic!("read {}: {error}", lockfile.display()))
    }

    /// Every `[[package]]` name paired with each version resolved for
    /// it. A name with two versions yields two entries, which is
    /// exactly the second-copy case this test has to catch.
    fn resolved_versions() -> BTreeMap<String, Vec<String>> {
        let text = lockfile_text();
        let mut resolved: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut name: Option<String> = None;
        for line in text.lines() {
            if let Some(value) = line.strip_prefix("name = ") {
                name = Some(value.trim_matches('"').to_owned());
                continue;
            }
            // A `version` key always follows its package's `name` key
            // within the same `[[package]]` block.
            if let Some(value) = line.strip_prefix("version = ")
                && let Some(package) = name.take()
            {
                resolved
                    .entry(package)
                    .or_default()
                    .push(value.trim_matches('"').to_owned());
            }
        }
        resolved
    }

    #[test]
    fn codec_pins_match_lockfile() {
        let resolved = resolved_versions();
        for &(crate_name, want) in ASSERTED {
            let versions = resolved.get(crate_name).map_or(&[][..], Vec::as_slice);
            assert_eq!(
                versions.len(),
                1,
                "{crate_name} must resolve to exactly one version, got {versions:?} — \
                 a second copy means the measured decoder is not the one that runs",
            );
            assert_eq!(
                versions.first().map(String::as_str),
                Some(want),
                "{crate_name} drifted from the asserted decoder tuple",
            );
        }
    }

    #[test]
    fn edgezero_sources_all_carry_the_canonical_repository_and_pinned_revision() {
        let text = lockfile_text();
        let edgezero_sources = text
            .lines()
            .filter(|line| line.starts_with("source = \"git+") && line.contains("edgezero"))
            .collect::<Vec<_>>();
        assert!(
            !edgezero_sources.is_empty(),
            "no edgezero git source in Cargo.lock, so a renamed dependency \
             must not make this assertion vacuous",
        );
        let expected =
            format!("source = \"git+{EDGEZERO_REPOSITORY}?rev={EDGEZERO_REV}#{EDGEZERO_REV}\"");
        let stale = edgezero_sources
            .iter()
            .filter(|line| **line != expected)
            .collect::<Vec<_>>();
        assert!(
            stale.is_empty(),
            "every edgezero source must use the canonical repository at {EDGEZERO_REV}, \
             these do not: {stale:?}",
        );
    }
}
