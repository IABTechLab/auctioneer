//! Exact disjoint coverage of retained owners. Aliases never allocate programs.

struct Charge<'a> {
    owner: &'a str,
    covered: &'a [&'a str],
    bytes: u64,
}

#[derive(Debug, Eq, PartialEq)]
enum Refusal {
    Incomplete,
    Coverage,
    Sharing,
    Arithmetic,
}

fn compose(
    expected: &[(&str, &str)],
    charges: &[Charge<'_>],
    aliases: &[(&str, &str)],
) -> Result<u64, Refusal> {
    if expected.iter().any(|(leaf, owner)| leaf.is_empty() || owner.is_empty())
        || charges.iter().any(|row| row.owner.is_empty() || row.covered.is_empty() || row.bytes == 0)
        || aliases.iter().any(|(alias, owner)| alias.is_empty() || owner.is_empty())
    {
        return Err(Refusal::Coverage);
    }
    Err(Refusal::Incomplete)
}

#[cfg(test)]
mod tests {
    use super::{Charge, Refusal, compose};

    const LEAVES: &[(&str, &str)] = &[
        ("fake_a_regex", "fake_a"),
        ("fake_b_regex", "fake_b"),
        ("fake_shared_hostname", "fake_shared"),
    ];

    fn charges() -> [Charge<'static>; 3] {
        [
            Charge { owner: "fake_a", covered: &["fake_a_regex"], bytes: 3 },
            Charge { owner: "fake_b", covered: &["fake_b_regex"], bytes: 3 },
            Charge { owner: "fake_shared", covered: &["fake_shared_hostname"], bytes: 5 },
        ]
    }

    #[test]
    fn retained_equal_programs_are_distinct_but_aliases_and_shared_format_are_not() {
        assert_eq!(compose(LEAVES, &charges(), &[]), Ok(11));
        for aliases in [
            vec![("fake_alias", "fake_a")],
            vec![("fake_alias", "fake_a"), ("fake_second_alias", "fake_a")],
        ] {
            assert_eq!(compose(LEAVES, &charges(), &aliases), Ok(11));
        }
        for (index, bytes, expected) in [(0, 4, 12), (1, 7, 15), (2, 6, 12)] {
            let mut rows = charges();
            rows[index].bytes = bytes;
            assert_eq!(compose(LEAVES, &rows, &[]), Ok(expected));
        }
    }

    #[test]
    fn retained_every_missing_duplicate_extra_or_misowned_subtree_refuses() {
        for removed in 0..3 {
            let mut rows = charges().into_iter().collect::<Vec<_>>();
            rows.remove(removed);
            assert_eq!(compose(LEAVES, &rows, &[]), Err(Refusal::Coverage));
        }
        let mut rows = charges().into_iter().collect::<Vec<_>>();
        rows.push(Charge { owner: "fake_a", covered: &["fake_a_regex"], bytes: 1 });
        assert_eq!(compose(LEAVES, &rows, &[]), Err(Refusal::Coverage));
        let mut rows = charges();
        rows[0].covered = &["fake_unknown"];
        assert_eq!(compose(LEAVES, &rows, &[]), Err(Refusal::Coverage));
        let mut rows = charges();
        rows[0].owner = "fake_b";
        assert_eq!(compose(LEAVES, &rows, &[]), Err(Refusal::Coverage));
    }

    #[test]
    fn retained_alias_targets_cannot_create_hide_or_recharge_owners() {
        for aliases in [
            vec![("fake_alias", "fake_unknown")],
            vec![("fake_a", "fake_b")],
            vec![("fake_alias", "fake_a"), ("fake_alias", "fake_b")],
        ] {
            assert_eq!(compose(LEAVES, &charges(), &aliases), Err(Refusal::Sharing));
        }
    }

    #[test]
    fn retained_sum_overflow_refuses_with_complete_disjoint_coverage() {
        let mut rows = charges();
        rows[0].bytes = u64::MAX;
        assert_eq!(compose(LEAVES, &rows, &[]), Err(Refusal::Arithmetic));
    }
}
