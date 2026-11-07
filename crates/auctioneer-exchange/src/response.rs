//! Deterministic `BidResponse` builder.
//!
//! Layout invariants:
//! - `seatbid` sorted by `seat` (ascending UTF-8).
//! - `ext.errors` / `ext.warnings` / `ext.responsetimemillis` serialize as
//!   sorted-key maps. We use `BTreeMap` to encode this in the type.
//! - Bidders are walked in registry order (the caller hands them in
//!   already sorted; we never re-shuffle).

use crate::responsetime::ResponseTimeStat;
use auctioneer_bidder::{BidderName, BrowserAuctionConfig, ConstructionDiagnostics, TypedBid};
use auctioneer_openrtb::types::bid_response::{Bid, BidResponse, SeatBid};
use serde::ser::{Error as _, SerializeSeq as _};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A single error/warning row. The wire shape is
/// `{code, message}` — the code is a raw `u32` so any code value
/// can be emitted without adding a new variant to the foundational `WarningCode`
/// enum (which only covers 10001 / 10999).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorEntry {
    pub code: u32,
    pub message: String,
}

/// Non-bid row for the `seatnonbid` array.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonBidRow {
    pub imp_id: String,
    pub statuscode: u32,
}

/// Per-bidder material the response builder needs. Keyed by
/// `BidderName` in the caller's `BTreeMap` so iteration is
/// lexicographic.
#[derive(Default)]
pub struct BidderResultBundle {
    /// Discovered implementation binding, distinct from alias and upstream seat.
    /// Low-level callers without bindings retain `None`, never a guessed name.
    pub implementation: Option<&'static str>,
    pub bids: Vec<TypedBid>,
    pub errors: Vec<ErrorEntry>,
    pub warnings: Vec<ErrorEntry>,
    pub timing: ResponseTimeStat,
    /// True iff at least one outbound call dispatched (i.e. the bidder
    /// produced a `make_requests` result that survived limits + privacy).
    pub dispatched: bool,
    /// Canonical identity is retained once, not copied into every provider record.
    pub browser_auctions: Option<BrowserAuctionResult>,
}

/// Exchange-attributed optional output from one bidder's completed calls.
pub struct BrowserAuctionResult {
    pub adapter: BidderName,
    pub configs: Vec<BrowserAuctionConfig>,
}

pub struct ResponseInput {
    pub bidder_visible_tmax_ms: u32,
    pub auction_timestamp_ms: u64,
    pub return_all_bid_status: bool,
    /// Caller-built `seatnonbid` rows (empty unless `return_all_bid_status`).
    pub seatnonbid: BTreeMap<BidderName, Vec<NonBidRow>>,
    pub bundles: BTreeMap<BidderName, BidderResultBundle>,
    /// Fixed, separately funded construction rows, including zero-call bidders.
    pub construction_diagnostics: BTreeMap<BidderName, ConstructionDiagnostics>,
    /// Auction-wide errors / warnings (e.g. code 12 `TmaxTimeout`).
    pub prebid_errors: Vec<ErrorEntry>,
    pub prebid_warnings: Vec<ErrorEntry>,
}

/// Build the deterministic `BidResponse`.
///
/// # Errors
/// Returns `serde_json::Error` if serializing one of the typed
/// `BTreeMap<String, _>` extension blocks fails (should be impossible
/// for these well-typed maps but propagated for defensive symmetry
/// with the rest of the error path).
pub fn build_response(input: ResponseInput) -> Result<BidResponse, serde_json::Error> {
    let no_live = input.bundles.values().all(|b| !b.dispatched);
    let nbr = (no_live && !input.bundles.is_empty()).then_some(7_i32);

    // Everything that READS the bundles runs before the bids are moved
    // out of them, so the move below is the last use.
    let (errors, warnings) = collect_errors_and_warnings(&input);
    let rtm = collect_response_times(&input.bundles);
    let seatnonbid_blocks = if input.return_all_bid_status {
        build_seatnonbid_blocks(&input.seatnonbid)
    } else {
        Vec::new()
    };

    let mut ext = assemble_ext(
        &errors,
        &warnings,
        &rtm,
        input.bidder_visible_tmax_ms,
        input.auction_timestamp_ms,
        seatnonbid_blocks,
    )?;
    // Serialize borrowed error views before consuming their bundle owners.
    let (seatbids, auction_configs) = build_seatbids(input.bundles);
    if !auction_configs.is_empty() {
        let prebid = ext
            .get_mut("prebid")
            .and_then(serde_json::Value::as_object_mut)
            .ok_or_else(|| serde_json::Error::custom("missing response extension container"))?;
        let mut fledge = serde_json::Map::new();
        fledge.insert(
            "auctionconfigs".into(),
            serde_json::Value::Array(auction_configs),
        );
        prebid.insert("fledge".into(), serde_json::Value::Object(fledge));
    }
    let cur = if seatbids.is_empty() {
        String::new()
    } else {
        "USD".to_owned()
    };

    Ok(BidResponse {
        id: String::new(),
        seatbid: seatbids,
        cur,
        nbr,
        ext: Some(serde_json::Value::Object(ext)),
        ..BidResponse::default()
    })
}

