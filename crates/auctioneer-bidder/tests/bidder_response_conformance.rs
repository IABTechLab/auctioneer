//! Bidder-owned sealed-decoder and node-budget conformance, discovered without a roster.

#![cfg(not(target_arch = "wasm32"))]

mod response_budget;

include!(concat!(env!("OUT_DIR"), "/bidder_response_conformance.rs"));
