//! Test-only comparator frozen from committed privacy enforcement, never production enforcement.
//! Source: crates/auctioneer-exchange/src/scrub/mod.rs
//! Revision: 3f0f7f5aeb01d96742e0dd2012f179887ee474bc (HEAD supplied for Task 7).
//! Retrieved with git show, not copied from a partially edited working tree.
//! The source below is verbatim except for omitting the cfg(test) GDPR wrapper
//! and cfg(test) unit-test modules/tests. No scrub behavior or visibility changed.
//!
//! Bidder-owned request scrub primitives and decision composition.
//!
//! Decisions are supplied by the caller; this module neither resolves
//! privacy nor reads policy configuration. The auction composes GDPR,
//! activity, CCPA, LMT, and COPPA decisions before invoking these scrubs.
//!
//! Scrubs are idempotent and compose monotonically. Denied user/device identifiers
//! and precise geo also clear their unclassified catch-all maps. Supported
//! same-field shadows are scrubbed; unrelated vendor payloads elsewhere
//! are not recursively scanned for identifiers.

use auctioneer_config_data::types::IpKeepBits;
use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_openrtb::types::device::Geo;
use auctioneer_openrtb::types::field::JsonField;
use serde_json::{Map, Number, Value};
use std::net::{Ipv4Addr, Ipv6Addr};

/// Already-resolved decisions for one bidder's owned request.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrubDecisions {
    pub pass_id: bool,
    pub pass_geo: bool,
    pub transmit_ufpd: bool,
    pub transmit_precise_geo: bool,
    pub transmit_tid: bool,
    pub ccpa: bool,
    pub lmt: bool,
    pub coppa: bool,
}

/// Apply the union of removals without letting an activity branch bypass
/// another policy. COPPA's full-geo removal dominates coordinate rounding.
pub(crate) fn apply_scrubs(req: &mut BidRequest, decisions: ScrubDecisions, keep_bits: IpKeepBits) {
    if !decisions.transmit_ufpd {
        scrub_user_fpd(req);
    }
    if decisions.transmit_ufpd && !decisions.pass_id {
        scrub_gdpr_id(req);
    }
    if decisions.ccpa || decisions.lmt {
        scrub_ccpa_lmt(req, keep_bits);
    }
    if !decisions.pass_geo || !decisions.transmit_precise_geo {
        scrub_geo_and_device_ip(req, keep_bits);
    }
    if decisions.coppa {
        scrub_coppa(req, keep_bits);
    }
    if !decisions.transmit_tid {
        scrub_tid(req);
    }
}

/// Applied when GDPR `PassID = false`.
///
/// Clears typed user/device identifiers and user demographics, EIDs in
/// both locations, and all user/device catch-all maps, including their
/// extension maps and unclassified structured-user-agent data. Standard
/// device context is typed so it survives; opaque contextual device data
/// does not, because unclassified fields are not an identifier allowlist. Typed
/// consent, contextual fields, device ATT status, and independently
/// permitted geo survive. Clearing `user.id` and
/// `user.buyeruid` goes beyond the narrow "device ids, user
/// demographics, `user.ext.eids`" summary on purpose: `buyeruid` is a
/// direct bidder-scoped user id, so leaving it would defeat the decision
/// the gate just made. Likewise the native `user.eids` — clearing only
/// the legacy `user.ext.eids` location would make the scrub trivially
/// bypassable by sending the same identifiers in the newer field.
///
/// Idempotent; safe on a request whose fields are already absent.
pub(crate) fn scrub_gdpr_id(req: &mut BidRequest) {
    if let Some(user) = req.user.as_mut() {
        user.id = None;
        user.buyeruid = None;
        user.yob = None;
        user.gender = None;
        // `user.eids` is a Vec, so it clears rather than nulls.
        user.eids.clear();
        // Unknown user fields are unclassified data, not an identifier allowlist.
        // Retaining them would bypass PassID through fields such as uid2 or hashes.
        user.other.clear();
        if let Some(ext) = user.ext.as_mut() {
            ext.eids = None;
            ext.other.clear();
        }
    }
    if let Some(device) = req.device.as_mut() {
        device.ifa = None;
        // Unknown device fields can carry identifiers under any key or nesting.
        // Standard device context is typed; opaque fields are not safe context.
        device.other.clear();
        if let Some(ext) = device.ext.as_mut() {
            ext.other.clear();
            if let Some(prebid) = ext.prebid.as_mut() {
                prebid.other.clear();
                if let Some(interstitial) = prebid.interstitial.as_mut() {
                    interstitial.other.clear();
                }
            }
        }
        if let Some(sua) = device.sua.as_mut() {
            sua.ext = JsonField::Missing;
            sua.other.clear();
            for brand in sua
                .browsers
                .as_mut()
                .into_iter()
                .flatten()
                .chain(sua.platform.as_mut())
            {
                if let Some(fields) = brand.fields_mut() {
                    fields.ext = JsonField::Missing;
                    fields.other.clear();
                }
            }
        }
    }
}

