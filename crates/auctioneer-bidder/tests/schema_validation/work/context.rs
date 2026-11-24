//! Caller-schedule correspondence only, not a source-completion capability.
//!
//! Fixed AST recipes justify the parent's invocation/input-walk events. Frozen
//! bytes bind the remaining opaque context identities, not their runtime costs.
//! Callee/iterator/serializer/allocation/drop work remains unproved here. The
//! parent's allocation-success premise excludes fatal OOM diagnostics only;
//! ordinary returned errors, partial construction and unwind are not excluded.

use super::{ContextBinding, ContextSources, Refusal, SourceRole};
use auctioneer_bidder::schema::SchemaInputLimits;
use quote::ToTokens;
use serde_json::Value;
use syn::visit_mut::{self, VisitMut};

pub(super) fn check(
    binding: &ContextBinding<'_>,
    sources: &ContextSources<'_>,
) -> Result<SchemaInputLimits, Refusal> {
    if binding.descriptor_binding != binding.context_binding {
        return Err(Refusal::BindingMismatch);
    }
    if !nonempty_collection(binding.context_roots) || !nonempty_collection(binding.compiled_sources)
    {
        return Err(Refusal::UnsupportedContext);
    }

    // Do not derive supported recipes from the same include_bytes! identities.
    // In particular, updating source and its capture cannot bless a new schedule.
    let mut native = parse(sources.native, SourceRole::Native)?;
    require_recipe(&native, native_recipe(), SourceRole::Native)?;
    require_recipe(
        &parse(sources.budget, SourceRole::Budget)?,
        budget_recipe(),
        SourceRole::Budget,
    )?;
    require_recipe(
        &parse(sources.input, SourceRole::Input)?,
        input_recipe(),
        SourceRole::Input,
    )?;
    require_recipe(
        &parse(sources.json_size, SourceRole::JsonSize)?,
        json_size_recipe(),
        SourceRole::JsonSize,
    )?;
    require_recipe(
        &parse(sources.exchange_schema, SourceRole::ExchangeSchema)?,
        exchange_recipe(),
        SourceRole::ExchangeSchema,
    )?;

    // Identity-only binding. This does not prove the full run/split/generator/
    // format closure, caller caps or any common runtime primitive cost.
    for (role, actual, frozen) in [
        (SourceRole::Native, sources.native, super::NATIVE),
        (SourceRole::Budget, sources.budget, super::BUDGET),
        (SourceRole::Input, sources.input, super::INPUT),
        (SourceRole::JsonSize, sources.json_size, super::JSON_SIZE),
        (
            SourceRole::ExchangeSchema,
            sources.exchange_schema,
            super::EXCHANGE_SCHEMA,
        ),
        (
            SourceRole::ExchangeRun,
            sources.exchange_run,
            super::EXCHANGE_RUN,
        ),
        (
            SourceRole::ExchangeSplit,
            sources.exchange_split,
            super::EXCHANGE_SPLIT,
        ),
        (
            SourceRole::ExchangeLimits,
            sources.exchange_limits,
            super::EXCHANGE_LIMITS,
        ),
        (
            SourceRole::DispatchGenerator,
            sources.dispatch_generator,
            super::DISPATCH_GENERATOR,
        ),
        (SourceRole::Formats, sources.formats, super::FORMATS),
    ] {
        if actual != frozen {
            return Err(Refusal::SourceShape(role));
        }
    }

    let mut extraction = NativeLimits::default();
    for item in &mut native.items {
        if let syn::Item::Fn(function) = item
            && function.sig.ident == "validate_native_parameters"
        {
            extraction.visit_block_mut(&mut function.block);
        }
    }
    if extraction.hits != 1 {
        return Err(Refusal::SourceShape(SourceRole::Native));
    }
    extraction
        .limits
        .ok_or(Refusal::SourceShape(SourceRole::Native))
}

