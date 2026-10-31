//! Bounded callback and bidder URL construction. Request values are never
//! parsed as template source after substitution.

use auctioneer_config::usersync::{
    MAX_SYNC_QUERY_PARAMS, SyncType, SyncerEndpoint, UserSyncConfig, validate_sync_query_encoding,
    validate_url_spelling, validate_user_macro, validate_user_sync,
};
use url::{Url, form_urlencoded};

const MAX_TEMPLATE_BYTES: usize = 8 * 1024;
const MAX_STAGE_BYTES: usize = 32 * 1024;
const DEFAULT_CALLBACK: &str = "{{.ExternalURL}}/setuid?bidder={{.SyncerKey}}&gdpr={{.GDPR}}&gdpr_consent={{.GDPRConsent}}&us_privacy={{.USPrivacy}}&gpp={{.GPP}}&gpp_sid={{.GPPSID}}&f={{.SyncType}}&uid={{.UserMacro}}";

#[derive(Clone, Copy)]
pub struct SyncPrivacyMacros<'a> {
    pub gdpr: &'a str,
    pub gdpr_consent: &'a str,
    pub us_privacy: &'a str,
    pub gpp: &'a str,
    pub gpp_sid: &'a str,
}

#[derive(Clone, Copy)]
pub struct SyncTemplateInput<'a> {
    pub host: &'a UserSyncConfig,
    pub endpoint: &'a SyncerEndpoint,
    pub callback_bidder: &'a str,
    pub bidder_name: &'a str,
    pub sync_type: SyncType,
    pub account: Option<&'a str>,
    pub privacy: SyncPrivacyMacros<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TemplateError {
    InvalidConfig,
    InvalidTemplate,
    InvalidCallback,
    InvalidEndpoint,
    OutputTooLong,
}

pub struct BuiltSyncUrl {
    pub url: String,
    pub callback_url: String,
    pub user_macro: String,
}

fn encoded(value: &str) -> Result<String, TemplateError> {
    let size =
        form_urlencoded::byte_serialize(value.as_bytes()).try_fold(0_usize, |size, part| {
            size.checked_add(part.len())
                .filter(|total| *total <= MAX_STAGE_BYTES)
                .ok_or(TemplateError::OutputTooLong)
        })?;
    let mut out = String::with_capacity(size);
    for part in form_urlencoded::byte_serialize(value.as_bytes()) {
        out.push_str(part);
    }
    Ok(out)
}

fn push_bounded(out: &mut String, part: &str) -> Result<(), TemplateError> {
    let size = out
        .len()
        .checked_add(part.len())
        .ok_or(TemplateError::OutputTooLong)?;
    if size > MAX_STAGE_BYTES {
        return Err(TemplateError::OutputTooLong);
    }
    // Geometric String growth must not exceed the checked stage capacity.
    if out.capacity() < size {
        out.reserve_exact(part.len());
    }
    out.push_str(part);
    Ok(())
}

fn render(
    template: &str,
    mut resolve: impl FnMut(&str) -> Result<String, TemplateError>,
) -> Result<String, TemplateError> {
    if template.is_empty()
        || template.len() > MAX_TEMPLATE_BYTES
        || template.bytes().any(|byte| byte < 0x20 || byte == 0x7f)
    {
        return Err(TemplateError::InvalidTemplate);
    }
    let mut out = String::new();
    let mut rest = template;
    while let Some((literal, after_open)) = rest.split_once("{{") {
        if literal.contains("}}") {
            return Err(TemplateError::InvalidTemplate);
        }
        push_bounded(&mut out, literal)?;
        let (name, after_close) = after_open
            .split_once("}}")
            .ok_or(TemplateError::InvalidTemplate)?;
        let value = resolve(name)?;
        push_bounded(&mut out, &value)?;
        rest = after_close;
    }
    if rest.contains("}}") {
        return Err(TemplateError::InvalidTemplate);
    }
    push_bounded(&mut out, rest)?;
    Ok(out)
}

