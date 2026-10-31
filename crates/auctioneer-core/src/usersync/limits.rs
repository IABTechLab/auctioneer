//! Per-call resource ceilings for cookie parsing and encoding.

#[derive(Clone, Copy, Debug)]
pub struct CookieLimits {
    pub max_header_count: usize,
    pub max_header_bytes: usize,
    pub max_encoded_bytes: usize,
    pub max_decoded_bytes: usize,
    pub max_json_nodes: usize,
}

impl Default for CookieLimits {
    fn default() -> Self {
        Self {
            max_header_count: 64,
            max_header_bytes: 32 * 1024,
            max_encoded_bytes: 32 * 1024,
            max_decoded_bytes: 24 * 1024,
            max_json_nodes: 4_096,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("cookie resource limit exceeded")]
pub struct CookieLimitError;