/// Takes the bundles BY VALUE and MOVES each bid into its `SeatBid`.
///
/// The bids are the largest thing an auction holds — a bid-dense
/// response reaches thousands of them, each a `Bid` with five live
/// string allocations — and a borrowing version had to clone every one,
/// leaving two full copies alive at once while the response serialized.
/// Nothing reads a bundle's bids after this point, so the copy funded
/// nothing. Everything that does read the bundles runs before the
/// caller hands them over.
fn build_seatbids(
    bundles: BTreeMap<BidderName, BidderResultBundle>,
) -> (Vec<SeatBid>, Vec<serde_json::Value>) {
    let mut seatbids: Vec<SeatBid> = Vec::new();
    let mut auction_configs = Vec::new();
    for (bidder, bundle) in bundles {
        if let Some(browser) = bundle.browser_auctions {
            for record in browser.configs {
                let mut output = serde_json::Map::new();
                output.insert("impid".into(), serde_json::Value::String(record.imp_id));
                output.insert(
                    "bidder".into(),
                    serde_json::Value::String(bidder.to_string()),
                );
                output.insert(
                    "adapter".into(),
                    serde_json::Value::String(browser.adapter.to_string()),
                );
                output.insert("config".into(), record.config);
                auction_configs.push(serde_json::Value::Object(output));
            }
        }
        if bundle.bids.is_empty() {
            continue;
        }
        let bids: Vec<Bid> = bundle.bids.into_iter().map(|typed| typed.bid).collect();
        seatbids.push(SeatBid {
            bid: bids,
            seat: bidder.as_str().to_owned(),
            group: 0,
            other: serde_json::Map::new(),
        });
    }
    seatbids.sort_by(|left, right| left.seat.cmp(&right.seat));
    (seatbids, auction_configs)
}

#[derive(Default)]
struct ErrorRows<'a> {
    ordinary: &'a [ErrorEntry],
    construction: Option<&'a ConstructionDiagnostics>,
    global: &'a [ErrorEntry],
}

impl Serialize for ErrorRows<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let count = self
            .ordinary
            .len()
            .checked_add(self.construction.map_or(0, ConstructionDiagnostics::len))
            .and_then(|length| length.checked_add(self.global.len()))
            .ok_or_else(|| S::Error::custom("error row count overflow"))?;
        let mut sequence = serializer.serialize_seq(Some(count))?;
        for entry in self.ordinary {
            sequence.serialize_element(entry)?;
        }
        if let Some(diagnostics) = self.construction {
            for entry in diagnostics.iter() {
                sequence.serialize_element(entry)?;
            }
        }
        for entry in self.global {
            sequence.serialize_element(entry)?;
        }
        sequence.end()
    }
}

type NamedErrorMap<'a> = BTreeMap<&'a str, ErrorRows<'a>>;

fn collect_errors_and_warnings(input: &ResponseInput) -> (NamedErrorMap<'_>, NamedErrorMap<'_>) {
    let mut errors: NamedErrorMap = BTreeMap::new();
    let mut warnings: NamedErrorMap = BTreeMap::new();
    for (bidder, bundle) in &input.bundles {
        if !bundle.errors.is_empty() {
            errors.entry(bidder.as_str()).or_default().ordinary = &bundle.errors;
        }
        if !bundle.warnings.is_empty() {
            warnings.entry(bidder.as_str()).or_default().ordinary = &bundle.warnings;
        }
    }
    for (bidder, diagnostics) in &input.construction_diagnostics {
        if !diagnostics.is_empty() {
            errors.entry(bidder.as_str()).or_default().construction = Some(diagnostics);
        }
    }
    // An accepted alias may share the global key; keep bidder rows first.
    if !input.prebid_errors.is_empty() {
        errors.entry("prebid").or_default().global = &input.prebid_errors;
    }
    if !input.prebid_warnings.is_empty() {
        warnings.entry("prebid").or_default().global = &input.prebid_warnings;
    }
    (errors, warnings)
}

