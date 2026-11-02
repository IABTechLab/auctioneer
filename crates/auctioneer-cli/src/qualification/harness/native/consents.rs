use super::super::io;
use super::super::{HarnessError, Result};
use super::load::{Category, Outcome};
use auctioneer_privacy::{gvl::GvlStore, tcf::parse_tcf_v2};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use std::collections::BTreeSet;
use std::path::Path;
use std::str::from_utf8;

pub struct Consent {
    version: u16,
    value: String,
    compiled_present: bool,
    generated_tcf: bool,
}

impl Consent {
    pub const fn version(&self) -> u16 {
        self.version
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Deserialize)]
struct Manifest {
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct Entry {
    spec: u8,
    vendor_list_version: u16,
    tcf_policy_version: u8,
}

fn patch(bytes: &mut [u8], offset: u32, width: u32, value: u32) -> Result<()> {
    for index in 0..width {
        let position = offset
            .checked_add(index)
            .ok_or(HarnessError::new("reference_unavailable"))?;
        let byte = bytes
            .get_mut(
                usize::try_from(position >> 3_u32)
                    .map_err(|_error| HarnessError::new("reference_unavailable"))?,
            )
            .ok_or(HarnessError::new("reference_unavailable"))?;
        let mask = 1_u8
            .checked_shl(
                7_u32
                    .checked_sub(position & 7)
                    .ok_or(HarnessError::new("reference_unavailable"))?,
            )
            .ok_or(HarnessError::new("reference_unavailable"))?;
        let shift = width
            .checked_sub(index.saturating_add(1))
            .ok_or(HarnessError::new("reference_unavailable"))?;
        if value.checked_shr(shift).unwrap_or(0) & 1 == 1 {
            *byte |= mask;
        } else {
            *byte &= !mask;
        }
    }
    Ok(())
}

fn variant(seed: &str, entry: &Entry) -> Result<Consent> {
    let (core, trailing) = seed
        .split_once('.')
        .filter(|(_core, trailing)| !trailing.is_empty())
        .ok_or(HarnessError::new("reference_unavailable"))?;
    if entry.vendor_list_version > 4095 || entry.tcf_policy_version > 63 {
        return Err(HarnessError::new("reference_unavailable"));
    }
    let mut bytes = URL_SAFE_NO_PAD
        .decode(core)
        .map_err(|_error| HarnessError::new("reference_unavailable"))?;
    // Modify only the reviewed public seed fields. The production parser still
    // validates the complete string, including unchanged disclosure segments.
    patch(&mut bytes, 78, 12, 2)?;
    patch(&mut bytes, 120, 12, u32::from(entry.vendor_list_version))?;
    patch(&mut bytes, 132, 6, u32::from(entry.tcf_policy_version))?;
    patch(&mut bytes, 176, 24, 0)?;
    let value = format!("{}.{}", URL_SAFE_NO_PAD.encode(bytes), trailing);
    let decoded =
        parse_tcf_v2(&value).map_err(|_error| HarnessError::new("reference_unavailable"))?;
    if decoded.cmp_id != 2
        || decoded.vendor_list_version != entry.vendor_list_version
        || decoded.tcf_policy_version != entry.tcf_policy_version
        || !decoded.purposes_li_transparency.is_empty()
    {
        return Err(HarnessError::new("reference_unavailable"));
    }
    let compiled = GvlStore::baked()
        .get(3, u32::from(entry.vendor_list_version))
        .ok_or(HarnessError::new("reference_unavailable"))?;
    if compiled.tcf_policy_version() != u32::from(entry.tcf_policy_version) {
        return Err(HarnessError::new("reference_unavailable"));
    }
    Ok(Consent {
        version: entry.vendor_list_version,
        value,
        compiled_present: true,
        generated_tcf: true,
    })
}

pub fn fixtures(root: &Path) -> Result<Vec<Consent>> {
    let manifest: Manifest = io::read_json(
        &root.join("crates/auctioneer-config/data/gvl_archive/manifest.json"),
        1 << 20_u32,
    )
    .map_err(|_error| HarnessError::new("reference_unavailable"))?;
    let bytes = io::read_bytes(
        &root.join("crates/auctioneer-privacy/tests/data/tcf_v2/valid_full_consent.txt"),
        16 << 10_u32,
    )
    .map_err(|_error| HarnessError::new("reference_unavailable"))?;
    let seed = from_utf8(&bytes)
        .map_err(|_error| HarnessError::new("reference_unavailable"))?
        .trim();
    let mut versions = BTreeSet::new();
    let mut rows = Vec::new();
    for entry in manifest
        .entries
        .iter()
        .filter(|entry| entry.spec == 3)
        .take(20)
    {
        if !versions.insert(entry.vendor_list_version) {
            return Err(HarnessError::new("reference_unavailable"));
        }
        rows.push(variant(seed, entry)?);
    }
    if rows.len() < 14 {
        return Err(HarnessError::new("reference_unavailable"));
    }
    Ok(rows)
}

pub fn credit(
    result: &Outcome,
    proof: &Consent,
    supplied: Option<&str>,
    seen: &mut BTreeSet<u16>,
) -> u64 {
    u64::from(
        supplied == Some(proof.value())
            && proof.compiled_present
            && proof.generated_tcf
            && result.category == Category::Success
            && result.status == 200
            && result.auction.privacy_blocked
            && !result.auction.privacy_refused
            && seen.insert(proof.version()),
    )
}

#[cfg(test)]
mod tests {
    use super::super::load::{Auction, ConfigStatus};
    use super::*;
    use crate::qualification::profile::Digest;
    use auctioneer_privacy::tcf::parse_tcf_v2;
    use std::fs::read_to_string;

