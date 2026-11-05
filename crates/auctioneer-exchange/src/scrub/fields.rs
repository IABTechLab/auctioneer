//! Exhaustive model accessors; privacy dispositions come only from compiled IDs.

use super::ChangeKind;
use super::decision::{Action, GeoMode, Input, action, clear_object};
use super::value::{self, Mutation, Transformed};
use crate::AuctionError;
use auctioneer_config_data::disclosure::{FieldId, SchemaId, Shape, Transform};
use auctioneer_openrtb::ext::imp::ImpExt;
use auctioneer_openrtb::ext::misc::{DeviceExt, DeviceExtPrebid, DeviceInterstitial, UserExt};
use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};
use auctioneer_openrtb::types::device::{BrandVersion, BrandVersionFields, Device, Geo, UserAgent};
use auctioneer_openrtb::types::field::JsonField;
use auctioneer_openrtb::types::source::Source;
use auctioneer_openrtb::types::user::{Eid, EidUid, User};
use serde_json::{Map, Value};

fn typed_input(field: FieldId, shape: Shape) -> Input {
    if field.descriptor().shape == shape {
        Input::Valid
    } else {
        Input::Invalid
    }
}

fn option<T>(
    slot: &mut Option<T>,
    schema: SchemaId,
    field: FieldId,
    shape: Shape,
    mutation: &mut Mutation<'_>,
) -> Action {
    let input = if slot.is_some() {
        typed_input(field, shape)
    } else {
        Input::Missing
    };
    let result = action(Some(field.descriptor()), input, mutation.disclosure);
    if let Action::Remove(kind) = result {
        *slot = None;
        mutation.record(schema, kind);
    }
    result
}

fn json<T>(
    slot: &mut JsonField<T>,
    schema: SchemaId,
    field: FieldId,
    shape: Shape,
    mutation: &mut Mutation<'_>,
) -> Action {
    let input = match slot {
        JsonField::Missing => Input::Missing,
        JsonField::Null => Input::Null,
        JsonField::Value(_) => typed_input(field, shape),
    };
    let result = action(Some(field.descriptor()), input, mutation.disclosure);
    if let Action::Remove(kind) = result {
        *slot = JsonField::Missing;
        mutation.record(schema, kind);
    }
    result
}

