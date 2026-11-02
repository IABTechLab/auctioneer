//! Implicit-field fill step.

use crate::ext::misc::RegsExt;
use crate::ext::misc::SiteExt;
use crate::ext::request_prebid::{Channel, ExtRequestPrebid};
use crate::request_headers::RequestHeaders;
use crate::types::bid_request::{BidRequest, BidRequestExt};
use crate::types::device::Device;
use crate::types::inventory::{Publisher, Site};
use crate::types::regs::Regs;
use ipnetwork::IpNetwork;
use std::net::IpAddr;
use std::sync::OnceLock;

/// `value`, cut to `limit` bytes at a character boundary.
///
/// Advisory headers are arbitrary UTF-8 and `String::truncate` panics
/// mid-sequence, so the cut walks back to a boundary. No ellipsis: this
/// is a value a bidder reads as data, not a diagnostic a human reads,
/// and three bytes of marker in a `device.ua` would be a worse lie than
/// a short one.
fn copied_within(value: &str, remaining: &mut usize) -> String {
    let limit = *remaining;
    if value.len() <= limit {
        *remaining = remaining.saturating_sub(value.len());
        return value.to_owned();
    }
    // `is_char_boundary(0)` always holds, so the search never fails.
    let end = (0..=limit)
        .rev()
        .find(|offset| value.is_char_boundary(*offset))
        .unwrap_or(0);
    *remaining = remaining.saturating_sub(end);
    value.get(..end).unwrap_or("").to_owned()
}

pub(super) fn run(
    req: &mut BidRequest,
    headers: &RequestHeaders<'_>,
    max_copied_header_bytes: u32,
) {
    // The allowance is ONE figure for the pair — the budget reserves
    // it once and multiplies it by `1 + bidders` — so handing the
    // whole of it to each copy funds half of what they retain
    // together: two 4 KiB copies against a 4 KiB reservation.
    //
    // Split rather than first-come: a shared running total let
    // whichever copy ran first consume everything and leave the other
    // EMPTY, and a silently absent `device.ua` is worse for a bidder
    // than a shortened one. Each field gets half, so the sum fits the
    // reservation whatever the inputs are.
    let combined = usize::try_from(max_copied_header_bytes).unwrap_or(usize::MAX);
    let per_field = combined.div_euclid(2);
    let mut page_budget = per_field;
    let mut ua_budget = combined.saturating_sub(per_field);
    site_default(req, headers, &mut page_budget, combined);
    fill_device(req, headers, &mut ua_budget);
    fill_at(req);
    fill_channel(req);
    ios_lmt_override(req);
    gpc(req, headers);
}

/// Site default. When `app` and `dooh` are None we always materialize
/// an empty `Site`. The URL source for enrichment is `Referer` when
/// it's present and non-empty; otherwise we fall back to the
/// caller-supplied `site.page`. Each of `site.page`, `site.domain`,
/// `site.publisher.domain` is populated from the URL source only when
/// its current value is empty (the caller's value always wins).
fn site_default(
    req: &mut BidRequest,
    headers: &RequestHeaders<'_>,
    copy_budget: &mut usize,
    header_source_cap: usize,
) {
    if req.app.is_some() || req.dooh.is_some() {
        return;
    }
    let site = req.site.get_or_insert_with(Site::default);

    let referer = headers.referer.filter(|value| !value.is_empty());

    // site.page <- Referer (only when caller-supplied page is absent).
    //
    // Charged FIRST, and once: this is the copy that is retained and
    // that every bidder clone carries. `url_source` below is parsed
    // for a host and dropped, so charging it too would debit the
    // allowance twice for one header and leave the retained copy
    // truncated shorter than the transient one.
    if let Some(value) = referer
        && site.page.is_none()
    {
        site.page = Some(copied_within(value, copy_budget));
    }

    // URL source for enrichment: the Referer when present, else the
    // caller's own `site.page`. Borrowed rather than copied — only the
    // host is kept, and `site.domain` is bounded by the URL it parses.
    // Do not parse an unbounded advisory URL or invent a host by truncating it.
    // Body-owned pages are admitted input; header sources share the copy cap.
    let url_source = match referer {
        Some(value) if value.len() <= header_source_cap => Some(value),
        Some(_) => None,
        None => site.page.as_deref(),
    };

    // site.domain + publisher.domain enrichment from the URL source.
    // publisher.domain is the eTLD+1 of the host (psl bundled PSL data).
    if let Some(url) = url_source
        && let Ok(parsed) = url::Url::parse(url)
        && let Some(host) = parsed.host_str()
        // IDNA can expand a short Unicode host. Both derived domain strings
        // are funded as at most one source-sized copy apiece.
        && host.len() <= url.len()
    {
        if site.domain.is_none() {
            site.domain = Some(host.to_owned());
        }
        let publisher = site.publisher.get_or_insert_with(Publisher::default);
        if publisher.domain.is_none()
            && let Some(etld_one) = etld_plus_one(host)
        {
            publisher.domain = Some(etld_one);
        }
    }
    match site.ext.as_mut() {
        None => {
            site.ext = Some(SiteExt {
                amp: Some(0_i32),
                ..Default::default()
            });
        }
        Some(ext) => {
            if ext.amp.is_none() {
                ext.amp = Some(0_i32);
            }
        }
    }
}