fn nonempty_collection(value: &Value) -> bool {
    match value {
        Value::Array(items) => !items.is_empty(),
        Value::Object(items) => !items.is_empty(),
        _ => false,
    }
}

fn parse(source: &[u8], role: SourceRole) -> Result<syn::File, Refusal> {
    let text = std::str::from_utf8(source).map_err(|_error| Refusal::SourceShape(role))?;
    syn::parse_file(text).map_err(|_error| Refusal::SourceShape(role))
}

fn tokens(value: &impl ToTokens) -> String {
    value.to_token_stream().to_string()
}

fn literal_doc(attribute: &syn::Attribute) -> bool {
    if !matches!(&attribute.style, syn::AttrStyle::Outer) || !attribute.path().is_ident("doc") {
        return false;
    }
    let syn::Meta::NameValue(meta) = &attribute.meta else {
        return false;
    };
    matches!(&meta.value, syn::Expr::Lit(value) if matches!(&value.lit, syn::Lit::Str(_)))
}

fn without_docs(mut item: syn::Item) -> syn::Item {
    match &mut item {
        syn::Item::Fn(function) => function.attrs.retain(|attribute| !literal_doc(attribute)),
        syn::Item::Const(value) => value.attrs.retain(|attribute| !literal_doc(attribute)),
        syn::Item::Impl(implementation) => {
            implementation
                .attrs
                .retain(|attribute| !literal_doc(attribute));
            for member in &mut implementation.items {
                if let syn::ImplItem::Fn(function) = member {
                    function.attrs.retain(|attribute| !literal_doc(attribute));
                }
            }
        }
        _ => {}
    }
    item
}

// Only selected top-level definitions count. A test or unused nested helper
// cannot supply a recipe. Duplicate definitions/impls refuse, as do unexpected
// non-doc attributes, changed signatures, callee arguments or nested control flow.
fn require_recipe(
    actual: &syn::File,
    supported: syn::File,
    role: SourceRole,
) -> Result<(), Refusal> {
    for expected in supported.items {
        let mut candidates = actual.items.iter().filter(|item| match (*item, &expected) {
            (syn::Item::Fn(left), syn::Item::Fn(right)) => left.sig.ident == right.sig.ident,
            (syn::Item::Const(left), syn::Item::Const(right)) => left.ident == right.ident,
            (syn::Item::Struct(left), syn::Item::Struct(right)) => left.ident == right.ident,
            (syn::Item::Impl(left), syn::Item::Impl(right)) => {
                tokens(&left.self_ty) == tokens(&right.self_ty)
            }
            (syn::Item::Use(_), syn::Item::Use(_)) => tokens(*item) == tokens(&expected),
            _ => false,
        });
        let Some(candidate) = candidates.next() else {
            return Err(Refusal::SourceShape(role));
        };
        if candidates.next().is_some()
            || tokens(&without_docs(candidate.clone())) != tokens(&without_docs(expected))
        {
            return Err(Refusal::SourceShape(role));
        }
    }
    Ok(())
}

#[derive(Default)]
struct NativeLimits {
    hits: usize,
    limits: Option<SchemaInputLimits>,
}

impl VisitMut for NativeLimits {
    fn visit_expr_struct_mut(&mut self, value: &mut syn::ExprStruct) {
        if value.qself.is_none() && value.path.is_ident("SchemaInputLimits") {
            self.hits += 1;
            let field = |name: &str| -> Option<u64> {
                let value = value.fields.iter().find(
                    |field| matches!(&field.member, syn::Member::Named(ident) if ident == name),
                )?;
                let syn::Expr::Lit(value) = &value.expr else {
                    return None;
                };
                let syn::Lit::Int(value) = &value.lit else {
                    return None;
                };
                value.base10_parse().ok()
            };
            self.limits = (|| {
                Some(SchemaInputLimits {
                    serialized_bytes: field("serialized_bytes")?,
                    nodes: field("nodes")?,
                    depth: u32::try_from(field("depth")?).ok()?,
                })
            })();
        }
        visit_mut::visit_expr_struct_mut(self, value);
    }
}