fn callback_with_account(
    callback: &str,
    callback_bidder: &str,
    user_macro: &str,
    account: Option<&str>,
) -> Result<String, TemplateError> {
    let (_, query) = callback
        .split_once('?')
        .ok_or(TemplateError::InvalidCallback)?;
    let final_parameter_count = query
        .split('&')
        .count()
        .checked_add(usize::from(account.is_some()))
        .ok_or(TemplateError::InvalidCallback)?;
    if final_parameter_count > MAX_SYNC_QUERY_PARAMS {
        return Err(TemplateError::InvalidCallback);
    }
    validate_sync_query_encoding(query).map_err(|_error| TemplateError::InvalidCallback)?;
    let query_start = callback
        .len()
        .checked_sub(query.len())
        .ok_or(TemplateError::InvalidCallback)?;
    let mut uid_offset = None;
    let mut bidder_seen = false;
    let mut cursor = 0_usize;
    for field in query.split('&') {
        let field_end = cursor
            .checked_add(field.len())
            .ok_or(TemplateError::InvalidCallback)?;
        let (raw_key, value) = field.split_once('=').unwrap_or((field, ""));
        let decoded = form_urlencoded::parse(field.as_bytes()).next();
        if decoded.as_ref().is_some_and(|(key, _)| key == "account") {
            return Err(TemplateError::InvalidCallback);
        }
        if let Some((key, bidder)) = &decoded
            && key == "bidder"
        {
            if bidder_seen || bidder != callback_bidder {
                return Err(TemplateError::InvalidCallback);
            }
            bidder_seen = true;
        }
        if decoded.as_ref().is_some_and(|(key, _)| key == "uid") {
            if uid_offset.is_some() || raw_key != "uid" || value != user_macro {
                return Err(TemplateError::InvalidCallback);
            }
            uid_offset = Some(
                query_start
                    .checked_add(cursor)
                    .ok_or(TemplateError::InvalidCallback)?,
            );
            if field_end != query.len() {
                return Err(TemplateError::InvalidCallback);
            }
        }
        cursor = field_end
            .checked_add(1)
            .ok_or(TemplateError::InvalidCallback)?;
    }
    if !bidder_seen {
        return Err(TemplateError::InvalidCallback);
    }
    let uid_start = uid_offset.ok_or(TemplateError::InvalidCallback)?;
    let Some(account_id) = account else {
        return Ok(callback.to_owned());
    };
    if account_id.is_empty() {
        return Err(TemplateError::InvalidCallback);
    }
    let encoded_account = encoded(account_id)?;
    let (prefix, suffix) = callback
        .split_at_checked(uid_start)
        .ok_or(TemplateError::InvalidCallback)?;
    let mut out = String::new();
    push_bounded(&mut out, prefix)?;
    push_bounded(&mut out, "account=")?;
    push_bounded(&mut out, &encoded_account)?;
    push_bounded(&mut out, "&")?;
    push_bounded(&mut out, suffix)?;
    Ok(out)
}

fn validate_emitted_endpoint(template: &str, emitted: &str) -> Result<(), TemplateError> {
    let (scheme, rest) = template
        .split_once("://")
        .ok_or(TemplateError::InvalidEndpoint)?;
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .ok_or(TemplateError::InvalidEndpoint)?;
    if authority.is_empty() || authority.contains("{{") || authority.contains("}}") {
        return Err(TemplateError::InvalidEndpoint);
    }
    let authority_base = format!("{scheme}://{authority}");
    let safety = UserSyncConfig {
        external_url: authority_base,
        ..UserSyncConfig::default()
    };
    validate_user_sync(&safety, cfg!(feature = "insecure-bidders"))
        .map_err(|_error| TemplateError::InvalidEndpoint)?;
    let parsed = Url::parse(emitted).map_err(|_error| TemplateError::InvalidEndpoint)?;
    let expected = Url::parse(&format!("{scheme}://{authority}/"))
        .map_err(|_error| TemplateError::InvalidEndpoint)?;
    validate_url_spelling(template, &expected).map_err(|_error| TemplateError::InvalidEndpoint)?;
    validate_url_spelling(emitted, &parsed).map_err(|_error| TemplateError::InvalidEndpoint)?;
    if parsed.scheme() != expected.scheme()
        || parsed.host() != expected.host()
        || parsed.port() != expected.port()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.fragment().is_some()
    {
        return Err(TemplateError::InvalidEndpoint);
    }
    Ok(())
}

