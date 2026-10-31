//! Borrowed structural admission before recursive JSON allocation.
//!
//! This serializer retains no output. It stops before descending beyond the
//! container ceiling, counts map keys as well as values, and treats the pinned
//! `serde_json` `RawValue` protocol as JSON rather than as a single string.

use serde::Serialize;
use serde::ser::{self, SerializeMap, SerializeSeq, SerializeStruct};
use std::error::Error;
use std::fmt;

/// Root containers are depth one.
pub const MAX_CONSTRUCTION_DEPTH: u32 = 128;
const RAW_VALUE: &str = "$serde_json::private::RawValue";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreflightError {
    Nodes,
    Depth,
    Representation,
}

impl fmt::Display for PreflightError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Nodes => "JSON construction node budget exceeded",
            Self::Depth => "JSON construction depth budget exceeded",
            Self::Representation => "unsupported JSON construction representation",
        })
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "remaining Error methods are deprecated or unstable"
)]
impl Error for PreflightError {}

impl ser::Error for PreflightError {
    fn custom<T: fmt::Display>(_message: T) -> Self {
        // Never turn a serializer's input-derived diagnostic into an audit/log.
        Self::Representation
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PreflightStats {
    pub nodes: u64,
    pub max_depth: u32,
}

struct Counter {
    stats: PreflightStats,
    depth: u32,
    node_limit: u64,
    depth_limit: u32,
    raw_string: bool,
}

impl Counter {
    fn node(&mut self) -> Result<(), PreflightError> {
        if self.raw_string {
            return Err(PreflightError::Representation);
        }
        self.stats.nodes = self
            .stats
            .nodes
            .checked_add(1)
            .ok_or(PreflightError::Nodes)?;
        if self.stats.nodes > self.node_limit {
            return Err(PreflightError::Nodes);
        }
        Ok(())
    }

    fn enter(&mut self) -> Result<(), PreflightError> {
        let next = self.depth.checked_add(1).ok_or(PreflightError::Depth)?;
        if next > self.depth_limit {
            return Err(PreflightError::Depth);
        }
        self.node()?;
        self.depth = next;
        self.stats.max_depth = self.stats.max_depth.max(next);
        Ok(())
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    fn raw(&mut self, json: &str) -> Result<(), PreflightError> {
        let mut depth = self.depth;
        let mut quoted = false;
        let mut escaped = false;
        for byte in json.bytes() {
            if quoted {
                if escaped {
                    escaped = false;
                } else {
                    match byte {
                        b'\\' => escaped = true,
                        b'"' => quoted = false,
                        _ => {}
                    }
                }
            } else {
                match byte {
                    b'"' => quoted = true,
                    b'{' | b'[' => {
                        depth = depth.checked_add(1).ok_or(PreflightError::Depth)?;
                        if depth > self.depth_limit {
                            return Err(PreflightError::Depth);
                        }
                        self.stats.max_depth = self.stats.max_depth.max(depth);
                    }
                    b'}' | b']' => {
                        depth = depth.checked_sub(1).ok_or(PreflightError::Representation)?;
                    }
                    _ => {}
                }
            }
        }
        let remaining = self
            .node_limit
            .checked_sub(self.stats.nodes)
            .ok_or(PreflightError::Nodes)?;
        let limit = u32::try_from(remaining).unwrap_or(u32::MAX);
        let nodes =
            crate::count_json_nodes(json.as_bytes(), limit).map_err(|error| match error {
                crate::BoundedJsonError::NodeBudget(_) => PreflightError::Nodes,
                crate::BoundedJsonError::Syntax(_) | crate::BoundedJsonError::RawValue(_) => {
                    PreflightError::Representation
                }
            })?;
        self.stats.nodes = self
            .stats
            .nodes
            .checked_add(u64::from(nodes))
            .ok_or(PreflightError::Nodes)?;
        Ok(())
    }
}

/// Inspect caller-owned input without a tree or serialization buffer.
///
/// # Errors
/// Refuses excessive nodes/depth or an unsupported serialization protocol.
/// The input remains borrowed: its owner remains responsible for cleanup.
pub fn preflight<T: Serialize + ?Sized>(
    value: &T,
    max_nodes: u64,
    max_depth: u32,
) -> Result<PreflightStats, PreflightError> {
    let mut counter = Counter {
        stats: PreflightStats::default(),
        depth: 0,
        node_limit: max_nodes,
        depth_limit: max_depth,
        raw_string: false,
    };
    value.serialize(&mut counter)?;
    Ok(counter.stats)
}

struct Compound<'a> {
    counter: &'a mut Counter,
    raw: bool,
    variant: bool,
}

impl Compound<'_> {
    fn field<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), PreflightError> {
        value.serialize(&mut *self.counter)
    }

    fn finish(self) {
        if !self.raw {
            self.counter.leave();
        }
        if self.variant {
            self.counter.leave();
        }
    }
}

macro_rules! scalar_methods {
    ($($method:ident($type:ty)),* $(,)?) => {$ (
        fn $method(self, _value: $type) -> Result<(), PreflightError> { self.node() }
    )*};
}