// These are parsed data, never compiled copies of handwritten implementation.
// The entire cfg-gated native body is pinned, including the non-wasm schedule.
fn native_recipe() -> syn::File {
    syn::parse_quote! {
        #[cfg(not(target_arch = "wasm32"))]
        use super::{
            SchemaCostError, SchemaEvaluationCost, SchemaEvaluationError, SchemaInputLimits,
            measure_schema_input,
        };
        use super::{SchemaWorkBudget, SchemaWorkLimits};
        use serde_json::Value;
        pub fn validate_native_parameters<F>(
            name: &str,
            value: &Value,
            budget: &mut SchemaWorkBudget,
            mut expired: F,
        ) -> Result<(), NativeSchemaError>
        where F: FnMut() -> bool,
        {
            if expired() { return Err(NativeSchemaError::Deadline); }
            #[cfg(not(target_arch = "wasm32"))]
            {
                if INITIALIZED.get() != Some(&true) || cfg!(feature = "schema-artifact-staging") {
                    return Err(NativeSchemaError::Unavailable);
                }
                let owner = super::staging::native_owner(name).ok_or(NativeSchemaError::Unavailable)?;
                let stats = measure_schema_input(
                    value,
                    SchemaInputLimits { serialized_bytes: 0x0001_0000, nodes: 4_096, depth: 32, },
                ).map_err(|_error| NativeSchemaError::InvalidParameters)?;
                let work = stats.serialized_bytes.checked_mul(16)
                    .and_then(|bytes| {
                        let nodes = stats.nodes.checked_mul(128)?;
                        bytes.checked_add(nodes)
                    })
                    .and_then(|units| units.checked_add(1_024));
                let cost = work.map(|work_units| SchemaEvaluationCost {
                    work_units,
                    scratch_bytes: NATIVE_SCHEMA_SCRATCH_BYTES,
                }).ok_or(SchemaCostError::Overflow);
                let mut available = true;
                let valid = budget.evaluate(cost, &mut expired, || {
                    super::staging::validate_native(owner, value).unwrap_or_else(|| {
                        available = false;
                        false
                    })
                }).map_err(|error| match error {
                    SchemaEvaluationError::Deadline => NativeSchemaError::Deadline,
                    SchemaEvaluationError::CostOverflow
                    | SchemaEvaluationError::ZeroWork
                    | SchemaEvaluationError::SingleWork
                    | SchemaEvaluationError::Scratch
                    | SchemaEvaluationError::AggregateWork => NativeSchemaError::Resource,
                })?;
                if expired() { return Err(NativeSchemaError::Deadline); }
                if !available { return Err(NativeSchemaError::Unavailable); }
                if valid { Ok(()) } else { Err(NativeSchemaError::InvalidParameters) }
            }
            #[cfg(target_arch = "wasm32")]
            {
                let _: (&str, &Value, &mut SchemaWorkBudget) = (name, value, budget);
                Err(NativeSchemaError::Unavailable)
            }
        }
    }
}

fn budget_recipe() -> syn::File {
    syn::parse_quote! {
        impl SchemaWorkBudget {
            #[must_use]
            pub const fn new(limits: SchemaWorkLimits) -> Self {
                Self { limits, remaining: limits.auction_work_units, }
            }
            #[must_use]
            pub const fn remaining(&self) -> u64 { self.remaining }
            pub fn evaluate<C: FnOnce() -> bool, V: FnOnce() -> bool>(
                &mut self,
                projected_cost: Result<SchemaEvaluationCost, SchemaCostError>,
                is_expired: C,
                validate: V,
            ) -> Result<bool, SchemaEvaluationError> {
                if is_expired() { return Err(SchemaEvaluationError::Deadline); }
                let cost = projected_cost
                    .map_err(|SchemaCostError::Overflow| SchemaEvaluationError::CostOverflow)?;
                if cost.work_units == 0 { return Err(SchemaEvaluationError::ZeroWork); }
                if cost.work_units > self.limits.single_evaluation_work_units {
                    return Err(SchemaEvaluationError::SingleWork);
                }
                if cost.scratch_bytes > self.limits.evaluation_scratch_bytes {
                    return Err(SchemaEvaluationError::Scratch);
                }
                let remaining = self.remaining.checked_sub(cost.work_units)
                    .ok_or(SchemaEvaluationError::AggregateWork)?;
                self.remaining = remaining;
                Ok(validate())
            }
        }
    }
}

