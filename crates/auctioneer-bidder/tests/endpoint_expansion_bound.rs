//! URL expansion is bounded by the metadata cap BEFORE it allocates.
//!
//! `META_BIDDER_REGION_BYTES` is 3 328, and it is a construction-time
//! bound: a URL that cannot fit it must not be built. It was being
//! applied at the END — `EndpointBuilder::build` substituted every macro
//! into a fresh `String` per substitution, then `set_url` refused the
//! result. A supported macro carrying 300 KB therefore produced a
//! 300 KB URL, and several copies of it, before the 3 328-byte cap threw
//! them all away.
//!
//! The rows here measure PEAK allocation across the whole expansion, not
//! the size of what survives, because what survives was always correct:
//! nothing over the cap ever reached the pool. What was wrong is what it
//! cost to find that out.

/// Test-scoped counting allocator (per-thread, peak bytes). Delegates
/// every operation to `System`.
pub mod alloc_track {
    #![expect(
        unsafe_code,
        reason = "a GlobalAlloc impl is unavoidably unsafe; test-only \
                  measurement backing the endpoint-expansion bound"
    )]

    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static LIVE: Cell<i64> = const { Cell::new(0) };
        static PEAK: Cell<i64> = const { Cell::new(0) };
    }

    /// Reset both counters, so a measurement starts from zero whatever
    /// the harness allocated before it.
    pub fn reset_on_this_thread() {
        let _live = LIVE.try_with(|cell| cell.set(0));
        let _peak = PEAK.try_with(|cell| cell.set(0));
    }

    /// The HIGHEST live-byte total this thread reached since the last
    /// reset.
    ///
    /// The peak rather than the retained figure, because the defect is a
    /// TRANSIENT: the oversize URL is built, refused and dropped, so a
    /// reading taken afterwards shows nothing at all.
    pub fn peak_on_this_thread() -> i64 {
        PEAK.try_with(Cell::get).unwrap_or(0)
    }

    fn add(bytes: usize) {
        // TLS teardown can make try_with fail; a lost count only relaxes
        // the measurement, never breaks the allocator.
        let _outcome = LIVE.try_with(|cell| {
            let now = cell
                .get()
                .saturating_add(i64::try_from(bytes).unwrap_or(i64::MAX));
            cell.set(now);
            let _peak = PEAK.try_with(|peak| {
                if now > peak.get() {
                    peak.set(now);
                }
            });
        });
    }

    fn sub(bytes: usize) {
        let _outcome = LIVE.try_with(|cell| {
            cell.set(
                cell.get()
                    .saturating_sub(i64::try_from(bytes).unwrap_or(i64::MAX)),
            );
        });
    }

    pub struct CountingAllocator;

    // SAFETY: every operation delegates verbatim to `System`; the
    // counting side effect touches only thread-local counters and never
    // the returned memory.
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            add(layout.size());
            // SAFETY: same layout contract as the caller's.
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc.
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            add(layout.size());
            // SAFETY: same layout contract as the caller's.
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            add(new_size);
            sub(layout.size());
            // SAFETY: ptr/layout come from a matching System alloc;
            // new_size is the caller's requested size.
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }
}

#[global_allocator]
static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
pub use auctioneer_bidder::test_support::fake_bidder;

#[cfg(test)]
mod tests {
    use super::alloc_track::{peak_on_this_thread, reset_on_this_thread};
    use super::fake_bidder::FakeBidder;
    use auctioneer_bidder::{
        Bidder as _, BidderName, BidderRequestInfo, CallBudget, EndpointBuilder,
        EndpointBuilderError, EndpointRejectReason, EndpointValidator, EntryPoint,
        META_BIDDER_REGION_BYTES, META_BYTES_CAPACITY, OutboundBuildError,
        OutboundRequestMetaBuilder, OutboundRequestSink, SinkAbort, SinkLimits,
    };
    use auctioneer_config_data::types::BidderInfo;
    use auctioneer_openrtb::types::bid_request::{Banner, BidRequest, Imp};
    use std::hint::black_box;
    use std::str::from_utf8;
    use std::thread;

