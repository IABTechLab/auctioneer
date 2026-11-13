//! Unicode 16 qualification with an independent official-data oracle and frozen Go evidence.

#[cfg(test)]
mod tests {
    use auctioneer_config_data::activity::simple_fold_eq;
    use serde::Deserialize;
    use sha2::{Digest as _, Sha256};
    use std::collections::{BTreeMap, BTreeSet};
    use std::num::NonZeroU32;
    use std::str::from_utf8;

    const OFFICIAL: &[u8] = include_bytes!("fixtures/unicode/CaseFolding-16.0.0.txt");
    const OFFICIAL_SHA256: &str =
        "6f1f9c588eb4a5c718d9e8f93b782685e5c7fec872cf05e8e6878053599e09bb";
    const GO_REFERENCE: &[u8] =
        include_bytes!("../../auctioneer-privacy/tests/data/privacy/reference.json");
    const GO_SHA256: &str = "da9414231de7ad7e71abfb3f79e2fa84a5353bb19b40ae391ca1121c20bc9d8f";

    type Canonical = BTreeMap<u32, u32>;
    type Classes = BTreeMap<u32, Vec<u32>>;

    #[derive(Deserialize)]
    struct Reference {
        go: String,
        unicode: String,
        fold_orbits: Vec<(u32, u32)>,
    }

    #[derive(Debug)]
    struct Oracle {
        canonical: Canonical,
        // C, S, F, T: only the first two grant simple-fold equivalence.
        records: [usize; 4],
    }

    fn representative(canonical: &Canonical, scalar: u32) -> u32 {
        *canonical.get(&scalar).unwrap_or(&scalar)
    }

    fn classes(canonical: &Canonical) -> Classes {
        let mut result = Classes::new();
        for (&scalar, &root) in canonical {
            result.entry(root).or_default().push(scalar);
        }
        result
    }

    fn scalar_token(token: &str) -> Result<u32, String> {
        if !(4..=6).contains(&token.len())
            || !token
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
        {
            return Err(format!("invalid code point token: {token}"));
        }
        let value = u32::from_str_radix(token, 16).map_err(|error| error.to_string())?;
        char::from_u32(value).ok_or_else(|| format!("not a Unicode scalar: {token}"))?;
        Ok(value)
    }

    // This union-find oracle consumes only official C/S edges, never library folds.
    fn root(parent: &Canonical, mut scalar: u32) -> u32 {
        while let Some(&next) = parent.get(&scalar) {
            if next == scalar {
                break;
            }
            scalar = next;
        }
        scalar
    }

    fn parse_case_folding(text: &str) -> Result<Oracle, String> {
        if text.lines().next() != Some("# CaseFolding-16.0.0.txt")
            || text.lines().last() != Some("# EOF")
        {
            return Err("wrong Unicode version/header or missing EOF".into());
        }
        let mut parent = Canonical::new();
        let mut statuses = BTreeMap::<u32, u8>::new();
        let mut records = [0_usize; 4];
        let mut previous = None;
        for (index, line) in text.lines().enumerate() {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let line_number = index.saturating_add(1);
            let invalid = || format!("invalid CaseFolding record at line {line_number}");
            let (data, name) = line.split_once('#').ok_or_else(invalid)?;
            let fields: Vec<_> = data.split(';').map(str::trim).collect();
            if fields.len() != 4 || !fields[3].is_empty() || name.trim().is_empty() {
                return Err(invalid());
            }
            let source = scalar_token(fields[0])?;
            let (slot, bit, status) = match fields[1] {
                "C" => (0, 1, b'C'),
                "S" => (1, 2, b'S'),
                "F" => (2, 4, b'F'),
                "T" => (3, 8, b'T'),
                _ => return Err(invalid()),
            };
            let mapping: Vec<_> = fields[2]
                .split_whitespace()
                .map(scalar_token)
                .collect::<Result<_, _>>()?;
            if (status == b'F' && mapping.len() < 2) || (status != b'F' && mapping.len() != 1) {
                return Err(invalid());
            }
            let seen = statuses.entry(source).or_default();
            if *seen & bit != 0 {
                return Err(format!("duplicate source/status at line {line_number}"));
            }
            *seen |= bit;
            if previous.is_some_and(|p| p >= (source, status)) {
                return Err(format!("unordered record at line {line_number}"));
            }
            previous = Some((source, status));
            records[slot] = records[slot].saturating_add(1);
            if status == b'C' || status == b'S' {
                if source == mapping[0] {
                    return Err(format!("identity C/S record at line {line_number}"));
                }
                let left = root(&parent, source);
                let right = root(&parent, mapping[0]);
                let minimum = left.min(right);
                parent.insert(left, minimum);
                parent.insert(right, minimum);
            }
        }
        // C cannot coexist with F/S; S requires F. T supplements C or F only.
        if statuses
            .values()
            .any(|mask| !matches!(*mask, 1 | 4 | 6 | 9 | 12))
        {
            return Err("invalid status combination for source".into());
        }
        let canonical = parent.keys().map(|&s| (s, root(&parent, s))).collect();
        Ok(Oracle { canonical, records })
    }

