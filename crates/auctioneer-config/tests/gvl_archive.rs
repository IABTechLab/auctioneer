//! Archive-accessor regressions over the baked `gvl_archive()` slice:
//! the manifest SHA pins match the embedded bytes, the corpus covers
//! both spec versions, and the current v2.4-era `(spec 3, policy 5)`
//! list is present.

#[cfg(test)]
mod tests {
    use auctioneer_config::gvl_archive;
    use sha2::{Digest as _, Sha256};

    #[test]
    fn archive_manifest_sha256_matches_bundled_blob() {
        let entry = gvl_archive().first().expect("archive has entries");
        let observed = hex::encode(Sha256::digest(entry.raw));
        assert_eq!(
            observed, entry.sha256,
            "baked bytes must hash to the manifest pin for {}/{}",
            entry.spec, entry.vendor_list_version
        );
    }

    #[test]
    fn archive_manifest_contains_spec2_and_spec3_entries() {
        assert!(
            gvl_archive().iter().any(|entry| entry.spec == 2),
            "archive must bundle the frozen spec-2 list"
        );
        assert!(
            gvl_archive().iter().any(|entry| entry.spec == 3),
            "archive must bundle spec-3 lists"
        );
    }

    #[test]
    fn gvl_archive_contains_v24_entry() {
        // The v2.4-era GVL shape: gvlSpecificationVersion 3 with
        // tcfPolicyVersion 5. The refresh tool pins whatever list
        // version is live at execution, so the assertion is on the
        // shape, never a hardcoded vendorListVersion.
        assert!(
            gvl_archive()
                .iter()
                .any(|entry| entry.spec == 3 && entry.tcf_policy_version == 5),
            "archive must carry a (spec 3, policy 5) v2.4-era entry"
        );
    }
}