fn option_child<T>(
    slot: &mut Option<T>,
    schema: SchemaId,
    field: FieldId,
    visitor: impl FnOnce(&mut T, &mut Mutation<'_>) -> Result<(), AuctionError>,
    mutation: &mut Mutation<'_>,
) -> Result<(), AuctionError> {
    if let Action::Visit(_) = option(slot, schema, field, Shape::Object, mutation)
        && let Some(child) = slot.as_mut()
    {
        visitor(child, mutation)?;
    }
    Ok(())
}

fn json_child<T>(
    slot: &mut JsonField<T>,
    schema: SchemaId,
    field: FieldId,
    visitor: impl FnOnce(&mut T, &mut Mutation<'_>) -> Result<(), AuctionError>,
    mutation: &mut Mutation<'_>,
) -> Result<(), AuctionError> {
    if let Action::Visit(_) = json(slot, schema, field, Shape::Object, mutation)
        && let Some(child) = slot.as_mut()
    {
        visitor(child, mutation)?;
    }
    Ok(())
}

fn geo_shadow(
    typed: Option<&Geo>,
    other: &mut Map<String, Value>,
    schema: SchemaId,
    mutation: &mut Mutation<'_>,
) {
    if typed.is_some()
        && !matches!(mutation.disclosure.geo, GeoMode::Pass)
        && other.remove("geo").is_some()
    {
        let kind = if matches!(mutation.disclosure.geo, GeoMode::RemoveAll) {
            ChangeKind::Coppa
        } else {
            ChangeKind::Geo
        };
        mutation.record(schema, kind);
    }
}

fn ip(
    slot: &mut Option<String>,
    other: &mut Map<String, Value>,
    field: FieldId,
    mutation: &mut Mutation<'_>,
) -> Result<(), AuctionError> {
    if let Action::Transform(transform @ (Transform::Ipv4 | Transform::Ipv6)) =
        option(slot, SchemaId::Device, field, Shape::String, mutation)
    {
        // An owning geo restriction closes conflicting same-slot occurrences.
        // Shadow-only addresses still run the identical map transform.
        if slot.is_some() && other.remove(field.descriptor().name).is_some() {
            mutation.record(SchemaId::Device, ChangeKind::Geo);
        }
        if let Some(contents) = slot.as_mut() {
            match value::mask_string(contents, transform, mutation)? {
                Transformed::Unchanged => {}
                Transformed::Changed => mutation.record(SchemaId::Device, ChangeKind::Coarsen),
                Transformed::Invalid => {
                    *slot = None;
                    mutation.record(SchemaId::Device, ChangeKind::Malformed);
                }
            }
        }
    }
    Ok(())
}

#[expect(
    clippy::float_cmp,
    reason = "audit records only an actual coordinate value change"
)]
fn coordinate(slot: &mut Option<f64>, field: FieldId, mutation: &mut Mutation<'_>) {
    if let Action::Transform(Transform::Coordinate) =
        option(slot, SchemaId::Geo, field, Shape::Number, mutation)
        && let Some(original) = *slot
    {
        if let Some(rounded) = value::coordinate(original) {
            if original != rounded {
                *slot = Some(rounded);
                mutation.record(SchemaId::Geo, ChangeKind::Coarsen);
            }
        } else {
            *slot = None;
            mutation.record(SchemaId::Geo, ChangeKind::Malformed);
        }
    }
}

pub(crate) fn user(user: &mut User, mutation: &mut Mutation<'_>) -> Result<(), AuctionError> {
    let User {
        id,
        buyeruid,
        yob,
        gender,
        keywords,
        consent,
        eids,
        geo,
        data,
        ext,
        other,
    } = user;
    let schema = SchemaId::User;
    option(id, schema, FieldId::UserId, Shape::String, mutation);
    option(
        buyeruid,
        schema,
        FieldId::UserBuyeruid,
        Shape::String,
        mutation,
    );
    option(yob, schema, FieldId::UserYob, Shape::Integer, mutation);
    option(gender, schema, FieldId::UserGender, Shape::String, mutation);
    option(
        keywords,
        schema,
        FieldId::UserKeywords,
        Shape::String,
        mutation,
    );
    option(
        consent,
        schema,
        FieldId::UserConsent,
        Shape::String,
        mutation,
    );
    let input = if eids.is_empty() {
        Input::Missing
    } else {
        typed_input(FieldId::UserEids, Shape::ObjectArray)
    };
    match action(
        Some(FieldId::UserEids.descriptor()),
        input,
        mutation.disclosure,
    ) {
        Action::Remove(kind) => {
            eids.clear();
            mutation.record(schema, kind);
        }
        Action::Visit(_) => {
            for entry in eids {
                eid(entry, mutation)?;
            }
        }
        Action::Keep | Action::Transform(_) => {}
    }
    geo_shadow(geo.as_ref(), other, schema, mutation);
    option_child(geo, schema, FieldId::UserGeo, self::geo, mutation)?;
    value::slot(data, schema, FieldId::UserData.descriptor(), mutation)?;
    option_child(ext, schema, FieldId::UserExt, user_ext, mutation)?;
    value::map(other, schema, mutation)
}

fn user_ext(ext: &mut UserExt, mutation: &mut Mutation<'_>) -> Result<(), AuctionError> {
    let UserExt { data, eids, other } = ext;
    let schema = SchemaId::UserExt;
    value::slot(data, schema, FieldId::UserExtData.descriptor(), mutation)?;
    value::slot(eids, schema, FieldId::UserExtEids.descriptor(), mutation)?;
    value::map(other, schema, mutation)
}

