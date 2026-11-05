//! Static request-disclosure descriptors; no request-local registry.

pub use crate::disclosure_types::{Delegate, Domains, Rule, Shape, Transform};

/// One protected object and its sorted reviewed fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SchemaDesc {
    pub id: SchemaId,
    pub name: &'static str,
    pub fields: &'static [FieldRule],
    pub domains: Domains,
}

/// A shape-checked field disposition and its derived disclosure domains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldRule {
    pub id: FieldId,
    pub name: &'static str,
    pub shape: Shape,
    pub rule: Rule,
    pub transform: Option<Transform>,
    pub child: Option<SchemaId>,
    pub delegate: Option<Delegate>,
    pub domains: Domains,
}

include!(concat!(env!("OUT_DIR"), "/disclosure.rs"));

impl SchemaDesc {
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&'static FieldRule> {
        let index = self
            .fields
            .binary_search_by(|field| field.name.cmp(name))
            .ok()?;
        self.fields.get(index)
    }
}
