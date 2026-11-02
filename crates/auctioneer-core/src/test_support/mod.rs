//! Fixtures shared by core tests and opt-in conformance harnesses.

#![allow(
    clippy::expect_used,
    reason = "Assertion-based setup is intentional in test-only fixture APIs."
)]

pub mod bidder_fixture;
pub mod boot_parity_fixture;
pub mod config_store;
pub mod live_catalog;
pub mod outbound_request;
pub mod usersync_keys;
