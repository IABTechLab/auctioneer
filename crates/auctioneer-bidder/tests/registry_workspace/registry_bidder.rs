//! Factory wrapper copied only into scratch implementation directories by registry_build.

use crate::Bidder;
use crate::fake_synthetic::{FakeBidder, IdentityEnvelope};
use std::sync::atomic::{AtomicUsize, Ordering};

mod protocol;

static CONSTRUCTORS: AtomicUsize = AtomicUsize::new(0);
static EXECUTIONS: AtomicUsize = AtomicUsize::new(0);

pub(crate) fn create_bidder() -> Box<dyn Bidder> {
    Box::new(FakeBidder {
        identity_envelope: Some(IdentityEnvelope {
            marker: protocol::marker::MARKER,
            module: module_path!(),
            constructor_count: CONSTRUCTORS.fetch_add(1, Ordering::SeqCst) + 1,
            executions: &EXECUTIONS,
        }),
        ..FakeBidder::default()
    })
}