#[expect(
    clippy::too_many_lines,
    reason = "one exhaustive Device adapter keeps every typed slot and sparse field compile-checked"
)]
pub(crate) fn device(device: &mut Device, mutation: &mut Mutation<'_>) -> Result<(), AuctionError> {
    let Device {
        ua,
        sua,
        ip: ipv4,
        ipv6,
        geo,
        dnt,
        lmt,
        os,
        devicetype,
        make,
        model,
        osv,
        hwv,
        h,
        w,
        ppi,
        pxratio,
        js,
        geofetch,
        flashver,
        language,
        langb,
        carrier,
        mccmnc,
        connectiontype,
        ifa,
        ext,
        other,
    } = device;
    let schema = SchemaId::Device;
    option(ua, schema, FieldId::DeviceUa, Shape::String, mutation);
    option(dnt, schema, FieldId::DeviceDnt, Shape::Integer, mutation);
    option(lmt, schema, FieldId::DeviceLmt, Shape::Integer, mutation);
    option(os, schema, FieldId::DeviceOs, Shape::String, mutation);
    option(ifa, schema, FieldId::DeviceIfa, Shape::String, mutation);
    json(
        devicetype,
        schema,
        FieldId::DeviceDevicetype,
        Shape::Integer,
        mutation,
    );
    json(make, schema, FieldId::DeviceMake, Shape::String, mutation);
    json(model, schema, FieldId::DeviceModel, Shape::String, mutation);
    json(osv, schema, FieldId::DeviceOsv, Shape::String, mutation);
    json(hwv, schema, FieldId::DeviceHwv, Shape::String, mutation);
    json(h, schema, FieldId::DeviceH, Shape::Integer, mutation);
    json(w, schema, FieldId::DeviceW, Shape::Integer, mutation);
    json(ppi, schema, FieldId::DevicePpi, Shape::Integer, mutation);
    json(
        pxratio,
        schema,
        FieldId::DevicePxratio,
        Shape::Number,
        mutation,
    );
    json(js, schema, FieldId::DeviceJs, Shape::Integer, mutation);
    json(
        geofetch,
        schema,
        FieldId::DeviceGeofetch,
        Shape::Integer,
        mutation,
    );
    json(
        flashver,
        schema,
        FieldId::DeviceFlashver,
        Shape::String,
        mutation,
    );
    json(
        language,
        schema,
        FieldId::DeviceLanguage,
        Shape::String,
        mutation,
    );
    json(langb, schema, FieldId::DeviceLangb, Shape::String, mutation);
    json(
        carrier,
        schema,
        FieldId::DeviceCarrier,
        Shape::String,
        mutation,
    );
    json(
        mccmnc,
        schema,
        FieldId::DeviceMccmnc,
        Shape::String,
        mutation,
    );
    json(
        connectiontype,
        schema,
        FieldId::DeviceConnectiontype,
        Shape::Integer,
        mutation,
    );
    ip(ipv4, other, FieldId::DeviceIp, mutation)?;
    ip(ipv6, other, FieldId::DeviceIpv6, mutation)?;
    geo_shadow(geo.as_ref(), other, schema, mutation);
    option_child(geo, schema, FieldId::DeviceGeo, self::geo, mutation)?;
    json_child(
        sua,
        schema,
        FieldId::DeviceSua,
        |child, child_mutation| user_agent(child, child_mutation),
        mutation,
    )?;
    option_child(ext, schema, FieldId::DeviceExt, device_ext, mutation)?;
    value::map(other, schema, mutation)
}

fn device_ext(ext: &mut DeviceExt, mutation: &mut Mutation<'_>) -> Result<(), AuctionError> {
    let DeviceExt {
        atts,
        prebid,
        other,
    } = ext;
    let schema = SchemaId::DeviceExt;
    option(
        atts,
        schema,
        FieldId::DeviceExtAtts,
        Shape::Integer,
        mutation,
    );
    json_child(
        prebid,
        schema,
        FieldId::DeviceExtPrebid,
        device_prebid,
        mutation,
    )?;
    value::map(other, schema, mutation)
}

