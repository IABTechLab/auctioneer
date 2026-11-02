use auctioneer_openrtb::types::{bid_request::Imp, field::JsonField};
use serde_json::{Map, Value};

pub(super) fn known<'a>(
    fields: &'a Map<String, Value>,
    name: &str,
) -> Result<Option<&'a Value>, ()> {
    let mut matches = fields
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name));
    let first = matches.next().map(|(_, value)| value);
    if matches.next().is_some() {
        Err(())
    } else {
        Ok(first)
    }
}

pub(super) fn object(value: Option<&Value>) -> Result<Option<&Map<String, Value>>, ()> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Object(fields)) => Ok(Some(fields)),
        Some(_) => Err(()),
    }
}
pub(super) fn text(value: Option<&Value>) -> Result<&str, ()> {
    match value {
        None | Some(Value::Null) => Ok(""),
        Some(Value::String(string)) => Ok(string),
        Some(_) => Err(()),
    }
}

pub(super) fn optional_known<'a>(
    fields: Option<&'a Map<String, Value>>,
    name: &str,
) -> Result<Option<&'a Value>, ()> {
    fields
        .map(|object_fields| known(object_fields, name))
        .transpose()
        .map(Option::flatten)
}
pub(super) fn int(value: Option<&Value>) -> Result<i64, ()> {
    match value {
        None | Some(Value::Null) => Ok(0),
        Some(integer) => integer.as_i64().ok_or(()),
    }
}
fn number(value: Option<&Value>) -> Result<f64, ()> {
    match value {
        None | Some(Value::Null) => Ok(0.0),
        Some(numeric) => numeric.as_f64().filter(|n| n.is_finite()).ok_or(()),
    }
}

// Check deterministic projection failures before a credential bundle is needed.

pub(super) fn int8(value: Option<&Value>) -> Result<i8, ()> {
    i8::try_from(int(value)?).map_err(|_range| ())
}

#[derive(Clone, Copy, Default)]
pub(super) struct VideoParameters {
    pub skip: i64,
    pub skipdelay: i64,
    pub size: i64,
}
#[derive(Clone, Copy)]
pub(super) struct Parameters<'a> {
    pub account: i64,
    pub site: i64,
    pub zone: i64,
    pub inventory: Option<&'a Map<String, Value>>,
    pub visitor: Option<&'a Map<String, Value>>,
    pub keywords: &'a [Value],
    pub multi: bool,
    pub video: VideoParameters,
    pub price_override: f64,
}
pub(super) fn parameters(imp: &Imp) -> Result<Parameters<'_>, ()> {
    let ext = imp.ext.as_ref().ok_or(())?;
    let alias = known(&ext.other, "bidder")?;
    let ((JsonField::Value(bidder), None) | (JsonField::Missing, Some(bidder))) =
        (&ext.bidder, alias)
    else {
        return Err(());
    };
    let fields = bidder.as_object().ok_or(())?;
    let id = |name| {
        let value = known(fields, name)?.ok_or(())?;
        let id = match value {
            Value::String(string)
                if !string.is_empty() && string.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                string.parse::<i64>().map_err(|_error| ())?
            }
            Value::Number(numeric) => numeric.as_i64().ok_or(())?,
            Value::String(_)
            | Value::Null
            | Value::Bool(_)
            | Value::Array(_)
            | Value::Object(_) => return Err(()),
        };
        if id > 0 { Ok(id) } else { Err(()) }
    };
    let video = object(known(fields, "video")?)?;
    text(optional_known(video, "language")?)?;
    for name in ["playerWidth", "playerHeight"] {
        match optional_known(video, name)? {
            None | Some(Value::Null | Value::Number(_)) => {}
            Some(Value::String(value)) if value.parse::<f64>().is_ok_and(f64::is_finite) => {}
            Some(_) => return Err(()),
        }
    }
    let debug = object(known(fields, "debug")?)?;
    let keywords = match known(fields, "keywords")? {
        None | Some(Value::Null) => [].as_slice(),
        Some(Value::Array(values)) if values.iter().all(Value::is_string) => values.as_slice(),
        Some(_) => return Err(()),
    };
    Ok(Parameters {
        account: id("accountId")?,
        site: id("siteId")?,
        zone: id("zoneId")?,
        inventory: object(known(fields, "inventory")?)?,
        visitor: object(known(fields, "visitor")?)?,
        keywords,
        multi: match known(fields, "bidonmultiformat")? {
            None | Some(Value::Null) => false,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err(()),
        },
        video: VideoParameters {
            skip: int(optional_known(video, "skip")?)?,
            skipdelay: int(optional_known(video, "skipdelay")?)?,
            size: int(optional_known(video, "size_id")?)?,
        },
        price_override: number(optional_known(debug, "cpmoverride")?)?,
    })
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::Bidder as _;
    use crate::bidders::rubicon::RubiconBidder;
    use crate::bidders::rubicon::test_support::{banner, context, request, settings};
    use serde_json::json;

    #[test]
    fn credential_preflight_rejects_every_invalid_parameter_without_a_bundle() {
        let settings = settings(&json!({
            "credentials_secret":"fake_credential_bundle","region":"us-east"
        }));
        let mut info = context();
        info.set_settings(Some("rubicon"), Some(&settings), None)
            .unwrap();
        assert!(info.credentials.is_none());
        for invalid in [
            json!(null),
            json!([]),
            json!({}),
            json!({"accountId":0_i64,"siteId":1_i64,"zoneId":1_i64}),
            json!({"accountId":"not-an-id","siteId":1_i64,"zoneId":1_i64}),
            json!({"accountId":"9223372036854775808","siteId":1_i64,"zoneId":1_i64}),
            json!({"accountId":1_i64,"siteId":null,"zoneId":1_i64}),
            json!({"accountId":1_i64,"siteId":1_i64,"zoneId":1_i64,"video":{"size_id":"bad"}}),
        ] {
            let mut imp = banner("invalid");
            imp["ext"]["bidder"] = invalid;
            let request = request(&json!([imp]));
            let before = serde_json::to_value(&request).unwrap();
            let error = RubiconBidder.credential_key(&request, &info).unwrap_err();
            assert_eq!(error.code, 2);
            assert_eq!(error.message, "rubicon no eligible impression");
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
        let error = RubiconBidder
            .credential_key(&request(&json!([])), &info)
            .unwrap_err();
        assert_eq!(error.code, 2);
        assert_eq!(error.message, "rubicon no eligible impression");
    }
}
