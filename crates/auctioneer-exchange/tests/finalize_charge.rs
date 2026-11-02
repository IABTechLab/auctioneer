//! Measure the live gzip finalizer's scratch independently of its two bodies.

#[cfg(test)]
use auctioneer_runtime::test_support::alloc_track;

#[global_allocator]
static ALLOCATOR: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

#[cfg(test)]
mod tests {
    use super::alloc_track;
    use auctioneer_bidder::{
        BidderName, BidderRequestInfo, CallBudget, Compression, EndpointRejectReason,
        EndpointValidator, EntryPoint, OutboundRequestSink, SinkLimits, TransportFinalize,
        finalize_for_transport,
    };
    use auctioneer_runtime::budget::OUTBOUND_FINALIZE_FIXED_BYTES;
    use std::io::{self, Write as _};
    use std::iter::repeat_with;

    struct Accept;
    impl EndpointValidator for Accept {
        fn validate(
            &self,
            _: &url::Url,
            _: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }

    #[test]
    fn gzip_scratch_and_output_peak_fit_the_separate_charge() {
        let info = BidderRequestInfo::new(
            BidderName::new("fake_bb"),
            BidderName::new("fake_bb"),
            "a",
            None,
            "https://example.test/bid",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        for size in [0, 1, 0x0004_0000] {
            let mut state = 13_u64;
            let input: Vec<u8> = repeat_with(|| {
                state ^= state << 13_u8;
                state ^= state >> 7_u8;
                state ^= state << 17_u8;
                u8::try_from(state & 0xff).unwrap()
            })
            .take(size)
            .collect();
            for cap in [1, 0x0004_0000, 300_000] {
                let budget = CallBudget::new(1);
                let mut sink = OutboundRequestSink::new(SinkLimits::new(size, 1), &budget, &Accept);
                sink.push_request(
                    &info,
                    |meta| {
                        meta.set_method("POST")?;
                        meta.set_url("https://example.test/bid")
                    },
                    |writer| writer.write_all(&input),
                )
                .unwrap();
                let request = sink.into_outcome().accepted.pop().unwrap();
                let before = alloc_track::reset_peak();
                let result = finalize_for_transport(
                    request,
                    &TransportFinalize {
                        gpc: Some("1"),
                        compression: Some(Compression::Gzip),
                        max_encoded_bytes: cap,
                    },
                );
                let peak = alloc_track::peak_since(before);
                let scratch = peak.saturating_sub(cap);
                let margined = scratch.checked_mul(6).unwrap().div_ceil(5);
                writeln!(
                    io::stderr(),
                    "gzip size={size} cap={cap} peak={peak} scratch_margin={margined}"
                )
                .unwrap();
                assert!(margined <= OUTBOUND_FINALIZE_FIXED_BYTES);
                drop(result);
            }
        }
    }
}