/// User FPD denial includes identifiers, but does not remove contextual
/// site/app/impression data or coarsen IP/geo on its own.
pub(crate) fn scrub_user_fpd(req: &mut BidRequest) {
    scrub_gdpr_id(req);
    if let Some(user) = req.user.as_mut() {
        user.data = None;
        user.keywords = None;
        for key in ["data", "keywords", "kwarray"] {
            user.other.remove(key);
        }
        if let Some(ext) = user.ext.as_mut() {
            ext.data = None;
            ext.other.remove("data");
        }
        scrub_shadow_ext_fields(&mut user.other, &["data"]);
    }
}

/// CCPA and LMT require ID removal and IP/geo coarsening, not broader FPD.
pub(crate) fn scrub_ccpa_lmt(req: &mut BidRequest, keep_bits: IpKeepBits) {
    scrub_gdpr_id(req);
    scrub_geo_and_device_ip(req, keep_bits);
}

/// COPPA additionally clears every geo field, retaining existing objects
/// as `{}` without creating absent objects.
pub(crate) fn scrub_coppa(req: &mut BidRequest, keep_bits: IpKeepBits) {
    scrub_ccpa_lmt(req, keep_bits);
    if let Some(user) = req.user.as_mut() {
        clear_geo(&mut user.geo, &mut user.other);
    }
    if let Some(device) = req.device.as_mut() {
        clear_geo(&mut device.geo, &mut device.other);
    }
}

fn clear_geo(geo: &mut Option<Geo>, other: &mut Map<String, Value>) {
    let shadow_object = other.remove("geo").is_some_and(|value| value.is_object());
    if geo.is_some() || shadow_object {
        *geo = Some(Geo::default());
    }
}

/// Remove transaction IDs, preserving ordinary request/impression IDs
/// and unrelated extension fields, including bidder parameters.
pub(crate) fn scrub_tid(req: &mut BidRequest) {
    if let Some(source) = req.source.as_mut() {
        source.tid = None;
        source.other.remove("tid");
    }
    for imp in &mut req.imp {
        if let Some(ext) = imp.ext.as_mut() {
            ext.tid = None;
            ext.other.remove("tid");
        }
        scrub_shadow_ext_fields(&mut imp.other, &["tid"]);
    }
}

/// Scrub only recognized leaves of the direct shadow extension object.
/// Preserve unrelated metadata; malformed containers fail closed.
fn scrub_shadow_ext_fields(other: &mut Map<String, Value>, fields: &[&str]) {
    let Some(Value::Object(ext)) = other.get_mut("ext") else {
        other.remove("ext");
        return;
    };
    for field in fields {
        ext.remove(*field);
    }
}

/// Applied when GDPR `PassGeo = false`. Preserves typed country, rounds
/// lat/lon to 2 decimals, removes unclassified geo fields/extensions in
/// both locations, and masks the client IP to `keep_bits` via CIDR.
///
/// A malformed address is DROPPED rather than passed through unmasked —
/// fail-closed on the privacy boundary.
pub(crate) fn scrub_geo_and_device_ip(req: &mut BidRequest, keep_bits: IpKeepBits) {
    // User geo carries the same precision and needs the same coarsening.
    if let Some(user) = req.user.as_mut() {
        round_geo_and_shadow_2dp(&mut user.geo, &mut user.other);
    }
    if let Some(device) = req.device.as_mut() {
        round_geo_and_shadow_2dp(&mut device.geo, &mut device.other);
        device.ip = device
            .ip
            .as_deref()
            .and_then(|v4| mask_ipv4(v4, keep_bits.v4));
        device.ipv6 = device
            .ipv6
            .as_deref()
            .and_then(|v6| mask_ipv6(v6, keep_bits.v6));
        // Drop shadows rather than letting flatten re-emit an unmasked value.
        device.other.remove("ip");
        device.other.remove("ipv6");
    }
}