fn input_recipe() -> syn::File {
    syn::parse_quote! {
        use auctioneer_runtime::serialized_json_bytes;
        use serde_json::Value;
        pub const MAX_SCHEMA_INPUT_DEPTH: u32 = 32;
        pub fn measure_schema_input(
            value: &Value,
            limits: SchemaInputLimits,
        ) -> Result<SchemaInputStats, SchemaInputError> {
            if limits.depth > MAX_SCHEMA_INPUT_DEPTH {
                return Err(SchemaInputError::InvalidDepthLimit);
            }
            let mut measurement = InputMeasurement {
                limits,
                stats: SchemaInputStats { serialized_bytes: 0, nodes: 0, max_depth: 0, },
            };
            measurement.visit(value, 0)?;
            Ok(measurement.stats)
        }
        struct InputMeasurement { limits: SchemaInputLimits, stats: SchemaInputStats, }
        impl InputMeasurement {
            fn charge_bytes(&mut self, bytes: u64) -> Result<(), SchemaInputError> {
                let next = self.stats.serialized_bytes.checked_add(bytes).ok_or(SchemaInputError::Bytes)?;
                if next > self.limits.serialized_bytes { return Err(SchemaInputError::Bytes); }
                self.stats.serialized_bytes = next;
                Ok(())
            }
            fn charge_node(&mut self) -> Result<(), SchemaInputError> {
                let next = self.stats.nodes.checked_add(1).ok_or(SchemaInputError::Nodes)?;
                if next > self.limits.nodes { return Err(SchemaInputError::Nodes); }
                self.stats.nodes = next;
                Ok(())
            }
            fn check_string_len(&self, raw_len: usize) -> Result<(), SchemaInputError> {
                let minimum = u64::try_from(raw_len).ok()
                    .and_then(|length| length.checked_add(2)).ok_or(SchemaInputError::Bytes)?;
                let remaining = self.limits.serialized_bytes.checked_sub(self.stats.serialized_bytes)
                    .ok_or(SchemaInputError::Bytes)?;
                if minimum > remaining { return Err(SchemaInputError::Bytes); }
                Ok(())
            }
            fn charge_string(&mut self, text: &str) -> Result<(), SchemaInputError> {
                self.check_string_len(text.len())?;
                let bytes = serialized_json_bytes(text)
                    .map_err(|_serialization| SchemaInputError::Representation)?;
                self.charge_bytes(bytes)
            }
            fn enter_container(&mut self, parent_depth: u32) -> Result<u32, SchemaInputError> {
                let depth = parent_depth.checked_add(1).ok_or(SchemaInputError::Depth)?;
                if depth > self.limits.depth { return Err(SchemaInputError::Depth); }
                self.stats.max_depth = self.stats.max_depth.max(depth);
                self.charge_bytes(2)?;
                Ok(depth)
            }
            fn visit(&mut self, value: &Value, parent_depth: u32) -> Result<(), SchemaInputError> {
                self.charge_node()?;
                match value {
                    Value::Array(items) => {
                        let depth = self.enter_container(parent_depth)?;
                        let mut first = true;
                        for item in items {
                            if !first { self.charge_bytes(1)?; }
                            first = false;
                            self.visit(item, depth)?;
                        }
                        Ok(())
                    }
                    Value::Object(properties) => {
                        let depth = self.enter_container(parent_depth)?;
                        let mut first = true;
                        for (key, item) in properties {
                            if !first { self.charge_bytes(1)?; }
                            first = false;
                            self.charge_node()?;
                            self.charge_string(key)?;
                            self.charge_bytes(1)?;
                            self.visit(item, depth)?;
                        }
                        Ok(())
                    }
                    Value::String(text) => self.charge_string(text),
                    Value::Null | Value::Bool(_) | Value::Number(_) => {
                        let bytes = serialized_json_bytes(value)
                            .map_err(|_serialization| SchemaInputError::Representation)?;
                        self.charge_bytes(bytes)
                    }
                }
            }
        }
    }
}

