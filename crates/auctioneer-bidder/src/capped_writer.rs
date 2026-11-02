//! `CappedWriter` — a bounded append target over a pre-allocated `Vec`.
//!
//! The writer never grows its backing buffer. The owner allocates
//! `Vec::with_capacity(cap)` (the sink for the raw body, the finalizer
//! for the encoded body) and the writer refuses every byte past `cap`,
//! so the allocation bound the isolate budget charges is an enforced
//! invariant rather than a convention.

use std::io;

/// Failure channel of every bounded body write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyWriteError {
    /// The write would have crossed `cap`. Nothing past the cap is
    /// appended; the backing buffer is left exactly as it was.
    OverCap { limit: u64 },
    /// `serde_json` failed for a reason that is not the cap — a
    /// non-string map key, a `Serialize` impl that errored, etc.
    Serialize,
    /// The backing buffer could not hold `cap` bytes without
    /// reallocating, so no writer may exist over it.
    UnderCapacity { capacity: usize, cap: usize },
}

/// A bounded writer over a caller-owned, pre-allocated byte buffer.
pub struct CappedWriter<'a> {
    buf: &'a mut Vec<u8>,
    cap: usize,
    /// `cap` in the width [`BodyWriteError::OverCap`] reports, converted
    /// once at construction so no write path does a lossy cast.
    cap_limit: u64,
    /// Set when an `io::Write` call was refused for crossing `cap`.
    /// `write_json` reads it to tell an over-cap failure apart from a
    /// genuine serializer failure, because `serde_json` erases the
    /// `io::Error` kind behind its own `Error` type.
    over_cap: bool,
}

impl<'a> CappedWriter<'a> {
    /// Wrap a pre-allocated buffer with a hard `cap` on appended bytes.
    ///
    /// `buf` is pre-allocated by the OWNER with `Vec::with_capacity(cap)`;
    /// the writer never grows it.
    ///
    /// # Errors
    /// [`BodyWriteError::UnderCapacity`] when `buf.capacity() < cap`. A
    /// buffer whose capacity is below `cap` would reallocate mid-write
    /// and break the allocation bound the isolate budget charges, so the
    /// constructor rejects it rather than documenting the requirement.
    pub fn new(buf: &'a mut Vec<u8>, cap: usize) -> Result<Self, BodyWriteError> {
        let capacity = buf.capacity();
        if capacity < cap {
            return Err(BodyWriteError::UnderCapacity { capacity, cap });
        }
        Ok(Self {
            buf,
            cap,
            cap_limit: u64::try_from(cap).unwrap_or(u64::MAX),
            over_cap: false,
        })
    }

