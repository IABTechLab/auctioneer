//! Shared handler-test harness: install a fixture `AuctioneerAppConfig` on a
//! request so the handler's own config loader reads it.
//!
//! Handler-path tests that need non-default runtime config (GDPR
//! enabled, an `[accounts.<id>]` entry, a host alias) would otherwise see
//! only baked defaults, because `resolve_runtime_config` silently
//! degrades to `AuctioneerAppConfig::default()` when no config store is
//! bound. This helper drives the REAL loader — it registers an in-memory
//! `ConfigStore` under the `"auctioneer_runtime"` id that the handler's
//! `from_store(ctx, "auctioneer_runtime", None)` call then reads — rather
//! than introducing a second config path.
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
