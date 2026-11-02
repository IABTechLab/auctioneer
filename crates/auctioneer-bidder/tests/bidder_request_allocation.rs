//! Measure bidder request construction after the caller's trees exist.
//! Tiny-cap rejection must not first clone those trees outside the capped writer.
//! Fixed-cap, cross-size comparisons distinguish borrowed serialization from
//! caller-tree copies without pinning allocator totals to one host's layout.

#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
mod allocation;
use allocation as alloc_track;

#[cfg(test)]
mod response_allocation;

#[cfg(test)]
#[global_allocator]
static COUNTING_ALLOC: alloc_track::CountingAllocator = alloc_track::CountingAllocator;

include!(concat!(
    env!("OUT_DIR"),
    "/bidder_allocation_conformance.rs"
));