    /// Append `bytes` if they fit entirely within the remaining cap.
    ///
    /// Shares a name with [`std::io::Write::write_all`] deliberately:
    /// bidders call this one and get a typed [`BodyWriteError`], while
    /// the trait method stays available to `serde_json` and `flate2`.
    ///
    /// # Errors
    /// [`BodyWriteError::OverCap`] when the append would cross `cap`.
    /// The buffer is unchanged in that case — this is an all-or-nothing
    /// append, never a partial one.
    #[expect(
        clippy::same_name_method,
        reason = "the inherent typed-error write_all is the bidder-facing API; \
                  the io::Write one serves serde_json and the gzip encoder"
    )]
    pub fn write_all(&mut self, bytes: &[u8]) -> Result<(), BodyWriteError> {
        if self.remaining() < bytes.len() {
            self.over_cap = true;
            return Err(self.over_cap_error());
        }
        self.buf.extend_from_slice(bytes);
        Ok(())
    }

    /// Serialize `value` as JSON directly into the capped buffer.
    ///
    /// # Errors
    /// [`BodyWriteError::OverCap`] when the encoding crosses `cap`, and
    /// [`BodyWriteError::Serialize`] for any other serializer failure.
    /// Bidders never see a `std::io` error.
    pub fn write_json<T>(&mut self, value: &T) -> Result<(), BodyWriteError>
    where
        T: serde::Serialize + ?Sized,
    {
        self.over_cap = false;
        match serde_json::to_writer(&mut *self, value) {
            Ok(()) => Ok(()),
            Err(_) if self.over_cap => Err(self.over_cap_error()),
            Err(_) => Err(BodyWriteError::Serialize),
        }
    }

    /// Bytes appended through this writer's cap so far.
    #[must_use]
    pub fn written(&self) -> usize {
        self.buf.len()
    }

    fn remaining(&self) -> usize {
        self.cap.saturating_sub(self.buf.len())
    }

    fn over_cap_error(&self) -> BodyWriteError {
        BodyWriteError::OverCap {
            limit: self.cap_limit,
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "io::Write's remaining defaults are correct here, and \
              write_all_vectored is unstable so it cannot be implemented on stable"
)]
impl io::Write for CappedWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let room = self.remaining();
        if room == 0 && !buf.is_empty() {
            self.over_cap = true;
            return Err(io::Error::from(io::ErrorKind::WriteZero));
        }
        let take = room.min(buf.len());
        let Some(fitting) = buf.get(..take) else {
            self.over_cap = true;
            return Err(io::Error::from(io::ErrorKind::WriteZero));
        };
        self.buf.extend_from_slice(fitting);
        if take < buf.len() {
            self.over_cap = true;
        }
        Ok(take)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{BodyWriteError, CappedWriter};

    #[test]
    fn capped_writer_exact_cap_passes_cap_plus_one_rejects() {
        let cap = 16_usize;
        let cap_limit = 16_u64;
        let mut buf = Vec::with_capacity(cap);
        let capacity_before = buf.capacity();
        let mut writer = CappedWriter::new(&mut buf, cap).unwrap();

        writer.write_all(&[b'a'; 15]).unwrap();
        // The cap-th byte is accepted.
        writer.write_all(b"b").unwrap();
        assert_eq!(writer.written(), cap);

        // Byte cap+1 is refused.
        let err = writer.write_all(b"c").unwrap_err();
        assert_eq!(err, BodyWriteError::OverCap { limit: cap_limit });

        assert_eq!(buf.len(), cap);
        assert_eq!(
            buf.capacity(),
            capacity_before,
            "an over-cap write must not grow the backing buffer"
        );
    }

    #[test]
    fn capped_writer_rejects_under_capacity_buffer() {
        let cap = 64_usize;

        let mut short = Vec::with_capacity(cap - 1);
        let short_capacity = short.capacity();
        match CappedWriter::new(&mut short, cap) {
            Err(BodyWriteError::UnderCapacity { capacity, cap: got }) => {
                assert_eq!(capacity, short_capacity);
                assert_eq!(got, cap);
            }
            Err(other) => panic!("expected UnderCapacity, got {other:?}"),
            Ok(_) => panic!("a buffer that would reallocate must not yield a writer"),
        }

        // The exact-capacity buffer is the normal call and is accepted.
        let mut exact = Vec::with_capacity(cap);
        CappedWriter::new(&mut exact, cap).unwrap();
    }

    #[test]
    fn capped_writer_write_json_over_cap_is_over_cap_not_serialize() {
        let cap = 8_usize;
        let cap_limit = 8_u64;
        let mut buf = Vec::with_capacity(cap);
        let mut writer = CappedWriter::new(&mut buf, cap).unwrap();

        let value = vec!["a-value-far-longer-than-the-cap"; 4];
        let err = writer.write_json(&value).unwrap_err();
        assert_eq!(err, BodyWriteError::OverCap { limit: cap_limit });
    }

    #[test]
    fn capped_writer_write_json_under_cap_round_trips() {
        let cap = 64_usize;
        let mut buf = Vec::with_capacity(cap);
        let mut writer = CappedWriter::new(&mut buf, cap).unwrap();

        writer.write_json(&["ab", "cd"]).unwrap();
        assert_eq!(writer.written(), 11);
        assert_eq!(buf.as_slice(), br#"["ab","cd"]"#);
    }
}