/// Public-suffix-list eTLD+1 lookup. Returns `None` for IP literals,
/// `localhost`, and any host with no registrable domain (e.g. bare TLDs).
fn etld_plus_one(host: &str) -> Option<String> {
    // IP literals don't have a domain; psl would return the literal back.
    if host.parse::<IpAddr>().is_ok() {
        return None;
    }
    let domain = psl::domain(host.as_bytes())?;
    str::from_utf8(domain.as_bytes()).ok().map(str::to_owned)
}

/// Device fills. IP sanitize-then-fill; UA / DNT.
fn fill_device(req: &mut BidRequest, headers: &RequestHeaders<'_>, copy_budget: &mut usize) {
    let device = req.device.get_or_insert_with(Device::default);

    // Sanitize: clear unparseable / private / wrong-family IPs even if
    // the client set them. device.ip must be IPv4; device.ipv6 must be
    // IPv6. Without this, a v4 in device.ipv6 (or vice versa) passes
    // the "clean public IP" check and blocks header fill because the
    // fill chain only runs when both fields are empty.
    sanitize_ip(&mut device.ip, Family::V4);
    sanitize_ip(&mut device.ipv6, Family::V6);

    // Fill from headers if still empty.
    if device.ip.is_none()
        && device.ipv6.is_none()
        && let Some(picked) = pick_valid_ip(headers)
    {
        // Route v4 vs v6 by the chosen IP's family.
        if picked.contains(':') {
            device.ipv6 = Some(picked);
        } else {
            device.ip = Some(picked);
        }
    }

    if device.ua.is_none()
        && let Some(user_agent) = headers.user_agent.filter(|value| !value.is_empty())
    {
        device.ua = Some(copied_within(user_agent, copy_budget));
    }
    if device.dnt.is_none() {
        match headers.dnt {
            Some("0") => device.dnt = Some(0_i32),
            Some("1") => device.dnt = Some(1_i32),
            _ => {}
        }
    }
}

/// Address family for body-IP sanitization. `device.ip` is the
/// `OpenRTB` IPv4 slot; `device.ipv6` is the IPv6 slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    V4,
    V6,
}

/// Client-supplied `device.ip` / `device.ipv6` is cleared unless it
/// parses as a clean public IP literal of the expected family. Body
/// fields must NOT carry ports, whitespace, or other transport
/// artifacts (port stripping is a header-chain behavior). Wrong-family
/// values are also cleared so the header chain can take over.
fn sanitize_ip(field: &mut Option<String>, family: Family) {
    if let Some(value) = field.as_deref()
        && !is_clean_public_ip_of(value, family)
    {
        *field = None;
    }
}

/// Strict body-IP validator: trimmed input must parse exactly as
/// `IpAddr` of the requested family (no port, no brackets) AND survive
/// `is_public_ip`. Refuses `8.8.8.8:1234`, `[2001:db8::1]:80`,
/// ` 8.8.8.8 `, `8.8.8.8/24`, a v4 in the v6 slot, etc.
fn is_clean_public_ip_of(value: &str, family: Family) -> bool {
    if value.trim() != value || value.is_empty() {
        return false;
    }
    let Ok(addr) = value.parse::<IpAddr>() else {
        return false;
    };
    let family_ok = match (family, addr) {
        (Family::V4, IpAddr::V4(_)) | (Family::V6, IpAddr::V6(_)) => true,
        (Family::V4, IpAddr::V6(_)) | (Family::V6, IpAddr::V4(_)) => false,
    };
    family_ok && is_public_ip(addr)
}

fn pick_valid_ip(headers: &RequestHeaders<'_>) -> Option<String> {
    // Order: True-Client-IP -> XFF hops (left-to-right) -> X-Real-IP -> RemoteAddr.
    // Each candidate runs through normalize_ip, which strips :port suffixes
    // (e.g. "1.2.3.4:5678", "[2001:db8::1]:80") before public-IP validation.
    if let Some(addr) = headers.true_client_ip.and_then(normalize_ip) {
        return Some(addr);
    }
    if let Some(xff) = headers.x_forwarded_for {
        for hop in xff.split(',') {
            if let Some(addr) = normalize_ip(hop) {
                return Some(addr);
            }
        }
    }
    if let Some(addr) = headers.x_real_ip.and_then(normalize_ip) {
        return Some(addr);
    }
    if let Some(addr) = headers.remote_addr.and_then(normalize_ip) {
        return Some(addr);
    }
    None
}