fn device_prebid(
    prebid: &mut DeviceExtPrebid,
    mutation: &mut Mutation<'_>,
) -> Result<(), AuctionError> {
    let DeviceExtPrebid {
        interstitial,
        other,
    } = prebid;
    let schema = SchemaId::DeviceExtPrebid;
    json_child(
        interstitial,
        schema,
        FieldId::DeviceExtPrebidInterstitial,
        device_interstitial,
        mutation,
    )?;
    value::map(other, schema, mutation)
}

fn device_interstitial(
    interstitial: &mut DeviceInterstitial,
    mutation: &mut Mutation<'_>,
) -> Result<(), AuctionError> {
    let DeviceInterstitial {
        minwidthperc,
        minheightperc,
        other,
    } = interstitial;
    let schema = SchemaId::DeviceInterstitial;
    json(
        minwidthperc,
        schema,
        FieldId::DeviceInterstitialMinwidthperc,
        Shape::Integer,
        mutation,
    );
    json(
        minheightperc,
        schema,
        FieldId::DeviceInterstitialMinheightperc,
        Shape::Integer,
        mutation,
    );
    value::map(other, schema, mutation)
}

fn geo(geo: &mut Geo, mutation: &mut Mutation<'_>) -> Result<(), AuctionError> {
    let Geo {
        country,
        lat,
        lon,
        ext,
        other,
    } = geo;
    let schema = SchemaId::Geo;
    if clear_object(schema, mutation.disclosure) {
        if country.is_some() || lat.is_some() || lon.is_some() || ext.is_some() || !other.is_empty()
        {
            *country = None;
            *lat = None;
            *lon = None;
            *ext = None;
            other.clear();
            mutation.record(schema, ChangeKind::Coppa);
        }
        return Ok(());
    }
    option(
        country,
        schema,
        FieldId::GeoCountry,
        Shape::String,
        mutation,
    );
    coordinate(lat, FieldId::GeoLat, mutation);
    coordinate(lon, FieldId::GeoLon, mutation);
    option(ext, schema, FieldId::GeoExt, Shape::Object, mutation);
    value::map(other, schema, mutation)
}

fn user_agent(sua: &mut UserAgent, mutation: &mut Mutation<'_>) -> Result<(), AuctionError> {
    let UserAgent {
        browsers,
        platform,
        mobile,
        architecture,
        bitness,
        model,
        source,
        ext,
        other,
    } = sua;
    let schema = SchemaId::UserAgent;
    if let Action::Visit(_) = json(
        browsers,
        schema,
        FieldId::UserAgentBrowsers,
        Shape::ObjectArray,
        mutation,
    ) && let Some(browser_entries) = browsers.as_mut()
    {
        for browser in browser_entries {
            brand(browser, mutation)?;
        }
    }
    json_child(
        platform,
        schema,
        FieldId::UserAgentPlatform,
        brand,
        mutation,
    )?;
    json(
        mobile,
        schema,
        FieldId::UserAgentMobile,
        Shape::Integer,
        mutation,
    );
    json(
        architecture,
        schema,
        FieldId::UserAgentArchitecture,
        Shape::String,
        mutation,
    );
    json(
        bitness,
        schema,
        FieldId::UserAgentBitness,
        Shape::String,
        mutation,
    );
    json(
        model,
        schema,
        FieldId::UserAgentModel,
        Shape::String,
        mutation,
    );
    json(
        source,
        schema,
        FieldId::UserAgentSource,
        Shape::Integer,
        mutation,
    );
    json(ext, schema, FieldId::UserAgentExt, Shape::Object, mutation);
    value::map(other, schema, mutation)
}