    /// Accepts every URL, so nothing but the expansion bound can
    /// refuse the request under measurement.
    struct AcceptAll;

    impl EndpointValidator for AcceptAll {
        fn validate(
            &self,
            _url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }

    /// The macro value the reviewer's case carries: far past the
    /// 3 328-byte region, and a supported macro so nothing refuses it
    /// for being unknown.
    const OVERSIZE_VALUE_BYTES: usize = 300 * 1_024;

    /// How many times the bidder-path fixture repeats its macro, and
    /// the value each repeat expands to. The PRODUCT is what a
    /// check-after-build pays; neither factor alone is over-cap, so
    /// `set_macro`'s per-value bound cannot refuse it first.
    const REPEATS: usize = 256;
    const VALUE_BYTES: usize = 2_048;
    const ENDPOINT_VALUE: &str = match from_utf8(&[b'a'; VALUE_BYTES]) {
        Ok(value) => value,
        Err(_) => panic!("the endpoint fixture is ASCII"),
    };

    fn info_with_account_macro() -> BidderInfo {
        BidderInfo {
            name: "fake_synthetic".into(),
            endpoint: Some("https://hb.example.test/openrtb?acct={{.AccountID}}".into()),
            uses_https: true,
            host_allowlist: None,
            endpoint_macros: vec!["AccountID".into()],
            ..BidderInfo::default()
        }
    }

    /// Peak bytes `call` reached, measured on a THREAD OF ITS OWN.
    ///
    /// The counters are thread-local and a fresh thread starts at zero,
    /// so the reading covers this call and nothing the harness did
    /// around it.
    fn peak_bytes_of<F>(call: F) -> usize
    where
        F: FnOnce() + Send + 'static,
    {
        thread::spawn(move || {
            reset_on_this_thread();
            call();
            usize::try_from(peak_on_this_thread()).unwrap_or(usize::MAX)
        })
        .join()
        .expect("the measurement thread completed")
    }

    /// An oversize macro value is refused before anything derived from
    /// it is allocated.
    ///
    /// The budget is the value's own length plus slack: holding the
    /// caller's `String` is unavoidable — it exists before `set_macro`
    /// is called — but nothing of that size may be DERIVED from it.
    /// Before the fix there were several: the recursive percent-decode
    /// for the traversal scan, the percent-encoded copy, and one whole
    /// expanded URL per substitution.
    #[test]
    fn an_oversize_macro_value_allocates_nothing_derived_from_itself() {
        // The whole pipeline, not just `set_macro`: an implementation
        // that accepts the value and refuses later still paid for what
        // it built, and the peak is what this row is about. The refusal
        // VARIANT is asserted below, after the peak -- an earlier
        // revision asserted it inside the measured closure, so the
        // un-bounded path panicked on the variant and the cost assertion
        // never ran at all.
        let peak = peak_bytes_of(|| {
            let info = info_with_account_macro();
            let value = "a".repeat(OVERSIZE_VALUE_BYTES);
            let mut builder = EndpointBuilder::new(&info).expect("the template is valid");
            if builder.set_macro("AccountID", &value).is_ok() {
                let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
                let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 100);
                let _refused = builder.expand_into(&mut meta);
            }
        });
        // One copy of the input, plus a generous allowance for the
        // fixture's own `BidderInfo` strings and the builder. A second
        // copy of a 300 KiB value would be 300 KiB over this.
        let budget = OVERSIZE_VALUE_BYTES.saturating_mul(3).saturating_div(2);
        assert!(
            peak <= budget,
            "refusing a {OVERSIZE_VALUE_BYTES}-byte macro value peaked at {peak} B \
             against a {budget} B budget. The caller's own String is unavoidable; \
             anything else of that size is a copy derived from a value that cannot \
             fit the {META_BIDDER_REGION_BYTES}-byte bidder region, allocated \
             before the cap was applied",
        );