/// Normalize a header-supplied IP string. Strips whitespace, an optional
/// `:port` suffix (v4: `1.2.3.4:5678`; v6: `[2001:db8::1]:80`), parses,
/// and accepts only addresses that pass `is_public_ip`. Returns the
/// canonical IP string (no port) when valid.
fn normalize_ip(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = strip_port(trimmed);
    let addr = candidate.parse::<IpAddr>().ok()?;
    if !is_public_ip(addr) {
        return None;
    }
    Some(addr.to_string())
}

/// Strip an optional `:port` suffix. Only strips when the suffix is
/// actually a port (`:` followed by a non-empty all-digits string).
/// - `[v6]` → returns `v6`
/// - `[v6]:8080` → returns `v6`
/// - `[v6]junk` / `[v6]:notaport` → returns the input unchanged so the
///   caller's `IpAddr::parse` fails on the garbage
/// - `1.2.3.4:8080` → returns `1.2.3.4`
/// - `1.2.3.4:notaport` → returns the input unchanged
/// - bare v6 with no brackets and no port (multiple colons) is left
///   alone — the colons belong to the address.
fn strip_port(value: &str) -> &str {
    if let Some(after_open) = value.strip_prefix('[')
        && let Some((host, after_close)) = after_open.split_once(']')
    {
        if after_close.is_empty() {
            return host;
        }
        if let Some(port) = after_close.strip_prefix(':')
            && is_numeric_port(port)
        {
            return host;
        }
        // [...]<garbage> — pass through unchanged so parse fails.
        return value;
    }
    // For v4 we can split on the last colon if exactly one colon is present.
    if value.matches(':').count() == 1
        && let Some((host, port)) = value.rsplit_once(':')
        && is_numeric_port(port)
    {
        return host;
    }
    value
}

fn is_numeric_port(port: &str) -> bool {
    !port.is_empty() && port.bytes().all(|byte| byte.is_ascii_digit())
}

/// IANA reserved / non-public IPv4 ranges checked at runtime.
/// `is_shared` / `is_documentation` / `is_benchmarking` /
/// `is_ietf_protocol_assignment` / `is_reserved` are unstable on
/// `Ipv4Addr` in Rust 1.95, so the ranges are spelled out as CIDRs.
static V4_BLOCK: OnceLock<Vec<IpNetwork>> = OnceLock::new();
/// IANA reserved / non-public IPv6 ranges.
static V6_BLOCK: OnceLock<Vec<IpNetwork>> = OnceLock::new();

fn is_public_ip(addr: IpAddr) -> bool {
    if addr.is_loopback() || addr.is_unspecified() || addr.is_multicast() {
        return false;
    }
    let v4_block = V4_BLOCK.get_or_init(|| {
        #[expect(
            clippy::expect_used,
            reason = "hardcoded IANA reserved-range CIDRs are statically valid"
        )]
        {
            [
                "0.0.0.0/8",       // RFC 1122 "this network" / source-only
                "100.64.0.0/10",   // RFC 6598 CGNAT / shared address space
                "192.0.0.0/24",    // RFC 7335 IETF protocol assignments
                "192.0.2.0/24",    // RFC 5737 documentation (TEST-NET-1)
                "198.18.0.0/15",   // RFC 2544 benchmarking
                "198.51.100.0/24", // RFC 5737 documentation (TEST-NET-2)
                "203.0.113.0/24",  // RFC 5737 documentation (TEST-NET-3)
                "240.0.0.0/4",     // RFC 1112 reserved future use
            ]
            .iter()
            .map(|cidr| cidr.parse().expect("hardcoded CIDR is valid"))
            .collect()
        }
    });
    let v6_block = V6_BLOCK.get_or_init(|| {
        #[expect(
            clippy::expect_used,
            reason = "hardcoded IPv6 reserved-range CIDRs are statically valid"
        )]
        {
            [
                "fc00::/7",       // RFC 4193 unique-local
                "fe80::/10",      // RFC 4291 link-local
                "fec0::/10",      // RFC 3879 deprecated site-local
                "2001:db8::/32",  // RFC 3849 documentation
                "100::/64",       // RFC 6666 discard prefix
                "64:ff9b::/96",   // RFC 6052 well-known NAT64
                "64:ff9b:1::/48", // RFC 8215 NAT64
                "2001::/23",      // RFC 2928 IETF protocol assignments
                "2001:2::/48",    // RFC 5180 benchmarking
                "3fff::/20",      // RFC 9637 reserved documentation
                "::ffff:0:0/96",  // RFC 4291 IPv4-mapped — already covered
                                  // by v4 rules in their native form
            ]
            .iter()
            .map(|cidr| cidr.parse().expect("hardcoded CIDR is valid"))
            .collect()
        }
    });

    match addr {
        IpAddr::V4(addr_v4) => {
            if addr_v4.is_private() || addr_v4.is_link_local() || addr_v4.is_broadcast() {
                return false;
            }
            !v4_block.iter().any(|net| net.contains(addr))
        }
        IpAddr::V6(_) => !v6_block.iter().any(|net| net.contains(addr)),
    }
}

