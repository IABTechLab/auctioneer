//! Shared hostile `OpenRTB` inputs, not shared privacy/debug policy expectations.

use auctioneer_openrtb::types::bid_request::BidRequest;
use serde_json::{Value, json};

#[must_use]
pub fn full_geo(lat: f64, lon: f64) -> Value {
    json!({
        "lat": lat, "lon": lon, "country": "USA", "type": 1,
        "accuracy": 5, "lastfix": 2, "ipservice": 3,
        "region": "CA", "regionfips104": "06", "metro": "807",
        "city": "San Francisco", "zip": "94107", "utcoffset": -480,
        "ext": {"vendor": {"lat": 1.23456, "id": "geo-context"}},
        "vendor": {"lon": 2.34567}
    })
}

/// Decode the shared hostile input through the ordinary request model.
///
/// # Errors
/// Returns a deserialization error if the fixture no longer matches the request model.
pub fn request() -> Result<BidRequest, serde_json::Error> {
    serde_json::from_value(json!({
        "id": "request-id", "at": 1, "tmax": 500, "cur": ["USD"],
        "user": {
            "id": "user-id", "buyeruid": "buyer-id", "yob": 1984, "gender": "F",
            "eids": [{"source": "native.test", "uids": [{"id": "native-id"}]}],
            "data": [{"id": "audience", "segment": [{"id": "segment-id"}]}],
            "keywords": "sports,news", "kwarray": ["sports", "news"],
            "consent": "consent-string", "custom": {"id": "opaque-user-id"},
            "geo": full_geo(51.507_351, -0.127_758),
            "ext": {
                "eids": [{"source": "legacy.test", "uids": [{"id": "legacy-id"}]}],
                "data": {"interest": "news"}, "consent": "legacy-consent-context",
                "vendor": {"id": "vendor-user-id", "eids": ["opaque-eid"]}
            }
        },
        "device": {
            "ifa": "ifa-id", "dpidsha1": "dpidsha1-id", "dpidmd5": "dpidmd5-id",
            "didsha1": "didsha1-id", "didmd5": "didmd5-id",
            "macsha1": "macsha1-id", "macmd5": "macmd5-id",
            "uid2": "opaque-root-device-id", "metadata": {"sha256email": "opaque-email"},
            "ip": "192.168.1.1", "ipv6": "2001:db8:85a3:1234:5678:8a2e:370:7335",
            "geo": full_geo(48.858_844, 2.294_351), "ua": "test-agent",
            "dnt": 1, "lmt": 1, "os": "iOS", "devicetype": 1,
            "ext": {"atts": 2, "vendor": {"ifa": "opaque-device-id", "ip": "opaque"}}
        },
        "regs": {"coppa": 1, "gdpr": 1, "us_privacy": "1YYY",
                 "gpp": "gpp-string", "gpp_sid": [2, 6], "ext": {"gpc": "1"}},
        "source": {"fd": 1, "tid": "source-tid", "pchain": "payment-chain",
                   "ext": {"schain": {"ver": "1.0", "complete": 1},
                           "vendor": {"tid": "opaque-source-tid"}}},
        "imp": [
            {"id": "imp-1", "banner": {"w": 300, "h": 250}, "bidfloor": 1.5,
             "ext": {"tid": "tid-1", "bidder": {"placementId": 42, "tid": "param-tid"},
                     "prebid": {"bidder": {"example": {"placementId": 42}}},
                     "example": {"id": "legacy-bidder-param"},
                     "data": {"context": "imp-context"}}},
            {"id": "imp-2", "ext": {"tid": "tid-2", "vendor": {"tid": "opaque"}}},
            {"id": "imp-no-ext"}
        ],
        "site": {"id": "site-id", "keywords": "context", "ext": {"data": {"id": "site-fpd"}}},
        "app": {"id": "app-id", "keywords": "context", "ext": {"data": {"id": "app-fpd"}}},
        "ext": {"vendor": {"id": "request-context", "tid": "opaque-request-tid"}},
        "vendor": {"eids": ["opaque-root-eid"]}
    }))
}