    #[test]
    fn consent_variants_preserve_disclosure_segments_and_match_fourteen_compiled_versions() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let seed = read_to_string(
            root.join("crates/auctioneer-privacy/tests/data/tcf_v2/valid_full_consent.txt"),
        )
        .unwrap();
        let suffix = seed.trim().split_once('.').unwrap().1;
        let rows = fixtures(&root).unwrap();
        assert!(rows.len() >= 14);
        let mut versions = BTreeSet::new();
        for row in rows.iter().take(14) {
            assert!(versions.insert(row.version));
            let decoded = parse_tcf_v2(&row.value).unwrap();
            assert_eq!(decoded.cmp_id, 2);
            assert_eq!(decoded.vendor_list_version, row.version);
            assert!(decoded.purposes_li_transparency.is_empty());
            assert_eq!(row.value.split_once('.').unwrap().1, suffix);
            let gvl = GvlStore::baked().get(3, u32::from(row.version)).unwrap();
            assert_eq!(
                u32::from(decoded.tcf_policy_version),
                gvl.tcf_policy_version()
            );
            assert!(row.compiled_present);
            assert!(row.generated_tcf);
        }
        assert_eq!(versions.len(), 14);
    }

    #[test]
    fn gvl_credit_requires_a_fresh_compiled_valid_consent_and_an_actual_nonstale_privacy_path() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let proof = fixtures(&root).unwrap().remove(0);
        let success = Outcome {
            category: Category::Success,
            status: 200,
            latency_us: 100,
            slow_reads: 0,
            body_sha256: Digest::of(b"response"),
            config_status: ConfigStatus::Ok,
            auction: Auction {
                privacy_blocked: true,
                ..Auction::default()
            },
        };
        assert_eq!(
            credit(&success, &proof, Some(proof.value()), &mut BTreeSet::new()),
            1
        );
        let mut checked = 0_i32;
        for mutation in [
            "http",
            "failed",
            "unblocked",
            "refused",
            "compiled",
            "generated",
            "reused",
        ] {
            let mut result = success.clone();
            let mut changed = Consent {
                version: proof.version,
                value: proof.value.clone(),
                compiled_present: true,
                generated_tcf: true,
            };
            let mut seen = BTreeSet::new();
            match mutation {
                "http" => result.status = 500,
                "failed" => result.category = Category::Error,
                "unblocked" => result.auction.privacy_blocked = false,
                "refused" => result.auction.privacy_refused = true,
                "compiled" => changed.compiled_present = false,
                "generated" => changed.generated_tcf = false,
                "reused" => {
                    seen.insert(proof.version);
                }
                _ => panic!("unreviewed consent mutation: {mutation}"),
            }
            assert_eq!(
                credit(&result, &changed, Some(changed.value()), &mut seen),
                0,
                "{mutation}"
            );
            checked += 1_i32;
        }
        assert_eq!(checked, 7_i32);
    }

    #[test]
    fn parse_credit_binds_the_generated_proof_to_the_exact_supplied_consent() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let proofs = fixtures(&root).unwrap();
        let proof = &proofs[0];
        let other = &proofs[1];
        let core = proof.value().split_once('.').unwrap().0;
        let result = Outcome {
            category: Category::Success,
            status: 200,
            latency_us: 1,
            slow_reads: 0,
            body_sha256: Digest::of(b"denied"),
            config_status: ConfigStatus::Ok,
            auction: Auction {
                privacy_blocked: true,
                ..Auction::default()
            },
        };
        let mut checked = 0_u8;
        for supplied in [
            None,
            Some("not-a-TC-string"),
            Some(core),
            Some(other.value()),
        ] {
            let mut seen = BTreeSet::new();
            assert_eq!(credit(&result, proof, supplied, &mut seen), 0);
            assert!(seen.is_empty());
            checked = checked.checked_add(1).unwrap();
        }
        let mut seen = BTreeSet::new();
        assert_eq!(credit(&result, proof, Some(proof.value()), &mut seen), 1);
        assert_eq!(seen, BTreeSet::from([proof.version()]));
        assert_eq!(checked, 4);
    }
}