    fn pinned_oracle(bytes: &[u8]) -> Result<Oracle, String> {
        if hex::encode(Sha256::digest(bytes)) != OFFICIAL_SHA256 {
            return Err("official Unicode 16 SHA-256 mismatch".into());
        }
        parse_case_folding(from_utf8(bytes).map_err(|error| error.to_string())?)
    }

    fn official_oracle() -> Oracle {
        pinned_oracle(OFFICIAL).expect("pinned official Unicode 16 C/S data")
    }

    fn frozen_go() -> Canonical {
        assert_eq!(hex::encode(Sha256::digest(GO_REFERENCE)), GO_SHA256);
        let reference: Reference =
            serde_json::from_slice(GO_REFERENCE).expect("unchanged pinned Go reference");
        assert_eq!(reference.go, "go1.23.12");
        assert_eq!(reference.unicode, "15.0.0");
        assert_eq!(reference.fold_orbits.len(), 1_454);
        let mut canonical = Canonical::new();
        for (scalar, expected) in reference.fold_orbits {
            char::from_u32(scalar).expect("Go member scalar");
            char::from_u32(expected).expect("Go canonical scalar");
            assert!(
                expected < scalar,
                "Go canonical representative is the minimum"
            );
            assert!(
                canonical.insert(scalar, expected).is_none(),
                "duplicate Go member"
            );
        }
        let roots: BTreeSet<_> = canonical.values().copied().collect();
        for expected in roots {
            assert_eq!(representative(&canonical, expected), expected);
            canonical.insert(expected, expected);
        }
        assert_eq!(canonical.len(), 2_878);
        assert_eq!(classes(&canonical).len(), 1_424);
        canonical
    }

    // Independent literals inspected in the official file, not obtained from the library.
    fn expected_added_classes() -> Vec<Vec<u32>> {
        let mut added = vec![
            vec![0x019b, 0xa7dc],
            vec![0x0264, 0xa7cb],
            vec![0x0390, 0x1fd3],
            vec![0x03b0, 0x1fe3],
            vec![0x1c89, 0x1c8a],
            vec![0xa7cc, 0xa7cd],
            vec![0xa7da, 0xa7db],
            vec![0xfb05, 0xfb06],
        ];
        // The 22 Garay pairs run from 10D50/10D70 through 10D65/10D85.
        added.extend(
            (0x0001_0d50_u32..=0x0001_0d65)
                .map(|upper| vec![upper, upper.checked_add(0x20).expect("Garay lower scalar")]),
        );
        added
    }

    fn folded(c: char) -> u32 {
        unicode_case_mapping::case_folded(c).map_or(u32::from(c), NonZeroU32::get)
    }

