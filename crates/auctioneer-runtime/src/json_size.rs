//! Allocation-free JSON sizing for pre-materialization resource charges.

use serde::Serialize;
use std::io::{self, Write};

#[derive(Default)]
struct Counter(u64);

#[expect(
    clippy::missing_trait_methods,
    reason = "provided Write methods delegate to write; vectored capability methods are unstable"
)]
impl Write for Counter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0 = u64::try_from(buf.len())
            .ok()
            .and_then(|len| self.0.checked_add(len))
            .ok_or_else(|| io::Error::other("JSON size overflow"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Count the exact serialized length without retaining an output buffer.
///
/// # Errors
/// Returns the serializer's error, including checked length overflow.
pub fn serialized_json_bytes<T: Serialize + ?Sized>(value: &T) -> Result<u64, serde_json::Error> {
    let mut counter = Counter::default();
    serde_json::to_writer(&mut counter, value)?;
    Ok(counter.0)
}

#[cfg(test)]
mod tests {
    use super::{Counter, serialized_json_bytes};
    use crate::alloc_track;
    use serde_json::json;
    use std::io::Write as _;

    #[test]
    fn counting_serialization_matches_actual_escaped_output() {
        for value in [
            json!(null),
            json!({"a":"\n\"\\\t", "b":[1_i32, false, null]}),
            json!({"unicode":"\u{212a}\u{1f600}"}),
            json!(vec![json!({"":[]}); 1000]),
        ] {
            let expected = u64::try_from(serde_json::to_vec(&value).unwrap().len()).unwrap();
            assert_eq!(serialized_json_bytes(&value).unwrap(), expected);
        }
    }

    #[test]
    fn overflow_does_not_wrap_the_counter() {
        let mut count = Counter(u64::MAX);
        assert!(count.write_all(b"x").is_err());
        assert_eq!(count.0, u64::MAX);
    }

    #[test]
    fn counter_does_not_allocate() {
        let value = serde_json::json!({"payload":vec![serde_json::json!({"":[]}); 1000]});
        let before = alloc_track::allocated_on_this_thread();
        let bytes = serialized_json_bytes(&value).unwrap();
        let after = alloc_track::allocated_on_this_thread();
        assert!(bytes > 0);
        assert_eq!(after, before);
    }
}
