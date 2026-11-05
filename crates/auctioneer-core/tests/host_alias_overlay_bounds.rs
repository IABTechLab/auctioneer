//! The host `[aliases]` overlay is bounded in entry COUNT and in alias
//! NAME length, and both bounds are enforced at config validation.
//!
//! Neither bound is decoration. `/info/bidders/all` aggregates every
//! registry bidder AND every runtime alias, and the boot envelope's
//! aux-route reservation funds that route as
//! `(registry_bidder_count + max_aliases) x per_bidder_detail_bytes`.
//! Both factors have to be bounded for the product to be one: an
//! unbounded entry count makes the first factor arbitrary, and an
//! unbounded alias name makes `per_bidder_detail_bytes` a function of
//! config rather than a constant. An overlay past either bound would
//! let a store push authorise an aux-route response the boot-time
//! envelope proof never charged for.
//!
//! Enforcement is at CONFIG VALIDATION and it is fail-loud: a rejected
//! overlay surfaces as the malformed-config 500 every other invalid
//! host shape produces. Truncating to the bound instead would serve a
//! silently different alias map than the operator pushed.
//!
//! The existing registry check (`build.rs`'s
//! `validate_host_aliases_against_registry`, and the runtime
//! `validate_alias_bases`) bounds neither of these: it asks only
//! whether each entry's BASE names a bidder this build carries, which
//! 66 valid entries with 65-byte names all do.

#[cfg(test)]
mod tests {
    use auctioneer_bidder::BidderName;
    use auctioneer_config::policy::HostPolicy;
    use auctioneer_config::usersync::validate_catalog_size;
    use auctioneer_core::config::resolve::resolve_host;
    use auctioneer_core::config::{
        AuctioneerAppConfig, MAX_ALIAS_NAME_BYTES, MAX_ALIASES, validate_effective,
        validate_effective_with_catalog, validate_host_alias_overlay,
    };
    use std::collections::BTreeMap;
    use std::fmt::Write as _;
    use validator::Validate as _;

    /// A four-byte UTF-8 character, for the byte-versus-character
    /// cases below. Spelled by escape so the file stays ASCII.
    const FOUR_BYTE_CHAR: &str = "\u{1F600}";

    /// How many entries an overlay carries, or `None` for no
    /// `[aliases]` section at all.
    fn entry_count(cfg: &AuctioneerAppConfig) -> Option<usize> {
        cfg.host.aliases.as_ref().map(BTreeMap::len)
    }

    /// Render an `[aliases]` overlay with `count` entries, each name
    /// `name_bytes` bytes of ASCII.
    ///
    /// ASCII so the byte count and the character count agree here and
    /// the fixture says exactly what it means; the multi-byte cases
    /// below get their own fixtures, where the two deliberately
    /// disagree.
    fn overlay(count: usize, name_bytes: usize) -> String {
        let mut out = String::from("schema_version = 1\n[host.aliases]\n");
        for index in 0..count {
            let suffix = format!("{index:04}");
            let prefix = if name_bytes <= MAX_ALIAS_NAME_BYTES {
                "fake_"
            } else {
                ""
            };
            let stem_len = name_bytes
                .checked_sub(prefix.len().checked_add(suffix.len()).unwrap())
                .expect("the fixture's name length must leave room for the index suffix");
            let name = format!("{prefix}{stem}{suffix}", stem = "a".repeat(stem_len));
            assert_eq!(
                name.len(),
                name_bytes,
                "the fixture must build a name of exactly the requested byte length",
            );
            writeln!(out, "{name} = \"fake_fixture_base\"")
                .expect("writing to a String cannot fail");
        }
        out
    }

    /// One `[aliases]` entry whose NAME is `name`, quoted so arbitrary
    /// UTF-8 survives the round trip through TOML.
    fn single_entry_overlay(name: &str) -> String {
        format!("schema_version = 1\n[host.aliases]\n{name:?} = \"fake_fixture_base\"\n")
    }

