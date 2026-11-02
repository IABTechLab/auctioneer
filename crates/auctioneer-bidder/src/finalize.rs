//! The sole post-construction transform, before metadata is shared.

use crate::{BidderHttpRequest, CappedWriter, OutboundRequestMeta};
use flate2::{Compression as GzipCompression, write::GzEncoder};
use std::io::Write as _;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    Gzip,
}

pub struct TransportFinalize<'a> {
    pub gpc: Option<&'a str>,
    pub compression: Option<Compression>,
    pub max_encoded_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FinalizeError {
    OversizeEncoded { limit: u64 },
    ReserveOverflow,
}

/// Consume a sink-built request and retain its metadata for response processing.
///
/// Metadata is shared only AFTER the final appends. Even a refused encoded
/// body returns the same metadata pool, so its positional error needs no copy.
pub fn finalize_for_transport(
    request: BidderHttpRequest,
    opts: &TransportFinalize<'_>,
) -> (
    Arc<OutboundRequestMeta>,
    Result<BidderHttpRequest, FinalizeError>,
) {
    request.finalize(opts)
}

pub(crate) fn finalize(
    meta: &mut OutboundRequestMeta,
    body: &mut Vec<u8>,
    opts: &TransportFinalize<'_>,
) -> Result<(), FinalizeError> {
    // A previously finalized request is already shared. Never append twice or
    // copy the pool to regain mutable access.
    if meta.is_finalized() {
        return Err(FinalizeError::ReserveOverflow);
    }
    let limit = opts.max_encoded_bytes;
    let cap = usize::try_from(limit).map_err(|_conversion_error| FinalizeError::ReserveOverflow)?;
    if opts.compression != Some(Compression::Gzip) && body.len() > cap {
        return Err(FinalizeError::OversizeEncoded { limit });
    }
    if opts.compression == Some(Compression::Gzip) {
        let mut encoded = Vec::new();
        encoded
            .try_reserve_exact(cap)
            .map_err(|_reserve_error| FinalizeError::ReserveOverflow)?;
        let writer = CappedWriter::new(&mut encoded, cap)
            .map_err(|_capacity_error| FinalizeError::ReserveOverflow)?;
        let mut compressor = GzEncoder::new(writer, GzipCompression::default());
        compressor
            .write_all(body)
            .and_then(|()| compressor.finish().map(|_writer| ()))
            .map_err(|_encoding_error| FinalizeError::OversizeEncoded { limit })?;
        *body = encoded;
        meta.append_exchange_header("Content-Encoding", "gzip")?;
    }
    if opts.gpc == Some("1") {
        meta.append_exchange_header("Sec-GPC", "1")?;
    }
    meta.mark_finalized();
    Ok(())
}