        // And the refusal carries its own reason, so an operator reading
        // the per-bidder entry learns the value was over the cap rather
        // than that some template failed.
        let info = info_with_account_macro();
        let mut builder = EndpointBuilder::new(&info).expect("the template is valid");
        let outcome = builder.set_macro("AccountID", &"a".repeat(OVERSIZE_VALUE_BYTES));
        assert!(
            matches!(outcome, Err(EndpointBuilderError::MacroValueOverCap { .. })),
            "a value past the bidder region must be refused by set_macro, named as \
             over the cap: {outcome:?}",
        );
    }

    /// And an expansion that fits a template but not the region is
    /// refused before the URL is built.
    ///
    /// This is the half `set_macro`'s own check cannot cover: each
    /// VALUE fits the region, their SUM does not. A template with
    /// several macros near the limit expands past it, and `build` would
    /// allocate one whole URL per substitution to find that out.
    #[test]
    fn an_expansion_over_the_region_is_refused_before_it_is_built() {
        // Each value is comfortably inside the region; three of them
        // together are not.
        let per_value = META_BIDDER_REGION_BYTES.saturating_div(2);
        let info = BidderInfo {
            name: "fake_synthetic".into(),
            endpoint: Some("https://hb.example.test/o?a={{.AccountID}}&p={{.PublisherID}}".into()),
            uses_https: true,
            host_allowlist: None,
            endpoint_macros: vec!["AccountID".into(), "PublisherID".into()],
            ..BidderInfo::default()
        };
        let mut builder = EndpointBuilder::new(&info).expect("the template is valid");
        let value = "a".repeat(per_value);
        builder
            .set_macro("AccountID", &value)
            .expect("one value inside the region is accepted");
        builder
            .set_macro("PublisherID", &value)
            .expect("so is the second");
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 100);
        let outcome = builder.expand_into(&mut meta);
        assert!(
            matches!(outcome, Err(SinkAbort::MetaOverCap { .. })),
            "two values of {per_value} B each expand past the \
             {META_BIDDER_REGION_BYTES} B region and must be refused: {outcome:?}",
        );
        assert_eq!(
            meta.written(),
            0,
            "a refused expansion must leave the byte pool untouched",
        );