/// `at` defaults to 1 (first-price) when absent OR 0.
fn fill_at(req: &mut BidRequest) {
    match req.at {
        None | Some(0_i32) => req.at = Some(1_i32),
        Some(_) => {}
    }
}

/// Channel default. Only when `app` is set AND `ext.prebid.channel`
/// is absent. Rejection of empty `name` happens in
/// `checks::reject_empty_channel_name` (not here).
fn fill_channel(req: &mut BidRequest) {
    if req.app.is_none() {
        return;
    }
    let ext = req.ext.get_or_insert_with(BidRequestExt::default);
    let prebid = ext.prebid.get_or_insert_with(ExtRequestPrebid::default);
    if prebid.channel.is_none() {
        prebid.channel = Some(Channel {
            name: Some("app".to_owned()),
            ..Default::default()
        });
    }
}

/// iOS LMT override based on `device.ext.atts` that is already on the
/// request (read-only — auctioneer does not derive atts). Rule: app + iOS,
/// atts==3 (Authorized) -> lmt=0; atts in 0/1/2 -> lmt=1.
fn ios_lmt_override(req: &mut BidRequest) {
    if req.app.is_none() {
        return;
    }
    let Some(device) = req.device.as_mut() else {
        return;
    };
    let Some(operating_system) = device.os.as_deref() else {
        return;
    };
    if !operating_system.eq_ignore_ascii_case("ios") {
        return;
    }
    let Some(atts_code) = device.ext.as_ref().and_then(|ext| ext.atts) else {
        return;
    };
    device.lmt = Some(match atts_code {
        3_i32 => 0_i32,         // Authorized
        0_i32..=2_i32 => 1_i32, // NotDetermined / Restricted / Denied
        _ => return,            // unknown ATTS code — leave lmt alone
    });
}

/// `Sec-GPC: 1` ALWAYS overwrites `regs.ext.gpc` to "1" (header
/// signal cannot be silently suppressed). Other header values are
/// ignored; a client-supplied JSON value is left alone.
fn gpc(req: &mut BidRequest, headers: &RequestHeaders<'_>) {
    if headers.sec_gpc == Some("1") {
        let regs = req.regs.get_or_insert_with(Regs::default);
        let ext = regs.ext.get_or_insert_with(RegsExt::default);
        ext.gpc = Some("1".to_owned());
    }
}

#[cfg(test)]
mod tests {
    /// These rows are about which fields implicit fill POPULATES, not
    /// about the copy bound, so they run with an allowance no fixture
    /// reaches. `copied_header_is_cut_to_the_allowance` is where the
    /// bound itself is asserted.
    const COPY_BUDGET_NOT_UNDER_TEST: u32 = u32::MAX;

    #[test]
    fn overlong_referer_cannot_expand_bounded_domain_enrichment() {
        let referer = format!("https://{}.example.test/", "h".repeat(8192));
        let headers = RequestHeaders {
            referer: Some(&referer),
            ..RequestHeaders::default()
        };
        let mut request = BidRequest::default();
        super::run(&mut request, &headers, 1024);
        let site = request.site.unwrap();
        assert_eq!(site.page.unwrap().len(), 512);
        assert!(site.domain.is_none());
        assert!(site.publisher.is_none());
    }