    /// The bounds are the spec's figures. Pinned here so a silent
    /// widening of either constant fails a test rather than shipping.
    #[test]
    fn the_bounds_are_the_specified_figures() {
        assert_eq!(MAX_ALIASES, 64, "the spec's `max_aliases` default is 64");
        assert_eq!(
            MAX_ALIAS_NAME_BYTES, 64,
            "the spec's `max_alias_name_bytes` is 64",
        );
    }

    #[test]
    fn effective_catalog_counts_merged_baked_and_runtime_aliases_at_the_boundary() {
        let compiled = auctioneer_config_data::bidders().len();
        let base = &auctioneer_config_data::bidders()
            .first()
            .expect("effective alias validation requires a baked base")
            .name;
        assert!(compiled <= 512);
        // The actual roster can fill the catalog. The separate injected-catalog
        // case pins 511/512/513 with 64 overlay aliases independently of it.
        for total in [compiled, 512, 513] {
            let mut host = auctioneer_config_data::host_default().clone();
            host.aliases.clear();
            let mut config = AuctioneerAppConfig::default();
            let alias_count = total.checked_sub(compiled).unwrap();
            let runtime_count = alias_count.min(64);
            let baked_count = alias_count.checked_sub(runtime_count).unwrap();
            for index in 0..baked_count {
                host.aliases
                    .insert(format!("fake_baked_{index:03}"), base.clone());
            }
            let overrides = (0..runtime_count)
                .map(|index| (format!("fake_runtime_{index:03}"), base.clone()))
                .collect();
            config.host.aliases = Some(overrides);
            assert_eq!(host.aliases.len(), baked_count);
            assert_eq!(config.host.aliases.as_ref().unwrap().len(), runtime_count);
            assert!(
                runtime_count <= 64,
                "the runtime overlay has its own count bound"
            );
            config.validate().unwrap();
            let merged = resolve_host(&config, &host);
            assert_eq!(compiled + merged.aliases.len(), total);
            assert_eq!(
                validate_catalog_size(compiled, merged.aliases.len()).is_ok(),
                total <= 512,
                "the catalog bound counts the inherited map and overlay together",
            );
            if total <= 512 && merged.aliases.len() > 64 {
                assert!(baked_count > 0);
                assert!(
                    validate_host_alias_overlay(&merged.aliases).is_err(),
                    "reapplying the runtime-only validator would wrongly reject this valid merged map"
                );
            }
            let result =
                validate_effective(&config, &host, auctioneer_config_data::account_default())
                    .map_err(|error| error.to_string());
            if total <= 512 {
                assert_eq!(result, Ok(()), "catalog total {total}");
            } else {
                assert_eq!(
                    result,
                    Err("bidder configuration structure invalid".to_owned()),
                    "the canonical validator refuses total catalog overflow with a fixed category",
                );
            }
        }
    }

    #[test]
    fn synthetic_compiled_catalog_and_full_overlay_have_independent_count_bounds() {
        for (compiled_count, total) in [(447_usize, 511_usize), (448, 512), (449, 513)] {
            let names: Vec<_> = (0..compiled_count)
                .map(|index| BidderName::new(format!("fake_base_{index:03}")))
                .collect();
            let config = AuctioneerAppConfig {
                host: HostPolicy {
                    aliases: Some(
                        (0_usize..64)
                            .map(|index| {
                                (format!("fake_alias_{index:03}"), "fake_base_000".to_owned())
                            })
                            .collect(),
                    ),
                    ..HostPolicy::default()
                },
                ..AuctioneerAppConfig::default()
            };
            config.validate().unwrap();
            let mut host = auctioneer_config_data::host_default().clone();
            host.aliases.clear();
            assert_eq!(
                compiled_count + config.host.aliases.as_ref().unwrap().len(),
                total
            );
            let result = validate_effective_with_catalog(
                &config,
                &host,
                auctioneer_config_data::account_default(),
                &names,
            );
            if total <= 512 {
                assert_eq!(result, Ok(()), "catalog total {total}");
            } else {
                assert_eq!(
                    result,
                    Err("user-sync catalog exceeds 512 names".to_owned())
                );
            }
        }
    }

