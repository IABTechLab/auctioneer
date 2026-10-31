// OpenRTB 2.6 typed model — request, response, and shared inventory/device/
// user/regs/source structs. Nested submodules are split by responsibility
// (one struct family per file); the parent `types.rs` here only re-declares
// the children.

pub mod bid_request;
pub mod bid_response;
pub mod device;
pub mod field;
pub mod inventory;
pub mod regs;
pub mod source;
pub mod user;