    /// Both copied advisory headers are cut to the allowance.
    ///
    /// The budget reserves ONE figure for the pair and multiplies it
    /// by `1 + bidders`, because every per-bidder clone carries both
    /// strings. Nothing bounded either, so a single oversize
    /// `User-Agent` broke the reservation on every clone at once.
    ///
    /// Asserted on both fields and on the COMBINED total: bounding
    /// each to the full allowance separately would let the two
    /// together reach twice what is funded, and a row checking only
    /// `device.ua` would not see that.
    #[test]
    fn copied_headers_are_cut_to_the_allowance() {
        const BUDGET: u32 = 1_024;
        let long = "u".repeat(8 * 1_024);
        let headers = RequestHeaders {
            user_agent: Some(&long),
            referer: Some(&long),
            ..RequestHeaders::default()
        };
        let mut req = BidRequest {
            id: "a".into(),
            ..BidRequest::default()
        };
        run(&mut req, &headers, BUDGET);

        let ua = req
            .device
            .as_ref()
            .and_then(|device| device.ua.as_deref())
            .expect("the User-Agent is copied into device.ua");
        let page = req
            .site
            .as_ref()
            .and_then(|site| site.page.as_deref())
            .expect("the Referer is copied into site.page");

        let budget = usize::try_from(BUDGET).expect("the budget fits a usize");
        assert!(
            ua.len() <= budget,
            "device.ua kept {} B of an {} B header against a {budget} B allowance",
            ua.len(),
            long.len(),
        );
        assert!(
            page.len() <= budget,
            "site.page kept {} B of an {} B header against a {budget} B allowance",
            page.len(),
            long.len(),
        );
        // THE claim: the pair shares ONE reservation, so their SUM is
        // what must fit — not each of them separately. An earlier
        // revision handed the whole allowance to each copy and
        // asserted `sum <= budget * 2`, which is the defect written
        // down as the requirement: two 4 KiB copies passed against a
        // 4 KiB reservation.
        let combined = ua.len().saturating_add(page.len());
        assert!(
            combined <= budget,
            "the copied pair totals {combined} B against a {budget} B SHARED \
             allowance; the budget reserves one figure for both and multiplies \
             it by `1 + bidders`, so a per-field bound funds half of what they \
             retain together",
        );
        // And the copy still happened: a bound that emptied the field
        // would pass every length check above.
        assert!(!ua.is_empty(), "the enrichment must still carry a value");
        assert!(!page.is_empty(), "the enrichment must still carry a value");
    }

    /// A cut never splits a multi-byte sequence.
    ///
    /// Advisory headers are arbitrary UTF-8 and `String::truncate`
    /// panics mid-sequence. Every offset across a 4-byte run is
    /// exercised, because the panic only happens at the interior ones
    /// and a single hand-picked limit can miss it.
    #[test]
    fn a_cut_header_never_splits_a_character() {
        let multibyte = "\u{1f600}".repeat(64);
        for budget in 0..=64_u32 {
            let headers = RequestHeaders {
                user_agent: Some(&multibyte),
                ..RequestHeaders::default()
            };
            let mut req = BidRequest {
                id: "a".into(),
                ..BidRequest::default()
            };
            run(&mut req, &headers, budget);
            let ua = req
                .device
                .as_ref()
                .and_then(|device| device.ua.as_deref())
                .unwrap_or("");
            assert!(
                ua.len() <= usize::try_from(budget).unwrap_or(usize::MAX),
                "a {budget} B allowance kept {} B",
                ua.len(),
            );
        }
    }

    use super::*;
    use crate::ext::misc::DeviceExt;
    use crate::types::bid_request::{Banner, Imp};
    use crate::types::inventory::App;

