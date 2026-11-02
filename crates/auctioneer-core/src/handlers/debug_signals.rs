//! Bounded, independent HTTP debug signals for the auction handler.

use auctioneer_config::usersync::validate_sync_query_encoding;
use edgezero_core::http::HeaderMap;
use url::form_urlencoded;

const MAX_QUERY_BYTES: usize = 4096;

pub(super) fn http_requested(query: Option<&str>, headers: &HeaderMap) -> bool {
    query_requested(query) || header_requested(headers)
}

fn query_requested(query: Option<&str>) -> bool {
    let Some(raw) = query else { return false };
    if raw.len() > MAX_QUERY_BYTES || validate_sync_query_encoding(raw).is_err() {
        return false;
    }
    form_urlencoded::parse(raw.as_bytes())
        .find(|(name, _value)| name == "debug")
        .is_some_and(|(_name, value)| value == "1")
}

fn header_requested(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all("x-auctioneer-debug").iter();
    let Some(value) = values.next() else {
        return false;
    };
    values.next().is_none()
        && value
            .to_str()
            .is_ok_and(|text| text.trim_matches([' ', '\t']) == "1")
}

#[cfg(test)]
mod tests {
    use super::{header_requested, http_requested, query_requested};
    use edgezero_core::http::{HeaderMap, HeaderValue};

    #[test]
    fn first_decoded_query_occurrence_wins_without_trimming() {
        for (query, expected) in [
            ("debug=1", true),
            ("%64ebug=%31", true),
            ("debug=0&debug=1", false),
            ("debug=1&debug=0", true),
            ("%64ebug=0&debug=1", false),
            ("debug", false),
            ("debug=", false),
            ("debug=true", false),
            ("Debug=1", false),
            ("debug=+1", false),
            ("debug=1+", false),
            ("debug=%2B1", false),
            ("debug=1%20", false),
            ("debug=01", false),
        ] {
            assert_eq!(query_requested(Some(query)), expected, "{query}");
        }
        assert!(!query_requested(None));
    }

    #[test]
    fn the_whole_query_is_strictly_validated_before_any_signal_is_accepted() {
        for suffix in ["%", "%0", "%gg", "%FF", "%C0%AF", "%ED%A0%80"] {
            let query = format!("debug=1&other={suffix}");
            assert!(!query_requested(Some(&query)), "{query}");
        }
        let exact = format!("debug=1&x={}", "a".repeat(4086));
        assert_eq!(exact.len(), 4096);
        assert!(query_requested(Some(&exact)));
        assert!(!query_requested(Some(&format!("{exact}a"))));
    }

    #[test]
    fn a_single_text_header_trims_only_optional_http_whitespace() {
        for (value, expected) in [
            ("1", true),
            (" \t1\t ", true),
            ("", false),
            ("true", false),
            ("01", false),
            ("1,1", false),
            ("1, 0", false),
        ] {
            let mut headers = HeaderMap::new();
            headers.append("X-Auctioneer-Debug", HeaderValue::from_str(value).unwrap());
            assert_eq!(header_requested(&headers), expected, "{value:?}");
        }
        let mut headers = HeaderMap::new();
        headers.append(
            "x-auctioneer-debug",
            HeaderValue::from_bytes(&[0xff]).unwrap(),
        );
        assert!(!header_requested(&headers));
        headers.clear();
        headers.append("x-auctioneer-debug", HeaderValue::from_static("1"));
        headers.append("x-auctioneer-debug", HeaderValue::from_static("1"));
        assert!(!header_requested(&headers));
    }

    #[test]
    fn malformed_or_false_query_cannot_cancel_the_independent_header() {
        for query in [None, Some("debug=0"), Some("debug=1&x=%FF")] {
            for header in [false, true] {
                let mut headers = HeaderMap::new();
                if header {
                    headers.insert("x-auctioneer-debug", HeaderValue::from_static("1"));
                }
                assert_eq!(http_requested(query, &headers), header);
            }
        }
        assert!(http_requested(Some("debug=1"), &HeaderMap::new()));
    }
}
