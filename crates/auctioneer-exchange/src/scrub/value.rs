//! Shape-checked per-schema maps, never an arbitrary request-Value walker.

use super::decision::{Action, Disclosure, Input, action, clear_object};
use super::{ChangeKind, DisclosureAudit};
use crate::AuctionError;
use auctioneer_config::disclosure::{FieldRule, SCHEMAS, SchemaId, Shape, Transform};
use serde_json::map::IterMut as ObjectIterMut;
use serde_json::{Map, Number, Value};
use std::array::from_fn;
use std::fmt::{self, Write as _};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::slice::IterMut as ArrayIterMut;
use std::str::from_utf8;

pub(crate) struct Mutation<'a> {
    pub disclosure: Disclosure,
    pub transforms: TransformScratch,
    pub audit: &'a mut DisclosureAudit,
}

impl Mutation<'_> {
    pub(crate) fn record(&mut self, schema: SchemaId, kind: ChangeKind) {
        self.audit.record_schema(schema, kind);
    }
}

/// A canonical IPv6 address needs at most 39 ASCII bytes.
pub(crate) type AddressBuffer = [u8; 39];

pub(crate) struct TransformScratch {
    bytes: AddressBuffer,
    used: usize,
    address: Option<IpAddr>,
}

impl Default for TransformScratch {
    fn default() -> Self {
        Self {
            bytes: [0; 39],
            used: 0,
            address: None,
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "fmt::Write defaults use this bounded write_str"
)]
impl fmt::Write for TransformScratch {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.used.checked_add(s.len()).ok_or(fmt::Error)?;
        let destination = self.bytes.get_mut(self.used..end).ok_or(fmt::Error)?;
        destination.copy_from_slice(s.as_bytes());
        self.used = end;
        Ok(())
    }
}

impl TransformScratch {
    fn mask(&mut self, raw: &str, transform: Transform, disclosure: Disclosure) -> Option<&str> {
        self.address = match transform {
            Transform::Ipv4 => {
                let address: Ipv4Addr = raw.parse().ok()?;
                let shift = u32::from(32_u8.saturating_sub(disclosure.ip_keep_bits.v4.min(32)));
                let mask = u32::MAX.checked_shl(shift).unwrap_or(0);
                Some(IpAddr::V4(Ipv4Addr::from(u32::from(address) & mask)))
            }
            Transform::Ipv6 => {
                let address: Ipv6Addr = raw.parse().ok()?;
                let shift = u32::from(128_u8.saturating_sub(disclosure.ip_keep_bits.v6.min(128)));
                let mask = u128::MAX.checked_shl(shift).unwrap_or(0);
                Some(IpAddr::V6(Ipv6Addr::from(u128::from(address) & mask)))
            }
            Transform::Coordinate => None,
        };
        let address = self.address?;
        self.used = 0;
        write!(self, "{address}").ok()?;
        from_utf8(self.bytes.get(..self.used)?).ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Transformed {
    Unchanged,
    Changed,
    Invalid,
}

pub(crate) fn mask_string(
    value: &mut String,
    transform: Transform,
    mutation: &mut Mutation<'_>,
) -> Result<Transformed, AuctionError> {
    let Some(masked) = mutation
        .transforms
        .mask(value, transform, mutation.disclosure)
    else {
        return Ok(Transformed::Invalid);
    };
    if value.as_str() == masked {
        return Ok(Transformed::Unchanged);
    }
    // Canonical output can grow; scoped funding covers the exact destination.
    if value.capacity() < masked.len() {
        let additional = masked
            .len()
            .checked_sub(value.len())
            .ok_or_else(super::limits::failure)?;
        value
            .try_reserve_exact(additional)
            .map_err(|_error| super::limits::failure())?;
    }
    value.clear();
    value.push_str(masked);
    Ok(Transformed::Changed)
}

#[expect(
    clippy::float_arithmetic,
    reason = "reviewed finite half-up coordinate arithmetic"
)]
pub(crate) fn coordinate(value: f64) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }
    let scaled = value * 100.0_f64;
    if !scaled.is_finite() {
        return None;
    }
    let shifted = scaled + 0.5_f64;
    if !shifted.is_finite() {
        return None;
    }
    let result = shifted.floor() / 100.0_f64;
    result.is_finite().then_some(result)
}

pub(crate) fn input(descriptor: Option<&FieldRule>, value: &Value) -> Input {
    if value.is_null() {
        return Input::Null;
    }
    let Some(field) = descriptor else {
        return Input::Valid;
    };
    let valid = match field.shape {
        Shape::String => value.is_string(),
        Shape::Integer => value.is_i64() || value.is_u64(),
        Shape::Number => value.is_number(),
        Shape::StringArray => value
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_string)),
        Shape::Object => value.is_object(),
        Shape::ObjectArray => value
            .as_array()
            .is_some_and(|items| items.iter().all(Value::is_object)),
        Shape::Json => true,
    };
    if valid { Input::Valid } else { Input::Invalid }
}

