//! Shared handler-test harness: install a fixture `AuctioneerAppConfig` on a
//! request so the handler's own config loader reads it.
//!
//! Handler-path tests that need non-default runtime config (GDPR
//! enabled, an `[accounts.<id>]` entry, a host alias) would otherwise see
//! a missing-config error without a bound store. This helper registers an
//! in-memory `ConfigStore` and declares its default entry in the request
//! registry. The handler's `AppConfig::from_request` follows that binding
//! through the existing funded loader. Fixture ids are independent of the
//! production manifest's default store; this introduces no second config path.
//!
//! The body sits inside a `#[cfg(test)]` module because clippy's
//! `allow-*-in-tests` exemptions key off a `cfg(test)` ancestor, and an
//! integration-test file's root module is not one.

// Re-exported ahead of the module it names: a `pub use` placed AFTER a
// `cfg(test)` module reads as a stray item to clippy.
#[cfg(test)]
pub use harness::ctx_with_config;

#[cfg(test)]
mod harness {
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::test_support::config_store::{context, runtime_registry};
    use edgezero_core::context::RequestContext;
    use edgezero_core::http::Request;

    /// Parse fixture TOML into `AuctioneerAppConfig`, wrap it in a
    /// `BlobEnvelope` (whose constructor computes the canonical sha256
    /// the extractor's integrity check verifies), and install it as the
    /// `"auctioneer_runtime"` config store on the request.
    pub fn ctx_with_config(toml_src: &str, mut request: Request) -> RequestContext {
        let cfg: AuctioneerAppConfig = toml::from_str(toml_src).expect("fixture TOML parses");
        let registry = runtime_registry(&cfg);
        request.extensions_mut().insert(registry);
        context(request)
    }
}