    #[test]
    fn full_compiled_catalog_admits_no_additional_host_alias() {
        let names: Vec<_> = (0_usize..512)
            .map(|index| BidderName::new(format!("fake_base_{index:03}")))
            .collect();
        let mut host = auctioneer_config_data::host_default().clone();
        host.aliases.clear();
        let mut config = AuctioneerAppConfig::default();
        assert_eq!(
            validate_effective_with_catalog(
                &config,
                &host,
                auctioneer_config_data::account_default(),
                &names,
            ),
            Ok(())
        );
        config.host.aliases = Some(BTreeMap::from([(
            "fake_one_alias".to_owned(),
            "fake_base_000".to_owned(),
        )]));
        config.validate().unwrap();
        assert_eq!(
            validate_effective_with_catalog(
                &config,
                &host,
                auctioneer_config_data::account_default(),
                &names,
            ),
            Err("user-sync catalog exceeds 512 names".to_owned())
        );
    }

    /// An overlay AT each bound is accepted: these are bounds, not
    /// targets, and a check that rejected the boundary would make the
    /// documented figure unreachable.
    #[test]
    fn an_overlay_at_both_bounds_is_accepted() {
        let cfg: AuctioneerAppConfig = toml::from_str(&overlay(MAX_ALIASES, MAX_ALIAS_NAME_BYTES))
            .expect("an overlay at both bounds parses");
        assert_eq!(
            entry_count(&cfg),
            Some(MAX_ALIASES),
            "the fixture must actually carry {MAX_ALIASES} entries",
        );
        cfg.validate()
            .expect("an overlay exactly at both bounds is valid config");
    }

    /// One entry past `MAX_ALIASES` is REJECTED, not truncated.
    #[test]
    fn an_overlay_past_the_entry_count_is_rejected() {
        let over = MAX_ALIASES + 1;
        let cfg: AuctioneerAppConfig =
            toml::from_str(&overlay(over, 16)).expect("the over-count overlay parses");
        assert_eq!(
            entry_count(&cfg),
            Some(over),
            "the fixture must carry more than {MAX_ALIASES} entries or this proves nothing",
        );
        let err = cfg.validate().expect_err(
            "an overlay with more than MAX_ALIASES entries must be REJECTED at config \
             validation: fail-loud, never silently truncated",
        );
        let rendered = err.to_string();
        assert!(
            rendered.contains("alias"),
            "the refusal must name the alias overlay so an operator can find it; got: \
             {rendered}",
        );
    }

    /// One BYTE past `MAX_ALIAS_NAME_BYTES` is rejected, with the entry
    /// count well inside its own bound so only the length check can
    /// account for the refusal.
    #[test]
    fn an_overlay_with_an_over_long_alias_name_is_rejected() {
        let cfg: AuctioneerAppConfig = toml::from_str(&overlay(2, MAX_ALIAS_NAME_BYTES + 1))
            .expect("the over-long-name overlay parses");
        let err = cfg.validate().expect_err(
            "an alias name longer than MAX_ALIAS_NAME_BYTES must be rejected at config \
             validation",
        );
        let rendered = err.to_string();
        assert!(
            rendered.contains("alias"),
            "the refusal must name the alias overlay; got: {rendered}",
        );
    }