fn collect_response_times(
    bundles: &BTreeMap<BidderName, BidderResultBundle>,
) -> BTreeMap<String, u32> {
    let mut rtm: BTreeMap<String, u32> = BTreeMap::new();
    for (bidder, bundle) in bundles {
        if bundle.dispatched {
            rtm.insert(bidder.as_str().to_owned(), bundle.timing.total_ms());
        }
    }
    rtm
}

fn build_seatnonbid_blocks(
    seatnonbid: &BTreeMap<BidderName, Vec<NonBidRow>>,
) -> Vec<serde_json::Value> {
    let mut rows: Vec<serde_json::Value> = Vec::new();
    for (bidder, non_bids) in seatnonbid {
        let nonbids: Vec<serde_json::Value> = non_bids
            .iter()
            .map(|row| {
                serde_json::json!({
                    "impid": row.imp_id,
                    "statuscode": row.statuscode,
                })
            })
            .collect();
        rows.push(serde_json::json!({
            "seat": bidder.as_str(),
            "nonbid": nonbids,
        }));
    }
    rows.sort_by_key(|value| {
        value
            .get("seat")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    });
    rows
}

fn assemble_ext(
    errors: &NamedErrorMap<'_>,
    warnings: &NamedErrorMap<'_>,
    rtm: &BTreeMap<String, u32>,
    bidder_visible_tmax_ms: u32,
    auction_timestamp_ms: u64,
    seatnonbid_blocks: Vec<serde_json::Value>,
) -> Result<serde_json::Map<String, serde_json::Value>, serde_json::Error> {
    let mut ext_obj = serde_json::Map::new();
    if !errors.is_empty() {
        ext_obj.insert("errors".into(), serde_json::to_value(errors)?);
    }
    if !warnings.is_empty() {
        ext_obj.insert("warnings".into(), serde_json::to_value(warnings)?);
    }
    if !rtm.is_empty() {
        ext_obj.insert("responsetimemillis".into(), serde_json::to_value(rtm)?);
    }
    ext_obj.insert(
        "tmaxrequest".into(),
        serde_json::Value::from(bidder_visible_tmax_ms),
    );
    let mut prebid_obj = serde_json::Map::new();
    prebid_obj.insert(
        "auctiontimestamp".into(),
        serde_json::Value::from(auction_timestamp_ms),
    );
    if !seatnonbid_blocks.is_empty() {
        prebid_obj.insert(
            "seatnonbid".into(),
            serde_json::Value::Array(seatnonbid_blocks),
        );
    }
    ext_obj.insert("prebid".into(), serde_json::Value::Object(prebid_obj));
    // Bounded diagnostics are attached separately by the exchange capture path.
    Ok(ext_obj)
}

#[cfg(test)]
mod tests {
    use super::{
        BidderName, BidderResultBundle, BrowserAuctionResult, ErrorEntry, ResponseInput,
        build_response,
    };
    use auctioneer_bidder::BrowserAuctionConfig;
    use serde_json::json;
    use std::collections::BTreeMap;

    const RESERVED_PREBID_NAMESPACE: &str = "prebid";