fn json_size_recipe() -> syn::File {
    syn::parse_quote! {
        use serde::Serialize;
        use std::io::{self, Write};
        #[derive(Default)]
        struct Counter(u64);
        #[expect(
            clippy::missing_trait_methods,
            reason = "provided Write methods delegate to write; vectored capability methods are unstable"
        )]
        impl Write for Counter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0 = u64::try_from(buf.len()).ok()
                    .and_then(|len| self.0.checked_add(len))
                    .ok_or_else(|| io::Error::other("JSON size overflow"))?;
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        pub fn serialized_json_bytes<T: Serialize + ?Sized>(value: &T) -> Result<u64, serde_json::Error> {
            let mut counter = Counter::default();
            serde_json::to_writer(&mut counter, value)?;
            Ok(counter.0)
        }
    }
}

fn exchange_recipe() -> syn::File {
    syn::parse_quote! {
        use super::{AuctionContext, ErrorEntry, MakeRequestsOutput, PerBidderRequest, TmaxResolution};
        use auctioneer_bidder::schema::{
            NATIVE_SCHEMA_SCRATCH_BYTES, NativeSchemaError, native_schema_available,
            native_schema_required, validate_native_parameters,
        };
        use auctioneer_openrtb::types::bid_request::BidRequest;
        use serde_json::Value;
        pub(super) fn admit(
            ctx: &AuctionContext<'_>,
            pbr: &mut PerBidderRequest,
            tmax: &TmaxResolution,
            out: &mut MakeRequestsOutput,
        ) -> bool {
            let mandatory = native_schema_required(pbr.base.as_str());
            let funded_legacy = ctx.schema_scratch_bytes >= NATIVE_SCHEMA_SCRATCH_BYTES
                && native_schema_available(pbr.base.as_str());
            if !mandatory && !funded_legacy { return true; }
            let errors = out.bidder_pre_errors.entry(pbr.bidder.clone()).or_default();
            if ctx.schema_scratch_bytes < NATIVE_SCHEMA_SCRATCH_BYTES
                || ctx.projection_reservation.is_none()
            {
                errors.push(error(NativeSchemaError::Unavailable));
                return false;
            }
            retain_valid(&mut pbr.request, errors, |value| {
                validate_native_parameters(pbr.base.as_str(), value, &mut out.schema_work, || {
                    ctx.clock.now_monotonic() >= tmax.deadline.deadline_at
                })
            });
            !pbr.request.imp.is_empty()
        }
        fn retain_valid<F>(request: &mut BidRequest, errors: &mut Vec<ErrorEntry>, mut validate: F)
        where F: FnMut(&Value) -> Result<(), NativeSchemaError>,
        {
            request.imp.retain(|imp| {
                let result = imp.ext.as_ref()
                    .and_then(|ext| ext.bidder.as_ref())
                    .ok_or(NativeSchemaError::InvalidParameters)
                    .and_then(&mut validate);
                match result {
                    Ok(()) => true,
                    Err(reason) => {
                        errors.push(error(reason));
                        false
                    }
                }
            });
        }
    }
}