#[expect(
    clippy::missing_trait_methods,
    reason = "default collect_seq and collect_map delegate to the same bounded primitives without allocating"
)]
impl<'a> ser::Serializer for &'a mut Counter {
    type Ok = ();
    type Error = PreflightError;
    type SerializeSeq = Compound<'a>;
    type SerializeTuple = Compound<'a>;
    type SerializeTupleStruct = Compound<'a>;
    type SerializeTupleVariant = Compound<'a>;
    type SerializeMap = Compound<'a>;
    type SerializeStruct = Compound<'a>;
    type SerializeStructVariant = Compound<'a>;

    scalar_methods!(
        serialize_bool(bool),
        serialize_i8(i8),
        serialize_i16(i16),
        serialize_i32(i32),
        serialize_i64(i64),
        serialize_i128(i128),
        serialize_u8(u8),
        serialize_u16(u16),
        serialize_u32(u32),
        serialize_u64(u64),
        serialize_u128(u128),
        serialize_f32(f32),
        serialize_f64(f64),
        serialize_char(char)
    );

    fn serialize_str(self, v: &str) -> Result<(), PreflightError> {
        if self.raw_string {
            self.raw(v)
        } else {
            self.node()
        }
    }
    fn serialize_bytes(self, v: &[u8]) -> Result<(), PreflightError> {
        self.enter()?;
        for _ in v {
            self.node()?;
        }
        self.leave();
        Ok(())
    }
    fn serialize_none(self) -> Result<(), PreflightError> {
        self.node()
    }
    fn serialize_some<T: Serialize + ?Sized>(self, value: &T) -> Result<(), PreflightError> {
        value.serialize(self)
    }
    fn serialize_unit(self) -> Result<(), PreflightError> {
        self.node()
    }
    fn serialize_unit_struct(self, _name: &'static str) -> Result<(), PreflightError> {
        self.node()
    }
    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
    ) -> Result<(), PreflightError> {
        self.node()
    }
    fn serialize_newtype_struct<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<(), PreflightError> {
        value.serialize(self)
    }
    fn serialize_newtype_variant<T: Serialize + ?Sized>(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        value: &T,
    ) -> Result<(), PreflightError> {
        self.enter()?;
        self.node()?;
        value.serialize(&mut *self)?;
        self.leave();
        Ok(())
    }
    fn serialize_seq(self, _len: Option<usize>) -> Result<Compound<'a>, PreflightError> {
        self.enter()?;
        Ok(Compound {
            counter: self,
            raw: false,
            variant: false,
        })
    }
    fn serialize_tuple(self, len: usize) -> Result<Compound<'a>, PreflightError> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Compound<'a>, PreflightError> {
        self.serialize_seq(Some(len))
    }
    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Compound<'a>, PreflightError> {
        self.enter()?;
        self.node()?;
        let mut compound = self.serialize_seq(Some(len))?;
        compound.variant = true;
        Ok(compound)
    }
    fn serialize_map(self, len: Option<usize>) -> Result<Compound<'a>, PreflightError> {
        self.serialize_seq(len)
    }
    fn serialize_struct(
        self,
        name: &'static str,
        len: usize,
    ) -> Result<Compound<'a>, PreflightError> {
        if name == RAW_VALUE {
            if len != 1 {
                return Err(PreflightError::Representation);
            }
            return Ok(Compound {
                counter: self,
                raw: true,
                variant: false,
            });
        }
        self.serialize_map(Some(len))
    }
    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _index: u32,
        _variant: &'static str,
        len: usize,
    ) -> Result<Compound<'a>, PreflightError> {
        self.enter()?;
        self.node()?;
        let mut compound = self.serialize_map(Some(len))?;
        compound.variant = true;
        Ok(compound)
    }
    fn collect_str<T: fmt::Display + ?Sized>(self, _value: &T) -> Result<(), PreflightError> {
        Err(PreflightError::Representation)
    }
    fn is_human_readable(&self) -> bool {
        true
    }
}

impl SerializeSeq for Compound<'_> {
    type Ok = ();
    type Error = PreflightError;
    fn serialize_element<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), PreflightError> {
        self.field(value)
    }
    fn end(self) -> Result<(), PreflightError> {
        self.finish();
        Ok(())
    }
}

macro_rules! tuple_impl {
    ($trait:ident) => {
        impl ser::$trait for Compound<'_> {
            type Ok = ();
            type Error = PreflightError;
            fn serialize_field<T: Serialize + ?Sized>(
                &mut self,
                value: &T,
            ) -> Result<(), PreflightError> {
                self.field(value)
            }
            fn end(self) -> Result<(), PreflightError> {
                self.finish();
                Ok(())
            }
        }
    };
}
impl ser::SerializeTuple for Compound<'_> {
    type Ok = ();
    type Error = PreflightError;
    fn serialize_element<T: Serialize + ?Sized>(
        &mut self,
        value: &T,
    ) -> Result<(), PreflightError> {
        self.field(value)
    }
    fn end(self) -> Result<(), PreflightError> {
        self.finish();
        Ok(())
    }
}
tuple_impl!(SerializeTupleStruct);
tuple_impl!(SerializeTupleVariant);

