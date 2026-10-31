//! Native requested heap plus explicit cache inline slots.
//! This excludes complete call frames, other schema scratch and physical bounds.

use crate::schema_formats::HOSTNAME_SOURCE;
use crate::schema_meta_heap::{FreshAnchoredHalfHeap, FreshMetaHeapError};
use regex_automata::meta::Cache;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum BundleHeapError {
    Arithmetic,
    UnsupportedFormat,
    Hostname(FreshMetaHeapError),
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct CacheBundleHeap {
    pub pattern_bytes: usize,
    pub inline_bytes: usize,
    pub format_bytes: usize,
    pub peak: usize,
}

impl CacheBundleHeap {
    /// Each pattern's full lifecycle peak remains funded while siblings grow.
    /// Inline bytes belong to the compiled wrapper, not a guessed Rust layout.
    pub(crate) fn compose(
        pattern_peaks: &[usize],
        inline_bytes: usize,
        format_bytes: usize,
    ) -> Result<Self, BundleHeapError> {
        let pattern_bytes = pattern_peaks.iter().try_fold(0_usize, |sum, bytes| {
            sum.checked_add(*bytes).ok_or(BundleHeapError::Arithmetic)
        })?;
        let peak = pattern_bytes
            .checked_add(inline_bytes)
            .and_then(|bytes| bytes.checked_add(format_bytes))
            .ok_or(BundleHeapError::Arithmetic)?;
        Ok(Self {
            pattern_bytes,
            inline_bytes,
            format_bytes,
            peak,
        })
    }
}

pub(crate) fn format_peak<'a>(
    formats: impl IntoIterator<Item = &'a str>,
) -> Result<usize, BundleHeapError> {
    let mut hostname = false;
    for format in formats {
        match format {
            // UUID's fixed decode buffer belongs to the separate call-frame proof.
            "uuid" => {}
            "hostname" => hostname = true,
            _ => return Err(BundleHeapError::UnsupportedFormat),
        }
    }
    if !hostname {
        return Ok(0);
    }
    let heap = FreshAnchoredHalfHeap::from_pattern(HOSTNAME_SOURCE, 255)
        .map_err(BundleHeapError::Hostname)?;
    // This cache is callback-local, outside the enclosing generated wrapper.
    heap.peak
        .checked_add(size_of::<Cache>())
        .ok_or(BundleHeapError::Arithmetic)
}

#[cfg(test)]
mod tests {
    use super::{BundleHeapError, Cache, CacheBundleHeap, format_peak};

    #[test]
    fn bundle_peak_keeps_every_pattern_inline_and_callback_owner() {
        assert_eq!(
            CacheBundleHeap::compose(&[11, 29, 43], 7, 13),
            Ok(CacheBundleHeap {
                pattern_bytes: 83,
                inline_bytes: 7,
                format_bytes: 13,
                peak: 103,
            })
        );
        for (patterns, inline, format, peak) in [
            (vec![], 0, 0, 0),
            (vec![], 5, 0, 5),
            (vec![], 0, 17, 17),
            (vec![23], 0, 0, 23),
            (vec![23, 37], 0, 0, 60),
        ] {
            assert_eq!(
                CacheBundleHeap::compose(&patterns, inline, format)
                    .unwrap()
                    .peak,
                peak
            );
        }
    }

    #[test]
    fn every_bundle_addition_refuses_overflow_instead_of_a_partial_charge() {
        for (patterns, inline, format) in [
            (vec![usize::MAX, 1], 0, 0),
            (vec![usize::MAX], 1, 0),
            (vec![usize::MAX], 0, 1),
            (vec![], usize::MAX, 1),
        ] {
            assert_eq!(
                CacheBundleHeap::compose(&patterns, inline, format),
                Err(BundleHeapError::Arithmetic)
            );
        }
        assert_eq!(
            CacheBundleHeap::compose(&[usize::MAX - 8], 3, 5)
                .unwrap()
                .peak,
            usize::MAX
        );
    }

    #[test]
    fn format_owners_are_admitted_explicitly_and_callbacks_are_not_retained() {
        assert_eq!(format_peak([]), Ok(0));
        assert_eq!(format_peak(["uuid", "uuid"]), Ok(0));
        let hostname = format_peak(["hostname"]).unwrap();
        assert!(hostname > 0);
        if size_of::<usize>() == 8 {
            assert_eq!(hostname, 156_942 + size_of::<Cache>());
        }
        assert_eq!(format_peak(["uuid", "hostname"]), Ok(hostname));
        assert_eq!(format_peak(["hostname", "hostname", "uuid"]), Ok(hostname));
        for formats in [
            vec!["unknown"],
            vec!["hostname", "unknown"],
            vec!["unknown", "uuid"],
            vec!["HOSTNAME"],
        ] {
            assert_eq!(
                format_peak(formats),
                Err(BundleHeapError::UnsupportedFormat)
            );
        }
    }
}