    fn assert_library_partition(canonical: &Canonical) {
        let mut actual_classes = BTreeMap::new();
        let mut checked = 0_u32;
        for scalar in 0..=0x0010_ffff {
            let Some(c) = char::from_u32(scalar) else {
                continue;
            };
            let expected = representative(canonical, scalar);
            let actual = folded(c);
            assert_eq!(
                representative(canonical, actual),
                expected,
                "different orbit for {scalar:x}"
            );
            assert_eq!(
                folded(char::from_u32(expected).expect("oracle orbit scalar")),
                actual,
                "split orbit for {scalar:x}"
            );
            if let Some(previous) = actual_classes.insert(actual, expected) {
                assert_eq!(
                    previous, expected,
                    "distinct oracle orbits merged for {scalar:x}"
                );
            }
            checked = checked.saturating_add(1);
        }
        assert_eq!(checked, 1_112_064);
    }

    #[test]
    fn official_unicode16_fixture_has_pinned_identity_and_shape() {
        let oracle = official_oracle();
        assert_eq!(OFFICIAL.len(), 86_092);
        assert_eq!(oracle.records, [1_453, 31, 104, 2]);
        assert_eq!(oracle.canonical.len(), 2_938);
        assert_eq!(classes(&oracle.canonical).len(), 1_454);
        for (bytes, digest) in [
            (
                include_bytes!("fixtures/unicode/ReadMe-16.0.0.txt").as_slice(),
                "14cafa23788d3a20dd21d6b0cdcb8d6dab520781fcd9ad9392f3b88ea607e633",
            ),
            (
                include_bytes!("fixtures/unicode/LICENSE-UNICODE.txt").as_slice(),
                "e7a93b009565cfce55919a381437ac4db883e9da2126fa28b91d12732bc53d96",
            ),
        ] {
            assert_eq!(hex::encode(Sha256::digest(bytes)), digest);
        }
    }

    #[test]
    fn every_unicode_scalar_has_exactly_the_official_unicode16_simple_fold_class() {
        let oracle = official_oracle();
        assert_eq!(unicode_case_mapping::UNICODE_VERSION, (16, 0, 0));
        assert_library_partition(&oracle.canonical);
    }

