//! Isolated generated-App acceptance: this target alone owns its boot gates,
//! native schema initialization, registry, logger and profile globals.
//! Run with conformance-suite, both with and without insecure-bidders.
//! These portable test reservations do not certify a provider's native profile.

#[cfg(test)]
#[cfg(all(not(target_arch = "wasm32"), feature = "conformance-suite"))]
mod fanout_limits;

#[cfg(test)]
#[cfg(all(not(target_arch = "wasm32"), feature = "conformance-suite"))]
mod native_bidder_onboarding {
    mod cases;
    mod fixtures;
    mod harness;
    mod privacy;
    mod sync;
}

#[cfg(test)]
#[cfg(all(not(target_arch = "wasm32"), not(feature = "conformance-suite")))]
#[test]
fn native_onboarding_requires_the_explicit_conformance_feature() {
    panic!(
        "native_bidder_onboarding requires --features conformance-suite; zero dispatched tests are not acceptance"
    );
}
