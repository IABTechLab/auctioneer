//! Shared test helpers without a process-wide allocator installation.

#![allow(
    clippy::expect_used,
    reason = "Assertion-based setup is intentional in test-only fixture APIs."
)]

pub mod alloc_track;