    #[test]
    fn frozen_go_classes_are_unchanged_and_unicode16_delta_is_exact() {
        let old = frozen_go();
        let new = official_oracle().canonical;
        let old_classes = classes(&old);
        let new_classes = classes(&new);
        assert_eq!(
            1_112_064_usize
                .checked_sub(old.len())
                .expect("old singletons")
                .checked_add(old_classes.len())
                .expect("old class count"),
            1_110_610
        );
        assert_eq!(
            1_112_064_usize
                .checked_sub(new.len())
                .expect("new singletons")
                .checked_add(new_classes.len())
                .expect("new class count"),
            1_110_580
        );
        let old_sets: BTreeSet<_> = old_classes.values().cloned().collect();
        let new_sets: BTreeSet<_> = new_classes.values().cloned().collect();
        let added: Vec<_> = new_sets.difference(&old_sets).cloned().collect();
        let removed: Vec<_> = old_sets.difference(&new_sets).cloned().collect();
        assert_eq!(old_sets.intersection(&new_sets).count(), 1_424);
        assert_eq!(added, expected_added_classes());
        assert_eq!(added.len(), 30);
        assert!(
            removed.is_empty(),
            "historical nontrivial classes changed: {removed:x?}"
        );

        // Include implicit singleton classes when detecting both merges and splits.
        let members: BTreeSet<_> = old.keys().chain(new.keys()).copied().collect();
        let mut old_to_new = BTreeMap::<u32, BTreeSet<u32>>::new();
        let mut new_to_old = BTreeMap::<u32, BTreeSet<u32>>::new();
        let mut changed_members = BTreeSet::new();
        for scalar in members {
            let before = representative(&old, scalar);
            let after = representative(&new, scalar);
            old_to_new.entry(before).or_default().insert(after);
            new_to_old.entry(after).or_default().insert(before);
            let before_class = old_classes
                .get(&before)
                .cloned()
                .unwrap_or_else(|| vec![scalar]);
            let after_class = new_classes
                .get(&after)
                .cloned()
                .unwrap_or_else(|| vec![scalar]);
            if before_class != after_class {
                changed_members.insert(scalar);
            }
        }
        let splits: Vec<_> = old_to_new.values().filter(|set| set.len() > 1).collect();
        let merges: Vec<Vec<_>> = new_to_old
            .values()
            .filter(|set| set.len() > 1)
            .map(|set| set.iter().copied().collect())
            .collect();
        assert!(splits.is_empty(), "old classes split: {splits:x?}");
        assert_eq!(merges, expected_added_classes());
        assert_eq!(merges.len(), 30);
        assert_eq!(changed_members.len(), 60);
        assert_eq!(
            changed_members,
            added.iter().flatten().copied().collect::<BTreeSet<_>>()
        );
        assert_eq!(
            1_112_064_usize.checked_sub(changed_members.len()),
            Some(1_112_004)
        );

        // Augment, rather than skip, the frozen relation with the entire pinned delta.
        let mut qualified_go = old;
        for class in expected_added_classes() {
            for &scalar in &class {
                assert_eq!(representative(&qualified_go, scalar), scalar);
                assert!(qualified_go.insert(scalar, class[0]).is_none());
            }
        }
        assert_eq!(qualified_go, new);
        assert_library_partition(&qualified_go);
    }

    #[test]
    fn unicode16_new_cyrillic_tje_class_is_recognized() {
        let old = frozen_go();
        let new = official_oracle().canonical;
        assert_ne!(representative(&old, 0x1c89), representative(&old, 0x1c8a));
        assert_eq!(representative(&new, 0x1c89), 0x1c89);
        assert_eq!(representative(&new, 0x1c8a), 0x1c89);
        // This is a behavioral red on unicode-case-mapping 0.5, not a version check.
        assert!(simple_fold_eq("\u{1c89}", "\u{1c8a}"));
        assert!(simple_fold_eq("\u{1c8a}", "\u{1c89}"));
    }

    #[test]
    fn oracle_rejects_bad_headers_records_statuses_and_scalars() {
        let valid = "# CaseFolding-16.0.0.txt\n0041; C; 0061; # A\n# EOF\n";
        let parsed = parse_case_folding(valid).unwrap();
        assert_eq!(parsed.records, [1, 0, 0, 0]);
        assert_eq!(
            parsed.canonical,
            Canonical::from([(0x0041, 0x0041), (0x0061, 0x0041)])
        );
        for malformed in [
            valid.replace("16.0.0", "15.0.0"),
            valid.replace("# EOF", ""),
        ] {
            assert_eq!(
                parse_case_folding(&malformed).unwrap_err(),
                "wrong Unicode version/header or missing EOF"
            );
        }
        for records in [
            "0041; C; 0061 # missing final semicolon",
            "0041; C; 0061; extra; # extra field",
            "0041; C; 0061;",
            "0041; C; 0061; #",
            "0041; X; 0061; # unknown status",
            "0041; CC; 0061; # wide status",
            "0041; C; ; # empty mapping",
            "0041; C; 0061 0062; # not simple",
            "0041; S; 0061 0062; # not simple",
            "0041; T; 0061 0062; # not a scalar mapping",
            "0041; F; 0061; # not full",
            "0041; S; 0061; # S without F",
            "0041; T; 0061; # T without default",
            "0041; C; 0061; # A\n0041; C; 0061; # duplicate",
            "0041; F; 0061 0062; # full\n0041; F; 0061 0062; # duplicate F",
            "0041; C; 0061; # common\n0041; F; 0061 0062; # conflict",
            "0041; C; 0061; # common\n0041; S; 0062; # conflict",
            "0042; C; 0062; # B\n0041; C; 0061; # out of order",
            "0041; C; 0041; # identity",
            "041; C; 0061; # short token",
            "00df; F; 0073 0073; # lowercase hex",
            "D800; C; 0061; # surrogate source",
            "0041; C; DFFF; # surrogate target",
            "110000; C; 0061; # out of range source",
            "0041; C; 110000; # out of range target",
            "0041; F; 0061 D800; # invalid ignored full target",
            "0041; T; 110000; # invalid ignored Turkic target",
        ] {
            let text = format!("# CaseFolding-16.0.0.txt\n{records}\n# EOF\n");
            assert!(parse_case_folding(&text).is_err(), "accepted {records}");
        }
    }