    #[test]
    fn browser_only_results_move_payloads_with_authoritative_alias_and_adapter_names() {
        let payload = json!({"payload": "private-config".repeat(1024), "bidder":"fake_forged", "adapter":"fake_forged"});
        let original = payload["payload"].as_str().unwrap().as_ptr();
        let bundles = BTreeMap::from([
            (
                BidderName::new("fake_z_alias"),
                BidderResultBundle {
                    browser_auctions: Some(BrowserAuctionResult {
                        adapter: BidderName::new("fake_base"),
                        configs: vec![BrowserAuctionConfig {
                            imp_id: "z_imp".into(),
                            config: payload,
                        }],
                    }),
                    dispatched: true,
                    ..BidderResultBundle::default()
                },
            ),
            (
                BidderName::new("fake_a_alias"),
                BidderResultBundle {
                    browser_auctions: Some(BrowserAuctionResult {
                        adapter: BidderName::new("fake_base"),
                        configs: vec![BrowserAuctionConfig {
                            imp_id: "a_imp".into(),
                            config: json!(null),
                        }],
                    }),
                    dispatched: true,
                    ..BidderResultBundle::default()
                },
            ),
        ]);
        let response = build_response(ResponseInput {
            bidder_visible_tmax_ms: 750,
            auction_timestamp_ms: 42,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles,
            construction_diagnostics: BTreeMap::new(),
            prebid_errors: Vec::new(),
            prebid_warnings: Vec::new(),
        })
        .unwrap();
        assert!(response.seatbid.is_empty());
        assert!(response.cur.is_empty());
        assert!(response.nbr.is_none());
        let ext = response.ext.unwrap();
        let records = ext["prebid"]["fledge"]["auctionconfigs"]
            .as_array()
            .unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(
            records[0],
            json!({"impid":"a_imp", "bidder":"fake_a_alias", "adapter":"fake_base", "config":null})
        );
        assert_eq!(records[1]["impid"], "z_imp");
        assert_eq!(records[1]["bidder"], "fake_z_alias");
        assert_eq!(records[1]["adapter"], "fake_base");
        assert_eq!(records[1]["config"]["bidder"], "fake_forged");
        assert_eq!(
            records[1]["config"]["payload"].as_str().unwrap().as_ptr(),
            original,
            "configuration trees must move, not serialize/reparse or clone"
        );
    }

    #[test]
    fn empty_browser_result_omits_the_optional_extension() {
        let response = build_response(ResponseInput {
            bidder_visible_tmax_ms: 750,
            auction_timestamp_ms: 42,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles: BTreeMap::from([(
                BidderName::new("fake_alias"),
                BidderResultBundle {
                    browser_auctions: Some(BrowserAuctionResult {
                        adapter: BidderName::new("fake_base"),
                        configs: Vec::new(),
                    }),
                    dispatched: true,
                    ..BidderResultBundle::default()
                },
            )]),
            construction_diagnostics: BTreeMap::new(),
            prebid_errors: Vec::new(),
            prebid_warnings: Vec::new(),
        })
        .unwrap();
        assert!(response.ext.unwrap()["prebid"].get("fledge").is_none());
    }

    #[test]
    fn nonfatal_diagnostics_join_existing_errors_without_losing_global_alias_rows() {
        use auctioneer_bidder::{ConstructionDiagnostic, ConstructionDiagnostics};
        use serde_json::json;

        let mut diagnostics = ConstructionDiagnostics::default();
        diagnostics.push(ConstructionDiagnostic::InvalidImpressionParameters);
        let mut bundles = BTreeMap::new();
        bundles.insert(
            BidderName::new(RESERVED_PREBID_NAMESPACE),
            BidderResultBundle {
                errors: vec![entry(5, "existing error")],
                ..BidderResultBundle::default()
            },
        );
        let input = ResponseInput {
            bidder_visible_tmax_ms: 750,
            auction_timestamp_ms: 0,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles,
            construction_diagnostics: BTreeMap::from([(
                BidderName::new(RESERVED_PREBID_NAMESPACE),
                diagnostics,
            )]),
            prebid_errors: vec![entry(12, "tmax expired")],
            prebid_warnings: vec![],
        };
        let response = build_response(input).unwrap();
        assert_eq!(
            response.ext.unwrap()["errors"]["prebid"],
            json!([
                {"code": 5_u32, "message": "existing error"},
                {"code": 2_u32, "message": "invalid impression parameters"},
                {"code": 12_u32, "message": "tmax expired"},
            ])
        );
    }

    fn entry(code: u32, msg: &str) -> ErrorEntry {
        ErrorEntry {
            code,
            message: msg.into(),
        }
    }

    #[test]
    fn empty_seatbid_returns_cur_empty_string() {
        let input = ResponseInput {
            bidder_visible_tmax_ms: 750,
            auction_timestamp_ms: 1_700_000_000_000,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles: BTreeMap::new(),
            construction_diagnostics: BTreeMap::new(),
            prebid_errors: vec![],
            prebid_warnings: vec![],
        };
        let resp = build_response(input).expect("build_response infallible on typed inputs");
        assert_eq!(resp.cur, "");
    }