#[expect(
    clippy::missing_trait_methods,
    reason = "default serialize_entry delegates to the same bounded key/value primitives without allocating"
)]
impl SerializeMap for Compound<'_> {
    type Ok = ();
    type Error = PreflightError;
    fn serialize_key<T: Serialize + ?Sized>(&mut self, key: &T) -> Result<(), PreflightError> {
        self.field(key)
    }
    fn serialize_value<T: Serialize + ?Sized>(&mut self, value: &T) -> Result<(), PreflightError> {
        self.field(value)
    }
    fn end(self) -> Result<(), PreflightError> {
        self.finish();
        Ok(())
    }
}
impl SerializeStruct for Compound<'_> {
    type Ok = ();
    type Error = PreflightError;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), PreflightError> {
        if self.raw {
            if key != RAW_VALUE {
                return Err(PreflightError::Representation);
            }
            self.counter.raw_string = true;
            let result = self.field(value);
            self.counter.raw_string = false;
            result
        } else {
            self.counter.node()?;
            self.field(value)
        }
    }
    fn end(self) -> Result<(), PreflightError> {
        self.finish();
        Ok(())
    }
    fn skip_field(&mut self, _key: &'static str) -> Result<(), PreflightError> {
        Ok(())
    }
}
impl ser::SerializeStructVariant for Compound<'_> {
    type Ok = ();
    type Error = PreflightError;
    fn serialize_field<T: Serialize + ?Sized>(
        &mut self,
        _key: &'static str,
        value: &T,
    ) -> Result<(), PreflightError> {
        self.counter.node()?;
        self.field(value)
    }
    fn end(self) -> Result<(), PreflightError> {
        self.finish();
        Ok(())
    }
    fn skip_field(&mut self, _key: &'static str) -> Result<(), PreflightError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_CONSTRUCTION_DEPTH, PreflightError, preflight};
    use crate::alloc_track::allocated_on_this_thread;
    use serde::Serialize;
    use serde_json::value::RawValue;
    use serde_json::{Value, json};

    #[test]
    fn borrowed_values_bound_depth_before_descent_and_leave_cleanup_to_caller() {
        for (depth, admitted) in [(127_u32, true), (128, true), (129, false), (4096, false)] {
            let mut value = Value::Null;
            for _ in 0..depth {
                value = Value::Array(vec![value]);
            }
            let result = preflight(&value, 8192, MAX_CONSTRUCTION_DEPTH);
            assert_eq!(result.is_ok(), admitted, "depth {depth}");
            if !admitted {
                assert_eq!(result, Err(PreflightError::Depth));
            }
            while let Value::Array(mut children) = value {
                value = children.pop().unwrap();
            }
        }
    }

    #[test]
    fn raw_extensions_are_counted_as_json_with_parent_depth_and_duplicate_keys() {
        #[derive(Serialize)]
        struct Request {
            ext: Box<RawValue>,
        }
        // A RawValue is already syntax-validated but still needs resource bounds.
        let raw = json!({"nested": [null, {"escaped": "[{\\\"}"}]}).to_string();
        let request = Request {
            ext: RawValue::from_string(raw).unwrap(),
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        let count = crate::count_json_nodes(&bytes, 100).unwrap();
        let stats = preflight(&request, u64::from(count), 128).unwrap();
        assert_eq!(stats.nodes, u64::from(count));
        assert_eq!(stats.max_depth, 4);
        assert_eq!(
            preflight(&request, u64::from(count - 1), 128),
            Err(PreflightError::Nodes)
        );
        assert_eq!(preflight(&request, 100, 3), Err(PreflightError::Depth));
        let dup_request = Request {
            ext: RawValue::from_string("{\"a\":0,\"a\":1}".into()).unwrap(),
        };
        assert_eq!(preflight(&dup_request, 7, 2).unwrap().nodes, 7);
        assert_eq!(preflight(&dup_request, 6, 2), Err(PreflightError::Nodes));
    }

    #[test]
    fn syntactic_node_counts_match_serialized_maps_flatten_and_enum_shapes() {
        #[derive(Serialize)]
        struct Flat {
            name: String,
            #[serde(flatten)]
            extra: serde_json::Map<String, Value>,
        }
        let flat = Flat {
            name: "context".into(),
            extra: serde_json::from_value(json!({"x": [null, true, {}]})).unwrap(),
        };
        let bytes = serde_json::to_vec(&flat).unwrap();
        let nodes = crate::count_json_nodes(&bytes, 100).unwrap();
        assert_eq!(preflight(&flat, 100, 128).unwrap().nodes, u64::from(nodes));
    }

    #[test]
    fn borrowed_structural_preflight_does_not_allocate_an_output_or_width_stack() {
        let value = json!({"items": vec![json!({"context": [null, true, 1_i32]}); 1024]});
        let before = allocated_on_this_thread();
        let stats = preflight(&value, 10000, 128).unwrap();
        let after = allocated_on_this_thread();
        assert_eq!(before, after);
        assert_eq!(stats.nodes, 6147);
    }
}