fn brand(entry: &mut BrandVersion, mutation: &mut Mutation<'_>) -> Result<(), AuctionError> {
    // Do not materialize sparse backing for an admitted empty brand.
    let Some(BrandVersionFields {
        brand,
        version,
        ext,
        other,
    }) = entry.fields_mut()
    else {
        return Ok(());
    };
    let schema = SchemaId::BrandVersion;
    json(
        brand,
        schema,
        FieldId::BrandVersionBrand,
        Shape::String,
        mutation,
    );
    json(
        version,
        schema,
        FieldId::BrandVersionVersion,
        Shape::StringArray,
        mutation,
    );
    json(
        ext,
        schema,
        FieldId::BrandVersionExt,
        Shape::Object,
        mutation,
    );
    value::map(other, schema, mutation)
}

fn eid(eid: &mut Eid, mutation: &mut Mutation<'_>) -> Result<(), AuctionError> {
    let Eid {
        source,
        uids,
        other,
    } = eid;
    let schema = SchemaId::Eid;
    option(source, schema, FieldId::EidSource, Shape::String, mutation);
    if let Action::Visit(_) = option(uids, schema, FieldId::EidUids, Shape::ObjectArray, mutation)
        && let Some(uid_entries) = uids.as_mut()
    {
        for uid in uid_entries {
            self::uid(uid, mutation)?;
        }
    }
    value::map(other, schema, mutation)
}

fn uid(uid: &mut EidUid, mutation: &mut Mutation<'_>) -> Result<(), AuctionError> {
    #[expect(
        clippy::unneeded_field_pattern,
        reason = "explicit mandatory id preserves exhaustive model coverage; its owning collection gates denial"
    )]
    let EidUid {
        id: _,
        atype,
        ext,
        other,
    } = uid;
    let schema = SchemaId::Uid;
    // The enclosing collection owns ID denial. A mandatory model field cannot
    // be omitted if a future compiled rule changes that ownership contract.
    if let Action::Remove(_) = action(
        Some(FieldId::UidId.descriptor()),
        typed_input(FieldId::UidId, Shape::String),
        mutation.disclosure,
    ) {
        return Err(super::limits::failure());
    }
    option(atype, schema, FieldId::UidAtype, Shape::Integer, mutation);
    option(ext, schema, FieldId::UidExt, Shape::Object, mutation);
    value::map(other, schema, mutation)
}

fn tid_option(slot: &mut Option<String>, audit: &mut super::DisclosureAudit) {
    if slot.take().is_some() {
        audit.record(ChangeKind::Tid);
    }
}

fn tid_map(other: &mut Map<String, Value>, audit: &mut super::DisclosureAudit) {
    if other.remove("tid").is_some() {
        audit.record(ChangeKind::Tid);
    }
}

#[expect(
    clippy::unneeded_field_pattern,
    reason = "exhaustive Source, Imp and ImpExt patterns enforce exact TID ownership when models grow"
)]
pub(crate) fn tid(req: &mut BidRequest, audit: &mut super::DisclosureAudit) {
    if let Some(Source {
        fd: _,
        tid,
        pchain: _,
        ext: _,
        other,
    }) = req.source.as_mut()
    {
        tid_option(tid, audit);
        tid_map(other, audit);
    }
    for Imp {
        id: _,
        banner: _,
        video: _,
        native: _,
        audio: _,
        bidfloor: _,
        bidfloorcur: _,
        secure: _,
        ext,
        other,
    } in &mut req.imp
    {
        if let Some(ImpExt {
            tid,
            prebid: _,
            bidder: _,
            other: ext_other,
        }) = ext.as_mut()
        {
            tid_option(tid, audit);
            tid_map(ext_other, audit);
        }
        match other.get_mut("ext") {
            Some(Value::Object(shadow_ext)) => tid_map(shadow_ext, audit),
            Some(_) => {
                other.remove("ext");
                audit.record(ChangeKind::Malformed);
            }
            None => {}
        }
    }
}
