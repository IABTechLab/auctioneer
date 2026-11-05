//! Dependency-free bounds shared by runtime configuration and build codegen.

/// Maximum UTF-8 byte length of a bidder or alias identity.
pub const MAX_BIDDER_NAME_BYTES: usize = 64;

/// Maximum UTF-8 bytes in a configured liveness banner.
pub const MAX_STATUS_BANNER_BYTES: usize = 8_192;