/// # Errors
/// Rejects invalid configuration, templates, callback/endpoint URLs, or an
/// encoding/substitution stage exceeding the URL byte limit.
pub fn build_sync_url(input: SyncTemplateInput<'_>) -> Result<BuiltSyncUrl, TemplateError> {
    validate_user_sync(input.host, cfg!(feature = "insecure-bidders"))
        .map_err(|_error| TemplateError::InvalidConfig)?;
    if input.callback_bidder.is_empty()
        || input.callback_bidder.len() > 256
        || input.bidder_name.is_empty()
        || input.bidder_name.len() > 256
    {
        return Err(TemplateError::InvalidConfig);
    }
    let user_macro = input.endpoint.user_macro.as_deref().unwrap_or("");
    if validate_user_macro(input.endpoint.user_macro.as_deref()).is_err() {
        return Err(TemplateError::InvalidTemplate);
    }
    let sync_type = match input.sync_type {
        SyncType::Iframe => "b",
        SyncType::Redirect => "i",
    };
    let callback_template = input
        .host
        .redirect_url
        .as_deref()
        .unwrap_or(DEFAULT_CALLBACK);
    if !user_macro.is_empty()
        && !callback_template.contains("?uid={{.UserMacro}}")
        && !callback_template.contains("&uid={{.UserMacro}}")
    {
        return Err(TemplateError::InvalidCallback);
    }
    let rendered_callback = render(callback_template, |name| {
        Ok(match name {
            ".ExternalURL" => input.host.external_url.trim_end_matches('/').to_owned(),
            ".SyncerKey" => encoded(input.callback_bidder)?,
            ".BidderName" => encoded(input.bidder_name)?,
            ".SyncType" => sync_type.to_owned(),
            ".UserMacro" => user_macro.to_owned(),
            ".GDPR" => encoded(input.privacy.gdpr)?,
            ".GDPRConsent" => encoded(input.privacy.gdpr_consent)?,
            ".USPrivacy" => encoded(input.privacy.us_privacy)?,
            ".GPP" => encoded(input.privacy.gpp)?,
            ".GPPSID" => encoded(input.privacy.gpp_sid)?,
            _ => return Err(TemplateError::InvalidTemplate),
        })
    })?;
    let callback = callback_with_account(
        &rendered_callback,
        input.callback_bidder,
        user_macro,
        input.account,
    )?;
    if callback.len() > MAX_STAGE_BYTES {
        return Err(TemplateError::OutputTooLong);
    }
    let escaped_callback = encoded(&callback)?;
    let url = render(&input.endpoint.url, |name| {
        Ok(match name {
            ".RedirectURL" => escaped_callback.clone(),
            ".GDPR" => encoded(input.privacy.gdpr)?,
            ".GDPRConsent" => encoded(input.privacy.gdpr_consent)?,
            ".USPrivacy" => encoded(input.privacy.us_privacy)?,
            ".GPP" => encoded(input.privacy.gpp)?,
            ".GPPSID" => encoded(input.privacy.gpp_sid)?,
            _ => return Err(TemplateError::InvalidTemplate),
        })
    })?;
    validate_emitted_endpoint(&input.endpoint.url, &url)?;
    Ok(BuiltSyncUrl {
        url,
        callback_url: callback,
        user_macro: user_macro.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        SyncPrivacyMacros, SyncTemplateInput, TemplateError, build_sync_url, callback_with_account,
        validate_emitted_endpoint,
    };
    use auctioneer_config::usersync::{
        MAX_SYNC_QUERY_PARAMS, SyncType, SyncerEndpoint, UserSyncConfig,
    };
    use url::form_urlencoded;

    #[test]
    fn rendered_callback_parameter_limit_includes_inserted_named_account() {
        for count in [
            MAX_SYNC_QUERY_PARAMS.saturating_sub(1),
            MAX_SYNC_QUERY_PARAMS,
            MAX_SYNC_QUERY_PARAMS.saturating_add(1),
        ] {
            let padding = "extra=x&".repeat(count - 2);
            let callback =
                format!("https://sync.example.com/setuid?bidder=fake_bbb&{padding}uid=#PMUID");
            for account in [None, Some("publisher")] {
                let result = callback_with_account(&callback, "fake_bbb", "#PMUID", account);
                let final_count = count.saturating_add(usize::from(account.is_some()));
                if final_count > MAX_SYNC_QUERY_PARAMS {
                    assert_eq!(
                        result,
                        Err(TemplateError::InvalidCallback),
                        "{count}, {account:?}"
                    );
                } else {
                    let rendered = result.expect("final query at or below the receiver limit");
                    let query = rendered.split_once('?').unwrap().1;
                    assert_eq!(query.split('&').count(), final_count);
                    assert_eq!(
                        form_urlencoded::parse(query.as_bytes())
                            .filter(|(key, _)| key == "account")
                            .count(),
                        usize::from(account.is_some())
                    );
                    assert!(rendered.ends_with("&uid=#PMUID"));
                }
            }
        }
    }

    #[test]
    fn rendered_callback_rejects_malformed_query_encoding_before_account_insertion() {
        for field in [
            "extra=%",
            "extra=%0",
            "extra=%GG",
            "extra=%FF",
            "extra=%C3%28",
            "extra%=x",
            "extra%FF=x",
        ] {
            let callback =
                format!("https://sync.example.com/setuid?bidder=fake_bbb&{field}&uid=#PMUID");
            for account in [None, Some("publisher")] {
                assert_eq!(
                    callback_with_account(&callback, "fake_bbb", "#PMUID", account),
                    Err(TemplateError::InvalidCallback),
                    "{field}, {account:?}"
                );
            }
        }
    }

    #[test]
    fn rendered_callback_preserves_valid_utf8_query_encoding() {
        let callback =
            "https://sync.example.com/setuid?bidder=fake_bbb&extra=%C3%A9%26%2B&uid=#PMUID";
        assert_eq!(
            callback_with_account(callback, "fake_bbb", "#PMUID", None).unwrap(),
            callback
        );
        assert_eq!(
            callback_with_account(callback, "fake_bbb", "#PMUID", Some("publisher%26+")).unwrap(),
            "https://sync.example.com/setuid?bidder=fake_bbb&extra=%C3%A9%26%2B&account=publisher%2526%2B&uid=#PMUID"
        );
    }

    #[test]
    fn rendered_callback_rejects_account_field_for_default_selection() {
        for query in [
            "account=forged&bidder=fake_bbb&uid=#PMUID",
            "bidder=fake_bbb&account=&uid=#PMUID",
            "bidder=fake_bbb&%61ccount=forged&uid=#PMUID",
            "bidder=fake_bbb&acc%6Funt=forged&uid=#PMUID",
            "bidder=fake_bbb&account=one&account=two&uid=#PMUID",
        ] {
            let callback = format!("https://sync.example.com/setuid?{query}");
            assert_eq!(
                callback_with_account(&callback, "fake_bbb", "#PMUID", None),
                Err(TemplateError::InvalidCallback),
                "{query}"
            );
        }
    }

    #[test]
    fn rendered_callback_rejects_earlier_account_even_when_it_matches_named_selection() {
        for field in [
            "account=forged",
            "account=publisher+%26+account",
            "account=",
            "%61ccount=publisher+%26+account",
            "account=forged&%61ccount=publisher+%26+account",
        ] {
            let callback =
                format!("https://sync.example.com/setuid?bidder=fake_bbb&{field}&uid=#PMUID");
            assert_eq!(
                callback_with_account(&callback, "fake_bbb", "#PMUID", Some("publisher & account"),),
                Err(TemplateError::InvalidCallback),
                "{field}"
            );
        }
    }

    #[test]
    fn rendered_callback_account_is_inserted_only_from_named_selection() {
        let callback = "https://sync.example.com/setuid?%62idder=fake_bbb&uid=#PMUID";
        assert_eq!(
            callback_with_account(callback, "fake_bbb", "#PMUID", None).unwrap(),
            callback
        );
        assert_eq!(
            callback_with_account(callback, "fake_bbb", "#PMUID", Some("")),
            Err(TemplateError::InvalidCallback)
        );
        let named = callback_with_account(
            callback,
            "fake_bbb",
            "#PMUID",
            Some("publisher & account%26+"),
        )
        .unwrap();
        assert_eq!(
            named,
            "https://sync.example.com/setuid?%62idder=fake_bbb&account=publisher+%26+account%2526%2B&uid=#PMUID"
        );
        let substituted = named.replace("#PMUID", "real-id");
        let parsed = url::Url::parse(&substituted).unwrap();
        let pairs: Vec<_> = parsed.query_pairs().collect();
        assert_eq!(pairs.len(), 3);
        assert_eq!(
            pairs[1],
            ("account".into(), "publisher & account%26+".into())
        );
        assert_eq!(pairs[2], ("uid".into(), "real-id".into()));
    }

    #[test]
    fn rendered_callback_requires_one_selected_bidder_and_terminal_uid() {
        for query in [
            "uid=#PMUID",
            "bidder=fake_aaa&uid=#PMUID",
            "bidder=fake_bbb&%62idder=fake_bbb&uid=#PMUID",
            "bidder=fake_bbb&uid=STOLEN",
            "bidder=fake_bbb&%75id=#PMUID",
            "bidder=fake_bbb&%75id=STOLEN&uid=#PMUID",
            "bidder=fake_bbb&uid=#PMUID&f=i",
        ] {
            let callback = format!("https://sync.example.com/setuid?{query}");
            assert_eq!(
                callback_with_account(&callback, "fake_bbb", "#PMUID", None),
                Err(TemplateError::InvalidCallback),
                "{query}"
            );
        }
    }

    fn assert_emitted_endpoint_refused(emitted: &str) {
        let template = "https://bidder.example.com:8443/sync?back={{.RedirectURL}}";
        assert_eq!(
            validate_emitted_endpoint(template, "https://bidder.example.com:8443/sync?back=safe"),
            Ok(())
        );
        assert_eq!(
            validate_emitted_endpoint(template, emitted),
            Err(TemplateError::InvalidEndpoint),
            "{emitted}"
        );
    }

    #[test]
    fn emitted_endpoint_rejects_changed_scheme() {
        assert_emitted_endpoint_refused("http://bidder.example.com:8443/sync?back=safe");
    }

    #[test]
    fn emitted_endpoint_rejects_changed_host() {
        assert_emitted_endpoint_refused("https://forged.example.com:8443/sync?back=safe");
    }

    #[test]
    fn emitted_endpoint_rejects_changed_port() {
        assert_emitted_endpoint_refused("https://bidder.example.com:9443/sync?back=safe");
    }

    #[test]
    fn emitted_endpoint_rejects_username() {
        assert_emitted_endpoint_refused("https://forged@bidder.example.com:8443/sync?back=safe");
    }

    #[test]
    fn emitted_endpoint_rejects_password_without_username() {
        assert_emitted_endpoint_refused("https://:forged@bidder.example.com:8443/sync?back=safe");
    }

    #[test]
    fn emitted_endpoint_rejects_fragment() {
        assert_emitted_endpoint_refused("https://bidder.example.com:8443/sync?back=safe#forged");
    }

    #[test]
    fn emitted_endpoint_rejects_parser_repaired_url_spellings() {
        for emitted in [
            "https:///bidder.example.com:8443/sync?back=safe",
            "https:////bidder.example.com:8443/sync?back=safe",
            "https://bidder.example.com:8443\\sync?back=safe",
            "https://bidder.example.com:8443/base\\sync?back=safe",
            "https://bidder.example.com:8443/base path?back=safe",
        ] {
            assert_emitted_endpoint_refused(emitted);
        }
    }

    #[test]
    fn template_spelling_is_checked_even_when_emitted_endpoint_is_canonical() {
        for template in [
            "https:///bidder.example.com:8443/sync?back={{.RedirectURL}}",
            "https://bidder.example.com:8443/base\\sync?back={{.RedirectURL}}",
            "https://bidder.example.com:8443/base path?back={{.RedirectURL}}",
        ] {
            assert_eq!(
                validate_emitted_endpoint(
                    template,
                    "https://bidder.example.com:8443/sync?back=safe"
                ),
                Err(TemplateError::InvalidEndpoint),
                "{template:?}"
            );
        }
    }

    #[test]
    fn bidder_name_callback_binding_must_match_selected_callback_identity() {
        let host = UserSyncConfig {
            redirect_url: Some(
                "{{.ExternalURL}}/setuid?bidder={{.BidderName}}&uid={{.UserMacro}}".to_owned(),
            ),
            ..UserSyncConfig::default()
        };
        let endpoint = SyncerEndpoint {
            url: "https://bidder.example.com/sync?back={{.RedirectURL}}".to_owned(),
            user_macro: Some("#PMUID".to_owned()),
        };
        let result = build_sync_url(SyncTemplateInput {
            host: &host,
            endpoint: &endpoint,
            callback_bidder: "fake_bbb",
            bidder_name: "fake_aaa",
            sync_type: SyncType::Redirect,
            account: None,
            privacy: SyncPrivacyMacros {
                gdpr: "",
                gdpr_consent: "",
                us_privacy: "",
                gpp: "",
                gpp_sid: "",
            },
        });
        assert!(matches!(result, Err(TemplateError::InvalidCallback)));
    }

    #[test]
    fn inserted_privacy_is_data_not_another_macro_or_query_field() {
        let host = UserSyncConfig {
            external_url: "https://sync.example.com".to_owned(),
            ..UserSyncConfig::default()
        };
        let endpoint = SyncerEndpoint {
            url: "https://bidder.example.com/sync?back={{.RedirectURL}}".to_owned(),
            user_macro: Some("$UID".to_owned()),
        };
        let built = build_sync_url(SyncTemplateInput {
            host: &host,
            endpoint: &endpoint,
            callback_bidder: "fake_bidder",
            bidder_name: "fake_bidder",
            sync_type: SyncType::Redirect,
            account: None,
            privacy: SyncPrivacyMacros {
                gdpr: "",
                gdpr_consent: "{{.UserMacro}}&account=forged",
                us_privacy: "",
                gpp: "",
                gpp_sid: "",
            },
        })
        .expect("data must be safely encoded");
        let callback = url::Url::parse(&built.callback_url).expect("callback URL");
        let values: Vec<_> = callback.query_pairs().collect();
        assert_eq!(
            values
                .iter()
                .find(|(key, _)| key == "gdpr_consent")
                .unwrap()
                .1,
            "{{.UserMacro}}&account=forged"
        );
        assert!(!values.iter().any(|(key, _)| key == "account"));
        assert_eq!(
            values.iter().find(|(key, _)| key == "uid").unwrap().1,
            "$UID"
        );
    }
}