    #[test]
    fn ext_errors_and_warnings_sorted_by_key() {
        let mut bundles: BTreeMap<BidderName, BidderResultBundle> = BTreeMap::new();
        bundles.insert(
            BidderName::new("fake_ccc"),
            BidderResultBundle {
                errors: vec![entry(4, "bad response")],
                dispatched: true,
                ..BidderResultBundle::default()
            },
        );
        bundles.insert(
            BidderName::new("fake_aaa"),
            BidderResultBundle {
                warnings: vec![entry(6, "disabled")],
                dispatched: true,
                ..BidderResultBundle::default()
            },
        );
        let input = ResponseInput {
            bidder_visible_tmax_ms: 750,
            auction_timestamp_ms: 1_700_000_000_000,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles,
            construction_diagnostics: BTreeMap::new(),
            prebid_errors: vec![],
            prebid_warnings: vec![],
        };
        let resp = build_response(input).expect("build_response infallible on typed inputs");
        let raw = serde_json::to_string(resp.ext.as_ref().unwrap()).unwrap();
        assert!(raw.contains(r#""fake_aaa""#));
        assert!(raw.contains(r#""fake_ccc""#));
    }

    #[test]
    fn prebid_alias_errors_are_preserved_before_global_errors() {
        let input = ResponseInput {
            bidder_visible_tmax_ms: 750,
            auction_timestamp_ms: 1_700_000_000_000,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles: BTreeMap::from([
                (
                    BidderName::new(RESERVED_PREBID_NAMESPACE),
                    BidderResultBundle {
                        errors: vec![entry(4, "bad response"), entry(1, "timeout")],
                        ..BidderResultBundle::default()
                    },
                ),
                (
                    BidderName::new("fake_ccc"),
                    BidderResultBundle {
                        errors: vec![entry(2, "connection failed")],
                        ..BidderResultBundle::default()
                    },
                ),
            ]),
            construction_diagnostics: BTreeMap::new(),
            prebid_errors: vec![entry(12, "tmax expired"), entry(5, "auction failed")],
            prebid_warnings: vec![],
        };

        let resp = build_response(input).expect("typed diagnostics serialize");
        let ext = resp.ext.expect("response has ext");
        assert_eq!(
            ext["errors"],
            serde_json::json!({
                "prebid": [
                    {"code": 4_u32, "message": "bad response"},
                    {"code": 1_u32, "message": "timeout"},
                    {"code": 12_u32, "message": "tmax expired"},
                    {"code": 5_u32, "message": "auction failed"},
                ],
                "fake_ccc": [{"code": 2_u32, "message": "connection failed"}],
            })
        );
        assert!(ext.get("warnings").is_none());
    }

    #[test]
    fn prebid_alias_warnings_are_preserved_before_global_warnings() {
        let input = ResponseInput {
            bidder_visible_tmax_ms: 750,
            auction_timestamp_ms: 1_700_000_000_000,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles: BTreeMap::from([
                (
                    BidderName::new(RESERVED_PREBID_NAMESPACE),
                    BidderResultBundle {
                        warnings: vec![entry(6, "disabled"), entry(10017, "privacy denied")],
                        ..BidderResultBundle::default()
                    },
                ),
                (
                    BidderName::new("fake_aaa"),
                    BidderResultBundle {
                        warnings: vec![entry(10012, "sampled out")],
                        ..BidderResultBundle::default()
                    },
                ),
            ]),
            prebid_errors: vec![],
            construction_diagnostics: BTreeMap::new(),
            prebid_warnings: vec![entry(10001, "invalid consent"), entry(10019, "stale GCL")],
        };

        let resp = build_response(input).expect("typed diagnostics serialize");
        let ext = resp.ext.expect("response has ext");
        assert_eq!(
            ext["warnings"],
            serde_json::json!({
                "fake_aaa": [{"code": 10012_u32, "message": "sampled out"}],
                "prebid": [
                    {"code": 6_u32, "message": "disabled"},
                    {"code": 10017_u32, "message": "privacy denied"},
                    {"code": 10001_u32, "message": "invalid consent"},
                    {"code": 10019_u32, "message": "stale GCL"},
                ],
            })
        );
        assert!(ext.get("errors").is_none());
    }

    #[test]
    fn base_response_does_not_emit_debug() {
        let input = ResponseInput {
            bidder_visible_tmax_ms: 750,
            auction_timestamp_ms: 1_700_000_000_000,
            return_all_bid_status: false,
            seatnonbid: BTreeMap::new(),
            bundles: BTreeMap::new(),
            construction_diagnostics: BTreeMap::new(),
            prebid_errors: vec![],
            prebid_warnings: vec![],
        };
        let resp = build_response(input).expect("build_response infallible on typed inputs");
        let raw = serde_json::to_string(resp.ext.as_ref().unwrap()).unwrap();
        assert!(
            !raw.contains("\"debug\""),
            "the base response builder must not inject ext.debug"
        );
    }
}