    #[test]
    fn mutated_mapping_fails_digest_and_changes_the_independent_relation() {
        let text = from_utf8(OFFICIAL).expect("official UTF-8");
        let before = "1C89; C; 1C8A;";
        assert_eq!(text.matches(before).count(), 1);
        let mutated = text.replacen(before, "1C89; C; 0061;", 1);
        assert!(
            pinned_oracle(mutated.as_bytes())
                .expect_err("digest rejection")
                .contains("SHA-256")
        );
        let original = official_oracle();
        let changed = parse_case_folding(&mutated).expect("structurally valid mutation");
        assert_eq!(original.records, changed.records);
        assert_eq!(
            representative(&original.canonical, 0x1c89),
            representative(&original.canonical, 0x1c8a)
        );
        assert_ne!(
            representative(&changed.canonical, 0x1c89),
            representative(&changed.canonical, 0x1c8a)
        );
        assert_eq!(
            representative(&changed.canonical, 0x1c89),
            representative(&changed.canonical, u32::from('a'))
        );
    }

    #[test]
    fn oracle_excludes_full_turkic_and_normalization_permissions() {
        let oracle = official_oracle();
        for (left, right) in [
            (0x0049, 0x0131),
            (0x0130, 0x0069),
            (0x00df, 0x0073),
            (0xfb05, 0x0073),
            (0x00e9, 0x0065),
        ] {
            assert_ne!(
                representative(&oracle.canonical, left),
                representative(&oracle.canonical, right)
            );
        }
        for class in [
            vec![0x004b, 0x006b, 0x212a],
            vec![0x0053, 0x0073, 0x017f],
            vec![0x03a3, 0x03c2, 0x03c3],
            vec![0x00df, 0x1e9e],
        ] {
            for &scalar in &class {
                assert_eq!(representative(&oracle.canonical, scalar), class[0]);
            }
        }
    }

    #[test]
    fn equality_uses_simple_not_ascii_full_locale_or_normalized_folding() {
        assert!(simple_fold_eq(
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            "abcdefghijklmnopqrstuvwxyz"
        ));
        for left in ["\u{212a}", "K", "k"] {
            for right in ["\u{212a}", "K", "k"] {
                assert!(simple_fold_eq(left, right));
            }
        }
        for class in [["S", "s", "\u{17f}"], ["\u{3a3}", "\u{3c2}", "\u{3c3}"]] {
            for left in class {
                for right in class {
                    assert!(simple_fold_eq(left, right));
                }
            }
        }
        assert!(!simple_fold_eq("\u{df}", "ss"));
        assert!(simple_fold_eq("\u{1e9e}", "\u{df}"));
        assert!(!simple_fold_eq("\u{130}", "i"));
        assert!(!simple_fold_eq("\u{131}", "I"));
        assert!(!simple_fold_eq("\u{e9}", "e\u{301}"));
        assert!(simple_fold_eq("Future\u{212a}", "futurek"));
        assert!(simple_fold_eq("", ""));
        assert!(!simple_fold_eq("k", "kk"));
    }
}