/// Prefer typed geo over a conflicting flattened container. For shadow-only
/// geo, retain only rounded coordinates and country, as for typed geo.
fn round_geo_and_shadow_2dp(geo: &mut Option<Geo>, other: &mut Map<String, Value>) {
    if let Some(typed) = geo.as_mut() {
        coarsen_geo(typed);
        other.remove("geo");
        return;
    }
    let Some(Value::Object(shadow)) = other.get_mut("geo") else {
        other.remove("geo");
        return;
    };
    shadow.retain(|key, _| matches!(key.as_str(), "lat" | "lon" | "country"));
    for key in ["lat", "lon"] {
        // Malformed coordinates are removed, never forwarded as precise data.
        if let Some(rounded) = shadow
            .remove(key)
            .and_then(|value| value.as_f64())
            .map(round_half_up_2dp)
            .and_then(Number::from_f64)
        {
            shadow.insert(key.to_owned(), Value::Number(rounded));
        }
    }
}

fn coarsen_geo(geo: &mut Geo) {
    geo.lat = geo.lat.map(round_half_up_2dp);
    geo.lon = geo.lon.map(round_half_up_2dp);
    // Alternate coordinates, postal codes, or opaque extensions cannot
    // retain precise location after the typed coordinates were coarsened.
    // bounded fanout also conservatively loses untyped context: region, regionfips104,
    // city, metro, utcoffset, type, ipservice, and lastfix. Context preservation
    // is follow-on privacy enforcement work, not a claim that these fields are precise location:
    // docs/superpowers/specs/2026-10-09-gdpr-scrub-classification-design.md.
    geo.other.clear();
    geo.ext = None;
}

/// Half-UP at 2 decimals over the already-parsed `f64`:
/// `floor(v * 100 + 0.5) / 100`. Worked, FP-unambiguous examples:
/// `1.238 -> 1.24`, `1.232 -> 1.23`, `-1.238 -> -1.24`, `-1.232 -> -1.23`.
///
/// Parsed-`f64` half-up is the documented semantic; there is no
/// decimal-exact rounding. `device.geo.lat/lon` arrives already parsed
/// into `f64` — the original decimal string is gone by the time this
/// runs — so decimal-exact rounding would need the parser re-plumbed.
/// The privacy purpose (coarsening precise geo to roughly a kilometre)
/// is fully served by binary half-up, and the rounding direction at an
/// unrepresentable half (`1.005` is held as `1.00499…` and therefore
/// rounds down) is immaterial to it.
///
/// Non-finite input is returned unchanged: a NaN or infinite coordinate
/// is malformed data the scrub must not convert into a bogus finite
/// value.
#[expect(
    clippy::float_arithmetic,
    reason = "intentional round-half-up on the f64 geo coordinate"
)]
fn round_half_up_2dp(value: f64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    ((value * 100.0_f64) + 0.5_f64).floor() / 100.0_f64
}

fn mask_ipv4(raw: &str, keep_bits: u8) -> Option<String> {
    let addr: Ipv4Addr = raw.parse().ok()?;
    let keep = keep_bits.min(32_u8);
    // `checked_shl` returns `None` when the shift reaches the bit width
    // (keep == 0 -> shift 32), which maps to an all-zero mask. That
    // folds the "keep nothing" case in rather than branching on it.
    // `saturating_sub` + `u32::from` keep the arithmetic and conversion
    // restriction lints satisfied; keep in 1..=32 gives shift 0..=31.
    let shift = 32_u32.saturating_sub(u32::from(keep));
    let mask: u32 = u32::MAX.checked_shl(shift).unwrap_or(0_u32);
    Some(Ipv4Addr::from(u32::from(addr) & mask).to_string())
}

fn mask_ipv6(raw: &str, keep_bits: u8) -> Option<String> {
    let addr: Ipv6Addr = raw.parse().ok()?;
    let keep = keep_bits.min(128_u8);
    // keep == 0 -> shift 128 -> `checked_shl` None -> all-zero mask.
    let shift = 128_u32.saturating_sub(u32::from(keep));
    let mask: u128 = u128::MAX.checked_shl(shift).unwrap_or(0_u128);
    Some(Ipv6Addr::from(u128::from(addr) & mask).to_string())
}