        // BEFORE IT IS BUILT, which is this row's title and the half the
        // refusal above does not show: `set_url` refused this case all
        // along. So the same expansion is measured, with a budget below
        // one whole expanded URL -- which `build` allocates per
        // substitution, so the un-projected path pays for several.
        let peak = peak_bytes_of(move || {
            let measured_info = BidderInfo {
                name: "fake_synthetic".into(),
                endpoint: Some(
                    "https://hb.example.test/o?a={{.AccountID}}&p={{.PublisherID}}".into(),
                ),
                uses_https: true,
                host_allowlist: None,
                endpoint_macros: vec!["AccountID".into(), "PublisherID".into()],
                ..BidderInfo::default()
            };
            let mut measured = EndpointBuilder::new(&measured_info).expect("the template is valid");
            let measured_value = "a".repeat(META_BIDDER_REGION_BYTES.saturating_div(2));
            measured
                .set_macro("AccountID", &measured_value)
                .expect("inside");
            measured
                .set_macro("PublisherID", &measured_value)
                .expect("inside");
            let mut measured_pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut measured_meta = OutboundRequestMetaBuilder::new(&mut measured_pool, 100);
            let _refused = measured.expand_into(&mut measured_meta);
        });
        // The two values and the meta pool are unavoidable. One expanded
        // URL on top of them is not, and `build` makes one per
        // substitution.
        let unavoidable = per_value
            .saturating_mul(2)
            .saturating_add(META_BYTES_CAPACITY);
        let budget = unavoidable.saturating_add(META_BIDDER_REGION_BYTES);
        assert!(
            peak <= budget,
            "refusing an expansion of two {per_value} B values peaked at {peak} B \
             against a {budget} B budget ({unavoidable} B of unavoidable inputs plus \
             one region's slack). Building the URL first costs one whole expanded \
             String per substitution before the cap is applied",
        );
    }

    /// The projection does not refuse a URL that FITS.
    ///
    /// The bound is a ceiling, not a tightening: an ordinary expansion
    /// must still reach the pool, and a projection that over-estimated
    /// would turn working bidders into code-5 entries. This is the row
    /// that would fail on an over-eager check, which an
    /// oversize-refusal row alone cannot catch.
    #[test]
    fn an_expansion_that_fits_still_reaches_the_pool() {
        let info = info_with_account_macro();
        let mut builder = EndpointBuilder::new(&info).expect("the template is valid");
        builder
            .set_macro("AccountID", "acct-1234")
            .expect("an ordinary account id is accepted");
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 100);
        builder
            .expand_into(&mut meta)
            .expect("an ordinary expansion fits the region and must reach the pool");
        let sent = meta.finish();
        assert_eq!(
            sent.url(),
            "https://hb.example.test/openrtb?acct=acct-1234",
            "the expanded URL must be written verbatim; the bound must not alter it",
        );
    }

    #[test]
    fn allocator_control_observes_a_real_allocation_independently_of_expansion() {
        let peak = peak_bytes_of(|| {
            let allocation = black_box(vec![7_u8; 8192]);
            drop(allocation);
        });
        assert!(
            peak >= 8192,
            "an inactive allocator cannot prove zero-allocation expansion"
        );
    }

    #[test]
    fn borrowed_endpoint_bindings_allocate_nothing_at_or_above_the_output_boundary() {
        let prefix = "https://hb.example.test/";
        let template = "https://hb.example.test/{{.SyntheticToken}}";
        let info = BidderRequestInfo::new(
            BidderName::new("fake_synthetic_alias"),
            BidderName::new("fake_synthetic"),
            "account",
            None,
            template,
            &["SyntheticToken"],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        let fitting = META_BIDDER_REGION_BYTES - 4 - prefix.len();
        for width in [1, fitting - 1, fitting, fitting + 1, OVERSIZE_VALUE_BYTES] {
            let input = "a".repeat(width);
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let pointer = pool.as_ptr();
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            reset_on_this_thread();
            let pairs = [("SyntheticToken", input.as_str())];
            let result = EndpointBuilder::from_request(&info, &pairs).expand_into(&mut meta);
            let peak = peak_on_this_thread();
            assert_eq!(peak, 0, "input width {width}");
            if width <= fitting {
                result.unwrap();
                assert_eq!(meta.written(), 4 + prefix.len() + width);
            } else {
                assert_eq!(
                    result,
                    Err(SinkAbort::MetaOverCap {
                        limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap()
                    })
                );
                assert_eq!(meta.written(), 4);
            }
            drop(meta);
            assert_eq!(pool.as_ptr(), pointer);
            assert_eq!(pool.capacity(), META_BYTES_CAPACITY);
        }
    }

    #[test]
    fn oversized_appended_query_components_allocate_nothing_and_roll_back() {
        // Both input strings and the pool exist before the allocator reset.
        // A serialize-then-check implementation would allocate an encoded
        // copy proportional to these escaping-heavy inputs before refusing.
        let oversized = " &+=%/?#".repeat(OVERSIZE_VALUE_BYTES.saturating_div(8));
        let info = BidderInfo {
            endpoint: Some("https://hb.example.test/o?existing=1".into()),
            ..BidderInfo::default()
        };
        for (label, name, value) in [
            ("oversized name", oversized.as_str(), "v"),
            ("oversized value", "k", oversized.as_str()),
        ] {
            let builder = EndpointBuilder::new(&info).unwrap();
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 100);
            meta.set_method("POST").unwrap();
            meta.header("x-keep", "yes").unwrap();
            let before = meta.written();
            // A successful first pair must roll back too, not just the
            // component whose encoding eventually crosses the cap.
            let pairs = [("first", "a b"), (name, value)];

            reset_on_this_thread();
            let outcome = builder.expand_with_query_into(&mut meta, &pairs);
            let peak = peak_on_this_thread();

            assert_eq!(peak, 0, "{label}: query expansion must not allocate");
            assert_eq!(
                outcome,
                Err(SinkAbort::MetaOverCap {
                    limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap(),
                }),
                "{label}: refusal must be attributed to the metadata cap",
            );
            assert_eq!(meta.written(), before, "{label}: the entire URL rolls back");
            EndpointBuilder::new(&info)
                .unwrap()
                .expand_with_query_into(&mut meta, &[("retry", "a b+&")])
                .unwrap();
            let sent = meta.finish();
            assert_eq!(sent.method(), "POST");
            assert_eq!(sent.headers().collect::<Vec<_>>(), [("x-keep", "yes")]);
            assert_eq!(
                sent.url(),
                "https://hb.example.test/o?existing=1&retry=a+b%2B%26",
                "{label}: refused query bytes must not survive the retry",
            );
        }
    }

    #[test]
    fn appended_query_encoding_is_exact_at_the_metadata_boundary() {
        let info = BidderInfo {
            endpoint: Some("https://hb.example.test/o?existing=1".into()),
            ..BidderInfo::default()
        };
        let encoded_prefix = format!("{}&n+%2B%26=", info.endpoint.as_deref().unwrap());
        for (target, admitted) in [
            (META_BIDDER_REGION_BYTES.saturating_sub(1), true),
            (META_BIDDER_REGION_BYTES, true),
            (META_BIDDER_REGION_BYTES.saturating_add(1), false),
        ] {
            let builder = EndpointBuilder::new(&info).unwrap();
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 100);
            meta.set_method("POST").unwrap();
            meta.header("x-keep", "yes").unwrap();
            let before = meta.written();
            let encoded_len = target
                .checked_sub(before)
                .and_then(|remaining| remaining.checked_sub(encoded_prefix.len()))
                .unwrap();
            // '&' expands to three bytes; spaces expand to one '+'. The
            // remainder makes adjacent encoded lengths distinct even when
            // the final write would otherwise be a three-byte escape.
            let escapes = encoded_len.saturating_div(3);
            let spaces = encoded_len.checked_rem(3).unwrap();
            let value = format!("{}{}", "&".repeat(escapes), " ".repeat(spaces));
            let expected = format!(
                "{encoded_prefix}{}{}",
                "%26".repeat(escapes),
                "+".repeat(spaces)
            );
            let pairs = [("n +&", value.as_str())];
            assert_eq!(before.saturating_add(expected.len()), target);

            reset_on_this_thread();
            let outcome = builder.expand_with_query_into(&mut meta, &pairs);
            let peak = peak_on_this_thread();

            assert_eq!(peak, 0, "{target} encoded metadata bytes must not allocate");
            if admitted {
                outcome.unwrap();
                assert_eq!(meta.written(), target);
                let sent = meta.finish();
                assert_eq!(sent.url(), expected);
                assert_eq!(sent.method(), "POST");
                assert_eq!(sent.headers().collect::<Vec<_>>(), [("x-keep", "yes")]);
            } else {
                assert_eq!(
                    outcome,
                    Err(SinkAbort::MetaOverCap {
                        limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap(),
                    }),
                );
                assert_eq!(meta.written(), before, "an over-cap URL rolls back in full");
                EndpointBuilder::new(&info)
                    .unwrap()
                    .expand_with_query_into(&mut meta, &[("retry", "ok")])
                    .unwrap();
                let sent = meta.finish();
                assert_eq!(sent.url(), "https://hb.example.test/o?existing=1&retry=ok");
                assert_eq!(sent.method(), "POST");
                assert_eq!(sent.headers().collect::<Vec<_>>(), [("x-keep", "yes")]);
            }
        }
    }

    #[test]
    fn query_replacement_uses_only_the_existing_pool_at_the_encoded_boundary() {
        let info = BidderInfo {
            endpoint: Some("https://hb.example.test/o?token%5Fid=1&keep=a%2Bb&token_id=2".into()),
            ..BidderInfo::default()
        };
        let prefix = "https://hb.example.test/o?keep=a%2Bb&token_id=";
        for target in [
            META_BIDDER_REGION_BYTES - 1,
            META_BIDDER_REGION_BYTES,
            META_BIDDER_REGION_BYTES + 1,
        ] {
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let pointer = pool.as_ptr();
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            meta.header("x-keep", "yes").unwrap();
            meta.imp_id("covered").unwrap();
            let before = meta.written();
            let encoded_len = target - before - prefix.len();
            let escapes = encoded_len.saturating_div(3);
            let spaces = encoded_len.checked_rem(3).unwrap();
            let value = format!("{}{}", "&".repeat(escapes), " ".repeat(spaces));
            let expected = format!("{prefix}{}{}", "%26".repeat(escapes), "+".repeat(spaces));
            let builder = EndpointBuilder::new(&info).unwrap();

            reset_on_this_thread();
            let result = builder.expand_with_query_set_into(&mut meta, "token_id", &value);
            let peak = peak_on_this_thread();

            assert_eq!(peak, 0, "target {target}");
            let sent = if target <= META_BIDDER_REGION_BYTES {
                result.unwrap();
                assert_eq!(meta.written(), target);
                let sent = meta.finish();
                assert_eq!(sent.url(), expected);
                sent
            } else {
                assert_eq!(
                    result,
                    Err(SinkAbort::MetaOverCap {
                        limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap(),
                    })
                );
                assert_eq!(meta.written(), before);
                meta.set_url("https://recovery.example.test/").unwrap();
                let sent = meta.finish();
                assert_eq!(sent.url(), "https://recovery.example.test/");
                sent
            };
            // finish transfers the pool into sent, leaving the caller's Vec
            // empty. Compare the URL's backing pointer with the original pool.
            assert_eq!(sent.url().as_ptr(), pointer.wrapping_add(before));
            assert_eq!(sent.method(), "POST");
            assert_eq!(sent.headers().collect::<Vec<_>>(), [("x-keep", "yes")]);
            assert_eq!(sent.covered_imp_ids().collect::<Vec<_>>(), ["covered"]);
        }
    }

    #[test]
    fn query_replacement_bounds_input_and_each_new_component_before_allocating() {
        let oversized = " &+=%/?#".repeat(OVERSIZE_VALUE_BYTES.saturating_div(8));
        let large_url = format!(
            "https://hb.example.test/o?token_id={}",
            "x".repeat(OVERSIZE_VALUE_BYTES)
        );
        let small_url = "https://hb.example.test/o?token_id=old&keep=1";
        for (label, endpoint, key, value) in [
            ("original URL", large_url.as_str(), "token_id", "1"),
            ("replacement key", small_url, oversized.as_str(), "1"),
            (
                "replacement value",
                small_url,
                "token_id",
                oversized.as_str(),
            ),
        ] {
            let info = BidderInfo {
                endpoint: Some(endpoint.into()),
                ..BidderInfo::default()
            };
            let builder = EndpointBuilder::new(&info).unwrap();
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();

            reset_on_this_thread();
            let result = builder.expand_with_query_set_into(&mut meta, key, value);
            let peak = peak_on_this_thread();

            assert_eq!(peak, 0, "{label}");
            assert_eq!(
                result,
                Err(SinkAbort::MetaOverCap {
                    limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap(),
                })
            );
            assert_eq!(meta.written(), 4, "{label}");
            meta.set_url("https://recovery.example.test/").unwrap();
            assert_eq!(meta.finish().url(), "https://recovery.example.test/");
        }
    }

    #[test]
    fn query_replacement_requires_the_original_url_to_fit_even_when_the_result_is_small() {
        let prefix = "https://hb.example.test/o?token_id=";
        let fitting = META_BIDDER_REGION_BYTES - 4 - prefix.len();
        for width in [fitting - 1, fitting, fitting + 1] {
            let info = BidderInfo {
                endpoint: Some(format!("{prefix}{}", "x".repeat(width))),
                ..BidderInfo::default()
            };
            let builder = EndpointBuilder::new(&info).unwrap();
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();

            reset_on_this_thread();
            let result = builder.expand_with_query_set_into(&mut meta, "token_id", "1");
            let peak = peak_on_this_thread();

            assert_eq!(peak, 0, "original width {width}");
            if width <= fitting {
                result.unwrap();
                assert_eq!(meta.finish().url(), format!("{prefix}1"));
            } else {
                assert_eq!(
                    result,
                    Err(SinkAbort::MetaOverCap {
                        limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap(),
                    })
                );
                assert_eq!(meta.written(), 4);
            }
        }
    }

    /// The BIDDER path is bounded too, not just `expand_into`.
    ///
    /// The original expansion rows call `expand_into` directly, so
    /// all of them passed while the function the bidders actually
    /// reach — `expand_outbound_url` — went through `build()` and
    /// allocated the whole expansion before `set_url` rejected it. A
    /// template repeating one macro allocated 320,704 bytes against
    /// the 3,328-byte region that way.
    ///
    /// So this row goes through `Bidder::make_requests` using the shared
    /// fake and measures ALLOCATION rather
    /// than the refusal: both orders refuse the template, and a guard
    /// asserting only on the `Err` passed against the defect.
    #[test]
    fn the_bidder_entry_point_refuses_before_allocating_the_expansion() {
        let peak = peak_bytes_of(|| {
            // A value that FITS the region on its own, repeated past
            // it. `str::replace` returns a fresh buffer per
            // substitution, so `build()` pays REPEATS x the growing
            // string while `expanded_len` refuses after allocating
            // none of it. A single oversize substitution does not
            // distinguish the two orders: measured, both peak at
            // 312,104 B, because the caller's own string dominates.
            let account = "a".repeat(VALUE_BYTES);
            let macros: &[&str] = &["AccountID"];
            let mut template = String::from("https://hb.example.test/x?");
            for index in 0..REPEATS {
                use std::fmt::Write as _;
                let _written = write!(template, "p{index}={{{{.AccountID}}}}&");
            }
            let info = BidderRequestInfo::new(
                BidderName::new("fake_synthetic"),
                BidderName::new("fake_synthetic"),
                &account,
                None,
                &template,
                macros,
                None,
                None,
                EntryPoint::Auction,
                &[],
            );
            let budget = CallBudget::new(4);
            let validator = AcceptAll;
            let mut sink =
                OutboundRequestSink::new(SinkLimits::new(0x0004_0000, 100), &budget, &validator);
            let request = BidRequest {
                id: "synthetic-request".into(),
                imp: vec![Imp {
                    id: "covered".into(),
                    banner: Some(Banner::default()),
                    ..Imp::default()
                }],
                ..BidRequest::default()
            };
            // The outcome is asserted OUTSIDE the measured closure: an
            // assertion in here would panic on the unbounded path
            // before the peak was read, exactly as the sibling row
            // above records.
            let bidder = FakeBidder {
                endpoint_values: &[("AccountID", ENDPOINT_VALUE)],
                ..FakeBidder::default()
            };
            let _outcome = bidder.make_requests(&request, &info, &mut sink);
        });

        // One copy of the caller's own account string is unavoidable —
        // it exists before the builder is constructed. Anything of
        // that size again is an expansion allocated before the cap
        // was applied.
        // The projected expansion is REPEATS x VALUE_BYTES. A bound
        // that refuses first allocates none of it; `build()` allocates
        // a fresh buffer per substitution and so approaches the
        // product. The budget sits well below the product and well
        // above the fixture's own strings.
        let projected = REPEATS.saturating_mul(VALUE_BYTES);
        let budget = projected.saturating_div(4);
        assert!(
            peak <= budget,
            "the bidder entry point peaked at {peak} B refusing a template that \
             expands to about {projected} B against the \
             {META_BIDDER_REGION_BYTES} B bidder region, over a {budget} B \
             budget. A bound that projects the length first allocates none of \
             the expansion",
        );

        // Outside the allocation window, prove the bidder reached endpoint
        // construction and that the same value can produce a real request.
        for repeats in [1, REPEATS] {
            assert_bidder_endpoint_outcome(repeats);
        }
    }

    fn assert_bidder_endpoint_outcome(repeats: usize) {
        let template = format!(
            "https://hb.example.test/x?{}",
            "p={{.AccountID}}&".repeat(repeats)
        );
        let info = BidderRequestInfo::new(
            BidderName::new("fake_synthetic"),
            BidderName::new("fake_synthetic"),
            ENDPOINT_VALUE,
            None,
            &template,
            &["AccountID"],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        let request = BidRequest {
            id: "synthetic-request".into(),
            imp: vec![Imp {
                id: "covered".into(),
                banner: Some(Banner::default()),
                ..Imp::default()
            }],
            ..BidRequest::default()
        };
        let bidder = FakeBidder {
            endpoint_values: &[("AccountID", ENDPOINT_VALUE)],
            ..FakeBidder::default()
        };
        let calls = CallBudget::new(1);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(4096, 1), &calls, &AcceptAll);
        bidder.make_requests(&request, &info, &mut sink).unwrap();
        let outcome = sink.into_outcome();
        if repeats == 1 {
            assert!(outcome.errors.is_empty());
            assert_eq!(outcome.accepted.len(), 1);
            let sent = &outcome.accepted[0];
            assert_eq!(sent.method(), "POST");
            assert_eq!(
                sent.url(),
                format!("https://hb.example.test/x?p={ENDPOINT_VALUE}&")
            );
            assert_eq!(sent.covered_imp_ids().collect::<Vec<_>>(), ["covered"]);
            let body: serde_json::Value = serde_json::from_slice(sent.body()).unwrap();
            assert_eq!(body["id"], "synthetic-request");
            assert_eq!(body["imp"][0]["id"], "covered");
            assert_eq!(bidder.written_bodies(), 1);
            assert_eq!(calls.remaining(), 0);
        } else {
            assert!(outcome.accepted.is_empty());
            assert_eq!(
                outcome.errors,
                [OutboundBuildError::MetaOverCap {
                    limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap(),
                }]
            );
            assert_eq!(bidder.written_bodies(), 0);
            assert_eq!(calls.remaining(), 1);
        }
    }

    /// An expansion right at the region boundary is admitted, and one
    /// byte past it is refused.
    ///
    /// The boundary rather than a round number, because an off-by-one
    /// in the projection is the failure an oversize row cannot see: a
    /// projection one byte pessimistic refuses a URL that fits, and one
    /// byte optimistic allocates one it cannot hold.
    #[test]
    fn the_projection_is_exact_at_the_region_boundary() {
        let prefix = "https://hb.example.test/o?a=";
        let template = format!("{prefix}{{{{.AccountID}}}}");
        // The expansion is `prefix` plus the value, so the largest
        // admissible value is the region less the prefix.
        let fitting = META_BIDDER_REGION_BYTES.saturating_sub(prefix.len());
        let info = BidderInfo {
            name: "fake_synthetic".into(),
            endpoint: Some(template),
            uses_https: true,
            host_allowlist: None,
            endpoint_macros: vec!["AccountID".into()],
            ..BidderInfo::default()
        };
        for (label, len, admitted) in [
            ("exactly at the region", fitting, true),
            ("one byte past it", fitting.saturating_add(1), false),
        ] {
            let mut builder = EndpointBuilder::new(&info).expect("the template is valid");
            // `a` percent-encodes to itself, so the encoded length is
            // the input length and the arithmetic is the plain one.
            builder
                .set_macro("AccountID", &"a".repeat(len))
                .expect("every value here is inside the region");
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 100);
            let outcome = builder.expand_into(&mut meta);
            assert_eq!(
                outcome.is_ok(),
                admitted,
                "{label}: a {len}-byte value expands to \
                 {} B against a {META_BIDDER_REGION_BYTES} B region, so it must \
                 {} -- got {outcome:?}",
                len.saturating_add(prefix.len()),
                if admitted {
                    "be admitted"
                } else {
                    "be refused"
                },
            );
        }
    }
}
