//! Response-owned admission for decoding a delivered body once.
//!
//! Standard and candidate decoding share the same nonrenewable receipt.
//! Candidate targets and policies are closed here; no caller-supplied DTO or
//! borrowed source span can escape the sealed transport response.

use crate::{BidderError, BidderErrorKind};
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct DecodeReceipt {
    started: AtomicBool,
}

impl DecodeReceipt {
    pub(crate) const fn new() -> Self {
        Self {
            started: AtomicBool::new(false),
        }
    }

    pub(crate) fn permits_limit_attachment(&mut self) -> bool {
        // Exclusive response ownership prevents a claim racing with attachment.
        !*self.started.get_mut()
    }

    pub(crate) fn claim(&self) -> Result<(), BidderError> {
        // Only ownership of this bit is shared; no decoded payload is published.
        if self.started.swap(true, Ordering::Relaxed) {
            let mut error =
                BidderError::bad_server_response("bidder response exceeded the decode budget");
            error.kind = BidderErrorKind::ResponseParse;
            return Err(error);
        }
        Ok(())
    }
}

mod session;

pub(crate) use session::{CandidatePolicy, decode_source_with_policy};

#[cfg(test)]
mod tests {
    use super::session::CandidateFailure;
    use std::mem::{align_of, size_of};

    #[test]
    fn proprietary_candidate_failure_record_is_one_byte_and_byte_aligned() {
        assert_eq!(size_of::<CandidateFailure>(), 1);
        assert_eq!(align_of::<CandidateFailure>(), 1);
    }
}