    /// The length bound is in BYTES, and alias names are arbitrary
    /// UTF-8.
    ///
    /// This is the case a `chars().count()` check gets wrong in the
    /// UNSAFE direction: 33 four-byte characters is 33 chars and 132
    /// bytes, so a character-counting check would ACCEPT a name more
    /// than twice the funded length. The aux-route reservation funds
    /// bytes, so bytes is the only measure that bounds it.
    #[test]
    fn a_multi_byte_name_is_measured_in_bytes_not_characters() {
        let name = FOUR_BYTE_CHAR.repeat(33);
        assert_eq!(name.chars().count(), 33, "33 characters");
        assert_eq!(name.len(), 132, "but 132 bytes");
        assert!(
            name.chars().count() <= MAX_ALIAS_NAME_BYTES && name.len() > MAX_ALIAS_NAME_BYTES,
            "the fixture must be inside the bound by chars and outside it by bytes, or \
             it cannot tell a byte check from a char check",
        );
        let cfg: AuctioneerAppConfig =
            toml::from_str(&single_entry_overlay(&name)).expect("the multi-byte overlay parses");
        cfg.validate().expect_err(
            "a 132-byte alias name must be rejected: the bound is BYTES, and a \
             character-counting check would accept a name twice the funded length",
        );
    }

    /// A multi-byte name INSIDE the byte bound is accepted, and the
    /// check does not panic reaching that verdict.
    ///
    /// A check implemented as `name[..MAX_ALIAS_NAME_BYTES]`, or any
    /// other byte-index slice of a `str`, panics when the index is not
    /// a character boundary. The 62-byte fixture puts a boundary two
    /// bytes inside the last character, so such an implementation would
    /// panic here instead of answering. Comparing lengths cannot.
    #[test]
    fn a_multi_byte_name_inside_the_byte_bound_is_accepted_without_panicking() {
        let aligned = format!("fake_aaa{}", FOUR_BYTE_CHAR.repeat(13));
        assert_eq!(aligned.len(), 60, "60 bytes, inside the 64-byte bound");
        let aligned_cfg: AuctioneerAppConfig =
            toml::from_str(&single_entry_overlay(&aligned)).expect("the multi-byte overlay parses");
        aligned_cfg
            .validate()
            .expect("a 60-byte multi-byte alias name is inside the 64-byte bound");

        // 62 bytes: 15 four-byte characters plus two ASCII. The
        // 64-byte bound now lands two bytes past the end, and a
        // 60-byte prefix slice would land mid-character.
        let straddling = format!("{aligned}ab");
        assert_eq!(straddling.len(), 62);
        let straddling_cfg: AuctioneerAppConfig =
            toml::from_str(&single_entry_overlay(&straddling))
                .expect("the straddling overlay parses");
        straddling_cfg
            .validate()
            .expect("a 62-byte name is inside the bound and deciding so must not panic");
    }

    /// An absent overlay is valid: `None` means "the baked alias map
    /// passes through", which is the shipped default.
    #[test]
    fn an_absent_overlay_is_valid() {
        let cfg = AuctioneerAppConfig::default();
        assert_eq!(
            entry_count(&cfg),
            None,
            "the default config carries no `[aliases]` section at all",
        );
        cfg.validate()
            .expect("no `[aliases]` section at all is valid config");
    }

    /// The bounds are the HOST overlay's. Request-level aliases
    /// (`ext.prebid.aliases`) are a different surface with a different
    /// funding story, and this change must not have narrowed them.
    ///
    /// Asserted by construction: the only field these checks read is
    /// `AuctioneerAppConfig::aliases`, and an empty host overlay validates
    /// regardless of what a request later declares.
    #[test]
    fn the_bounds_do_not_reach_the_request_level_alias_surface() {
        let cfg: AuctioneerAppConfig = toml::from_str("schema_version = 1\n[host.aliases]\n")
            .expect("an empty host overlay parses");
        assert_eq!(
            entry_count(&cfg),
            Some(0),
            "an empty `[aliases]` table is an empty map, not `None`",
        );
        cfg.validate()
            .expect("an empty host overlay is valid however many aliases a request declares");
    }
}
