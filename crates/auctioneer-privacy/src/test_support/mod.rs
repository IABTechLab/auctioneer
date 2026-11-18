//! Test encoders shared by privacy unit tests and downstream test targets.
//!
//! Enable `test-support` only in dev-dependencies. These helpers are not
//! part of the serving privacy API.

#![allow(
    clippy::expect_used,
    reason = "Assertion-based setup is intentional in test-only fixture APIs."
)]

pub mod tc_encode;
pub mod tc_encode_segments;

use std::ops::Deref;

use crate::tcf::{IndexedTcf, TcfString};

/// Retains a decoded TC string with its publisher-restriction index, as
/// authorization does, so allocation guards measure both.
#[must_use]
pub fn retained_tcf(tcf: Box<TcfString>) -> impl Deref<Target = TcfString> {
    IndexedTcf::new(tcf)
}