    fn base_req() -> BidRequest {
        BidRequest {
            id: "r".into(),
            imp: vec![Imp {
                id: "i1".into(),
                banner: Some(Banner::default()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn etld_plus_one_basic_co_uk() {
        assert_eq!(
            etld_plus_one("foo.bar.example.co.uk").as_deref(),
            Some("example.co.uk"),
        );
    }

    #[test]
    fn etld_plus_one_basic_com() {
        assert_eq!(
            etld_plus_one("news.example.com").as_deref(),
            Some("example.com"),
        );
    }

    #[test]
    fn etld_plus_one_rejects_ip_literal() {
        assert!(etld_plus_one("192.168.1.1").is_none());
    }

    #[test]
    fn etld_plus_one_rejects_localhost() {
        assert!(etld_plus_one("localhost").is_none());
    }

    #[test]
    fn referer_overrides_site_page_for_domain_derivation() {
        // When Referer is present, every empty site field is populated
        // from it — including site.domain and publisher.domain — even
        // if site.page is already set to something else.
        let mut req = base_req();
        req.site = Some(Site {
            page: Some("https://override.example/article/legacy".to_owned()),
            ..Default::default()
        });
        let headers = RequestHeaders {
            referer: Some("https://news.example.com/article/123"),
            ..Default::default()
        };
        run(&mut req, &headers, COPY_BUDGET_NOT_UNDER_TEST);
        let site = req.site.unwrap();
        // Caller-supplied page wins (spec: "only if currently empty").
        assert_eq!(
            site.page.as_deref(),
            Some("https://override.example/article/legacy"),
        );
        // But domain + publisher.domain come from Referer, not page.
        assert_eq!(site.domain.as_deref(), Some("news.example.com"));
        assert_eq!(
            site.publisher.unwrap().domain.as_deref(),
            Some("example.com"),
        );
    }

    #[test]
    fn site_default_populates_publisher_domain_from_referer() {
        let mut req = base_req();
        let headers = RequestHeaders {
            referer: Some("https://news.example.com/article/123"),
            ..Default::default()
        };
        run(&mut req, &headers, COPY_BUDGET_NOT_UNDER_TEST);
        let site = req.site.unwrap();
        assert_eq!(site.domain.as_deref(), Some("news.example.com"));
        assert_eq!(
            site.publisher.unwrap().domain.as_deref(),
            Some("example.com"),
        );
    }

    #[test]
    fn normalize_ip_strips_v4_port() {
        let normalized = normalize_ip("8.8.8.8:1234");
        assert_eq!(normalized.as_deref(), Some("8.8.8.8"));
    }

    #[test]
    fn normalize_ip_rejects_v4_with_non_numeric_port_suffix() {
        // strip_port must only strip when the suffix is a non-empty
        // all-digits port. Otherwise "8.8.8.8:notaport" would silently
        // parse as "8.8.8.8" and be accepted; here the raw input
        // reaches IpAddr::parse which rejects it.
        assert!(normalize_ip("8.8.8.8:notaport").is_none());
    }

    #[test]
    fn normalize_ip_rejects_v4_with_empty_port_suffix() {
        // "8.8.8.8:" has a colon but no port number — refuse to strip.
        assert!(normalize_ip("8.8.8.8:").is_none());
    }

    #[test]
    fn normalize_ip_rejects_bracketed_v6_with_trailing_garbage() {
        // "[2001:4860:4860::8888]junk" previously survived because
        // strip_port returned the host portion ignoring trailing
        // garbage. Now the strip_port returns the raw input so the
        // IpAddr parse fails.
        assert!(normalize_ip("[2001:4860:4860::8888]junk").is_none());
    }

    #[test]
    fn normalize_ip_rejects_bracketed_v6_with_non_numeric_port_suffix() {
        assert!(normalize_ip("[2001:4860:4860::8888]:notaport").is_none());
    }

    #[test]
    fn normalize_ip_strips_bracketed_v6_with_numeric_port() {
        // Counterpart to the rejection tests: real `[v6]:port` form
        // still strips cleanly.
        let normalized = normalize_ip("[2001:4860:4860::8888]:443");
        assert_eq!(normalized.as_deref(), Some("2001:4860:4860::8888"));
    }

    #[test]
    fn normalize_ip_strips_v6_bracket_port_then_rejects_documentation() {
        // The bracket-port form parses out cleanly, but 2001:db8::/32 is
        // RFC 3849 documentation — the public-IP gate must reject it.
        let normalized = normalize_ip("[2001:db8::1]:80");
        assert!(normalized.is_none());
    }

    #[test]
    fn normalize_ip_accepts_real_public_v6() {
        // Google DNS — outside every IANA reserved block.
        let normalized = normalize_ip("2001:4860:4860::8888");
        assert_eq!(normalized.as_deref(), Some("2001:4860:4860::8888"));
    }

    #[test]
    fn normalize_ip_rejects_v6_documentation_block() {
        assert!(normalize_ip("2001:db8::1").is_none());
    }

    #[test]
    fn normalize_ip_rejects_v6_discard_prefix() {
        // RFC 6666 100::/64.
        assert!(normalize_ip("100::1").is_none());
    }

    #[test]
    fn normalize_ip_rejects_v6_site_local() {
        // RFC 3879 deprecated site-local fec0::/10.
        assert!(normalize_ip("fec0::1").is_none());
    }

    #[test]
    fn normalize_ip_trims_whitespace() {
        let normalized = normalize_ip("  8.8.8.8  ");
        assert_eq!(normalized.as_deref(), Some("8.8.8.8"));
    }

    #[test]
    fn normalize_ip_rejects_cgnat() {
        // RFC 6598 100.64.0.0/10 must be rejected as non-public.
        assert!(normalize_ip("100.64.1.1").is_none());
    }

    #[test]
    fn normalize_ip_rejects_documentation_block() {
        // RFC 5737 TEST-NET-1 192.0.2.0/24.
        assert!(normalize_ip("192.0.2.5").is_none());
    }

    #[test]
    fn normalize_ip_rejects_benchmarking_block() {
        // RFC 2544 198.18.0.0/15.
        assert!(normalize_ip("198.19.0.1").is_none());
    }

    #[test]
    fn normalize_ip_rejects_class_e_reserved() {
        assert!(normalize_ip("240.0.0.1").is_none());
    }

    #[test]
    fn normalize_ip_rejects_zero_network() {
        // RFC 1122 0.0.0.0/8 — "this network", source-only.
        assert!(normalize_ip("0.0.0.0").is_none());
        assert!(normalize_ip("0.1.2.3").is_none());
    }

    #[test]
    fn normalize_ip_rejects_v6_ipv4_mapped() {
        // RFC 4291 ::ffff:0:0/96 — IPv4-mapped IPv6 should be rejected
        // in the v6 form so callers can't smuggle a private v4 through
        // a v6 path.
        assert!(normalize_ip("::ffff:10.0.0.1").is_none());
        assert!(normalize_ip("::ffff:192.168.1.1").is_none());
    }

    #[test]
    fn pick_valid_ip_strips_port_from_x_real_ip() {
        let mut req = base_req();
        let headers = RequestHeaders {
            x_real_ip: Some("8.8.8.8:5678"),
            ..Default::default()
        };
        run(&mut req, &headers, COPY_BUDGET_NOT_UNDER_TEST);
        assert_eq!(req.device.unwrap().ip.as_deref(), Some("8.8.8.8"));
    }

    #[test]
    fn at_defaults_when_absent_or_zero() {
        let mut req = base_req();
        run(
            &mut req,
            &RequestHeaders::default(),
            COPY_BUDGET_NOT_UNDER_TEST,
        );
        assert_eq!(req.at, Some(1_i32));

        let mut req_zero = base_req();
        req_zero.at = Some(0_i32);
        run(
            &mut req_zero,
            &RequestHeaders::default(),
            COPY_BUDGET_NOT_UNDER_TEST,
        );
        assert_eq!(req_zero.at, Some(1_i32));

        let mut req_two = base_req();
        req_two.at = Some(2_i32);
        run(
            &mut req_two,
            &RequestHeaders::default(),
            COPY_BUDGET_NOT_UNDER_TEST,
        );
        assert_eq!(req_two.at, Some(2_i32));
    }

    #[test]
    fn channel_app_default_for_app_requests() {
        let mut req = base_req();
        req.site = None;
        req.app = Some(App::default());
        run(
            &mut req,
            &RequestHeaders::default(),
            COPY_BUDGET_NOT_UNDER_TEST,
        );
        let name = req.ext.unwrap().prebid.unwrap().channel.unwrap().name;
        assert_eq!(name.as_deref(), Some("app"));
    }

    #[test]
    fn ios_lmt_override_authorized_sets_zero() {
        let mut req = base_req();
        req.site = None;
        req.app = Some(App::default());
        req.device = Some(Device {
            os: Some("iOS".into()),
            ext: Some(DeviceExt {
                atts: Some(3_i32),
                ..Default::default()
            }),
            ..Default::default()
        });
        run(
            &mut req,
            &RequestHeaders::default(),
            COPY_BUDGET_NOT_UNDER_TEST,
        );
        assert_eq!(req.device.unwrap().lmt, Some(0_i32));
    }

    #[test]
    fn ios_lmt_override_restricted_sets_one() {
        let mut req = base_req();
        req.site = None;
        req.app = Some(App::default());
        req.device = Some(Device {
            os: Some("ios".into()),
            ext: Some(DeviceExt {
                atts: Some(2_i32),
                ..Default::default()
            }),
            ..Default::default()
        });
        run(
            &mut req,
            &RequestHeaders::default(),
            COPY_BUDGET_NOT_UNDER_TEST,
        );
        assert_eq!(req.device.unwrap().lmt, Some(1_i32));
    }

    #[test]
    fn private_ip_in_request_is_cleared() {
        let mut req = base_req();
        req.device = Some(Device {
            ip: Some("10.0.0.1".into()),
            ..Default::default()
        });
        run(
            &mut req,
            &RequestHeaders::default(),
            COPY_BUDGET_NOT_UNDER_TEST,
        );
        assert!(req.device.unwrap().ip.is_none());
    }

    #[test]
    fn body_ip_with_port_is_cleared_then_header_chain_fills() {
        // Body device.ip must be a clean IP literal (port stripping is
        // a header-chain behavior, not a body one). Previously
        // sanitize_ip called normalize_ip and silently passed values
        // like "8.8.8.8:1234" through without writing back — they
        // survived as malformed AND blocked header fill. Now they get
        // cleared and the header chain takes over.
        let mut req = base_req();
        req.device = Some(Device {
            ip: Some("8.8.8.8:1234".into()),
            ..Default::default()
        });
        let headers = RequestHeaders {
            x_real_ip: Some("1.1.1.1"),
            ..Default::default()
        };
        run(&mut req, &headers, COPY_BUDGET_NOT_UNDER_TEST);
        assert_eq!(req.device.unwrap().ip.as_deref(), Some("1.1.1.1"));
    }

    #[test]
    fn body_ip_with_whitespace_is_cleared() {
        let mut req = base_req();
        req.device = Some(Device {
            ip: Some(" 8.8.8.8 ".into()),
            ..Default::default()
        });
        let headers = RequestHeaders {
            x_real_ip: Some("1.1.1.1"),
            ..Default::default()
        };
        run(&mut req, &headers, COPY_BUDGET_NOT_UNDER_TEST);
        assert_eq!(req.device.unwrap().ip.as_deref(), Some("1.1.1.1"));
    }

    #[test]
    fn body_ipv6_with_bracket_port_is_cleared() {
        let mut req = base_req();
        req.device = Some(Device {
            ipv6: Some("[2001:4860:4860::8888]:443".into()),
            ..Default::default()
        });
        let headers = RequestHeaders {
            x_real_ip: Some("2001:4860:4860::8844"),
            ..Default::default()
        };
        run(&mut req, &headers, COPY_BUDGET_NOT_UNDER_TEST);
        let device = req.device.unwrap();
        assert!(device.ipv6.is_some());
        assert_eq!(device.ipv6.as_deref(), Some("2001:4860:4860::8844"));
    }

    #[test]
    fn v4_address_in_ipv6_field_is_cleared_then_header_chain_fills() {
        // device.ipv6 carrying a v4 literal ("8.8.8.8") would pass a
        // family-blind sanitize (parses as IpAddr, public) and block
        // header fill. Family-aware sanitize clears it so the header
        // chain runs.
        let mut req = base_req();
        req.device = Some(Device {
            ipv6: Some("8.8.8.8".into()),
            ..Default::default()
        });
        let headers = RequestHeaders {
            x_real_ip: Some("1.1.1.1"),
            ..Default::default()
        };
        run(&mut req, &headers, COPY_BUDGET_NOT_UNDER_TEST);
        let device = req.device.unwrap();
        assert!(device.ipv6.is_none());
        assert_eq!(device.ip.as_deref(), Some("1.1.1.1"));
    }

    #[test]
    fn v6_address_in_ipv4_field_is_cleared_then_header_chain_fills() {
        let mut req = base_req();
        req.device = Some(Device {
            ip: Some("2001:4860:4860::8888".into()),
            ..Default::default()
        });
        let headers = RequestHeaders {
            x_real_ip: Some("2001:4860:4860::8844"),
            ..Default::default()
        };
        run(&mut req, &headers, COPY_BUDGET_NOT_UNDER_TEST);
        let device = req.device.unwrap();
        assert!(device.ip.is_none());
        assert_eq!(device.ipv6.as_deref(), Some("2001:4860:4860::8844"));
    }

    #[test]
    fn clean_public_body_ip_is_preserved() {
        // Counterpart to the sanitization tests: a clean public IP in
        // the body must NOT be touched (no canonicalization, no header
        // override). Catches the inverse regression where the sanitizer
        // gets too aggressive.
        let mut req = base_req();
        req.device = Some(Device {
            ip: Some("8.8.8.8".into()),
            ..Default::default()
        });
        let headers = RequestHeaders {
            x_real_ip: Some("1.1.1.1"),
            ..Default::default()
        };
        run(&mut req, &headers, COPY_BUDGET_NOT_UNDER_TEST);
        assert_eq!(req.device.unwrap().ip.as_deref(), Some("8.8.8.8"));
    }

    #[test]
    fn public_ip_from_xff_left_to_right() {
        let mut req = base_req();
        let headers = RequestHeaders {
            x_forwarded_for: Some("10.0.0.1, 8.8.8.8, 1.1.1.1"),
            ..Default::default()
        };
        run(&mut req, &headers, COPY_BUDGET_NOT_UNDER_TEST);
        assert_eq!(req.device.unwrap().ip.as_deref(), Some("8.8.8.8"));
    }

    #[test]
    fn sec_gpc_one_overwrites_body_zero() {
        let mut req = base_req();
        req.regs = Some(Regs {
            ext: Some(RegsExt {
                gpc: Some("0".into()),
                ..Default::default()
            }),
            ..Default::default()
        });
        let headers = RequestHeaders {
            sec_gpc: Some("1"),
            ..Default::default()
        };
        run(&mut req, &headers, COPY_BUDGET_NOT_UNDER_TEST);
        assert_eq!(req.regs.unwrap().ext.unwrap().gpc.as_deref(), Some("1"));
    }

    #[test]
    fn site_materialized_even_without_referer() {
        let mut req = base_req();
        run(
            &mut req,
            &RequestHeaders::default(),
            COPY_BUDGET_NOT_UNDER_TEST,
        );
        assert!(req.site.is_some());
        assert_eq!(req.site.unwrap().ext.unwrap().amp, Some(0_i32));
    }

    #[test]
    fn site_not_materialized_when_app_present() {
        let mut req = base_req();
        req.app = Some(App::default());
        run(
            &mut req,
            &RequestHeaders::default(),
            COPY_BUDGET_NOT_UNDER_TEST,
        );
        assert!(req.site.is_none());
    }

    #[test]
    fn ua_dnt_filled_only_when_empty() {
        let mut req = base_req();
        let headers = RequestHeaders {
            user_agent: Some("Mozilla"),
            dnt: Some("1"),
            ..Default::default()
        };
        run(&mut req, &headers, COPY_BUDGET_NOT_UNDER_TEST);
        let device = req.device.unwrap();
        assert_eq!(device.ua.as_deref(), Some("Mozilla"));
        assert_eq!(device.dnt, Some(1_i32));
    }
}