/// Apply one field's shared action; child traversal is handled by bounded frames.
pub(crate) fn field(
    schema: SchemaId,
    descriptor: Option<&FieldRule>,
    value: &mut Value,
    mutation: &mut Mutation<'_>,
) -> Result<bool, AuctionError> {
    let retained = match action(descriptor, input(descriptor, value), mutation.disclosure) {
        Action::Keep | Action::Visit(_) => true,
        Action::Remove(kind) => {
            mutation.record(schema, kind);
            false
        }
        Action::Transform(Transform::Coordinate) => {
            let Some(number) = value
                .as_f64()
                .and_then(coordinate)
                .and_then(Number::from_f64)
            else {
                mutation.record(schema, ChangeKind::Malformed);
                return Ok(false);
            };
            if value.as_f64() != number.as_f64() {
                *value = Value::Number(number);
                mutation.record(schema, ChangeKind::Coarsen);
            }
            true
        }
        Action::Transform(transform @ (Transform::Ipv4 | Transform::Ipv6)) => {
            let transformed = if let Value::String(contents) = value {
                mask_string(contents, transform, mutation)?
            } else {
                Transformed::Invalid
            };
            match transformed {
                Transformed::Unchanged => true,
                Transformed::Changed => {
                    mutation.record(schema, ChangeKind::Coarsen);
                    true
                }
                Transformed::Invalid => {
                    mutation.record(schema, ChangeKind::Malformed);
                    false
                }
            }
        }
    };
    Ok(retained)
}

fn retain(
    map: &mut Map<String, Value>,
    schema: SchemaId,
    mutation: &mut Mutation<'_>,
    retention_failure: &mut Option<AuctionError>,
) -> Result<bool, AuctionError> {
    if clear_object(schema, mutation.disclosure) {
        if !map.is_empty() {
            map.clear();
            mutation.record(schema, ChangeKind::Coppa);
        }
        return Ok(false);
    }
    map.retain(|name, value| {
        if retention_failure.is_some() {
            return true;
        }
        match field(schema, schema.descriptor().field(name), value, mutation) {
            Ok(keep) => keep,
            Err(error) => {
                *retention_failure = Some(error);
                true
            }
        }
    });
    retention_failure.take().map_or(Ok(true), Err)
}

enum Frame<'a> {
    Object {
        schema: SchemaId,
        entries: ObjectIterMut<'a>,
    },
    Array {
        schema: SchemaId,
        items: ArrayIterMut<'a, Value>,
    },
}

impl<'a> Frame<'a> {
    fn next(&mut self, disclosure: Disclosure) -> Option<(SchemaId, &'a mut Value)> {
        match self {
            Self::Object { schema, entries } => {
                for (name, value) in entries.by_ref() {
                    let descriptor = schema.descriptor().field(name);
                    let input = if value.is_null() {
                        Input::Null
                    } else {
                        Input::Valid
                    };
                    if let Action::Visit(child) = action(descriptor, input, disclosure) {
                        return Some((child, value));
                    }
                }
                None
            }
            Self::Array { schema, items } => items.next().map(|value| (*schema, value)),
        }
    }
}

// A compiled acyclic schema edge can add one object and one array frame.
const FRAME_CAPACITY: usize = SCHEMAS.len() * 2;

pub(crate) struct Frames<'a> {
    storage: [Option<Frame<'a>>; FRAME_CAPACITY],
    used: usize,
    retention_failure: Option<AuctionError>,
}

impl<'a> Frames<'a> {
    fn new() -> Self {
        Self {
            storage: from_fn(|_| None),
            used: 0,
            retention_failure: None,
        }
    }

    fn push(&mut self, frame: Frame<'a>) -> Result<(), AuctionError> {
        let slot = self
            .storage
            .get_mut(self.used)
            .ok_or_else(super::limits::failure)?;
        *slot = Some(frame);
        self.used = self
            .used
            .checked_add(1)
            .ok_or_else(super::limits::failure)?;
        Ok(())
    }

    fn top(&mut self) -> Option<&mut Frame<'a>> {
        self.storage
            .get_mut(self.used.checked_sub(1)?)
            .and_then(Option::as_mut)
    }

    fn pop(&mut self) {
        if let Some(index) = self.used.checked_sub(1) {
            if let Some(slot) = self.storage.get_mut(index) {
                *slot = None;
            }
            self.used = index;
        }
    }
}

/// Walk only declared child schemas; opaque and unknown subtrees are never scanned for privacy.
pub(crate) fn map(
    map: &mut Map<String, Value>,
    schema: SchemaId,
    mutation: &mut Mutation<'_>,
) -> Result<(), AuctionError> {
    let mut frames = Frames::new();
    if !retain(map, schema, mutation, &mut frames.retention_failure)? {
        return Ok(());
    }
    frames.push(Frame::Object {
        schema,
        entries: map.iter_mut(),
    })?;
    while let Some(frame) = frames.top() {
        let Some((child_schema, value)) = frame.next(mutation.disclosure) else {
            frames.pop();
            continue;
        };
        let child_frame = match value {
            Value::Object(entries) => retain(
                entries,
                child_schema,
                mutation,
                &mut frames.retention_failure,
            )?
            .then(|| Frame::Object {
                schema: child_schema,
                entries: entries.iter_mut(),
            }),
            Value::Array(items) => Some(Frame::Array {
                schema: child_schema,
                items: items.iter_mut(),
            }),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
        };
        if let Some(child) = child_frame {
            frames.push(child)?;
        }
    }
    Ok(())
}

/// A Value-valued typed slot uses exactly the map field action and child engine.
pub(crate) fn slot(
    slot: &mut Option<Value>,
    schema: SchemaId,
    descriptor: &FieldRule,
    mutation: &mut Mutation<'_>,
) -> Result<(), AuctionError> {
    let Some(value) = slot.as_mut() else {
        return Ok(());
    };
    if !field(schema, Some(descriptor), value, mutation)? {
        *slot = None;
        return Ok(());
    }
    if let Action::Visit(child) = action(
        Some(descriptor),
        input(Some(descriptor), value),
        mutation.disclosure,
    ) {
        match value {
            Value::Object(entries) => map(entries, child, mutation)?,
            Value::Array(items) => {
                for item in items {
                    if let Value::Object(entries) = item {
                        map(entries, child, mutation)?;
                    }
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    Ok(())
}
