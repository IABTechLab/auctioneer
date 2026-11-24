//! Scope indexing for the selected serde emitter, not general Rust resolution.

use super::Refusal;
use super::model::{Binding, Origin, Ty};
use std::collections::BTreeMap;
use syn::visit_mut::{self, VisitMut as _};
use syn::{FnArg, GenericArgument, Item, Pat, PathArguments, ReturnType, Type, UseTree};

pub(super) struct SourceFunction<'a> {
    pub identity: String,
    pub module: String,
    pub receiver: Option<String>,
    pub signature: &'a syn::Signature,
    pub body: &'a syn::Block,
}

pub(super) struct Index<'a> {
    pub functions: Vec<SourceFunction<'a>>,
    pub roots: Vec<usize>,
    pub fields: BTreeMap<String, BTreeMap<String, Ty>>,
    pub statics: BTreeMap<String, Ty>,
    pub initialization: BTreeMap<String, Vec<String>>,
    imports: BTreeMap<String, String>,
    aliases: BTreeMap<String, &'a Type>,
}

pub(super) fn join(module: &str, name: &str) -> String {
    if module.is_empty() {
        name.to_owned()
    } else if name.is_empty() {
        module.to_owned()
    } else {
        format!("{module}::{name}")
    }
}

pub(super) fn path(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

impl<'a> Index<'a> {
    pub(super) fn collect(syntax: &'a syn::File) -> Result<Self, Refusal> {
        let mut inspected = syntax.clone();
        let mut attributes = Attributes { refused: false };
        attributes.visit_file_mut(&mut inspected);
        attributes.finish()?;
        let mut index = Self {
            functions: Vec::new(),
            roots: Vec::new(),
            fields: BTreeMap::new(),
            statics: BTreeMap::new(),
            initialization: BTreeMap::new(),
            imports: BTreeMap::new(),
            aliases: BTreeMap::new(),
        };
        index.declarations(&syntax.items, "")?;
        index.structures(&syntax.items, "")?;
        let markers: Vec<_> = index
            .functions
            .iter()
            .filter_map(|function| {
                (function.module.is_empty() && function.signature.ident == "is_valid")
                    .then_some(function.receiver.clone())
                    .flatten()
            })
            .collect();
        if markers.is_empty() {
            // The existing direct source fixtures exercise the same lowering,
            // without pretending to be a captured emitted declaration.
            for name in ["is_valid", "new_search_cache"] {
                if let Some(id) = index.function(name) {
                    index.roots.push(id);
                }
            }
        } else if let [marker] = markers.as_slice() {
            index.couple(marker)?;
        } else {
            return Err(Refusal::UnsupportedSource);
        }
        if index.roots.is_empty() {
            return Err(Refusal::UnclassifiedCall);
        }
        Ok(index)
    }

    fn declarations(&mut self, items: &'a [Item], module: &str) -> Result<(), Refusal> {
        for item in items {
            match item {
                Item::Use(item) => self.import(module, "", &item.tree)?,
                Item::Type(item) => {
                    if self
                        .aliases
                        .insert(join(module, &item.ident.to_string()), &item.ty)
                        .is_some()
                    {
                        return Err(Refusal::UnsupportedSource);
                    }
                }
                Item::Fn(function) => {
                    self.add_function(module, None, &function.sig, &function.block)?
                }
                Item::Impl(item) => {
                    if item.trait_.is_some() {
                        return Err(Refusal::UnsupportedSource);
                    }
                    let Type::Path(receiver) = item.self_ty.as_ref() else {
                        return Err(Refusal::ReceiverMismatch);
                    };
                    let receiver = join(module, &path(&receiver.path));
                    for member in &item.items {
                        if let syn::ImplItem::Fn(function) = member {
                            self.add_function(
                                module,
                                Some(receiver.clone()),
                                &function.sig,
                                &function.block,
                            )?;
                        } else {
                            return Err(Refusal::UnsupportedSource);
                        }
                    }
                }
                Item::Mod(item) => {
                    let (_, items) = item.content.as_ref().ok_or(Refusal::UnsupportedSource)?;
                    self.declarations(items, &join(module, &item.ident.to_string()))?;
                }
                Item::Struct(_) | Item::Static(_) | Item::Const(_) => {}
                _ => return Err(Refusal::UnsupportedSource),
            }
        }
        Ok(())
    }

    fn add_function(
        &mut self,
        module: &str,
        receiver: Option<String>,
        signature: &'a syn::Signature,
        body: &'a syn::Block,
    ) -> Result<(), Refusal> {
        let identity = join(
            receiver.as_deref().unwrap_or(module),
            &signature.ident.to_string(),
        );
        if self.function(&identity).is_some() {
            return Err(Refusal::UnsupportedSource);
        }
        self.functions.push(SourceFunction {
            identity,
            module: module.to_owned(),
            receiver,
            signature,
            body,
        });
        Ok(())
    }

    fn import(&mut self, module: &str, prefix: &str, tree: &UseTree) -> Result<(), Refusal> {
        let (name, target) = match tree {
            UseTree::Path(item) => {
                return self.import(module, &join(prefix, &item.ident.to_string()), &item.tree);
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    self.import(module, prefix, item)?;
                }
                return Ok(());
            }
            UseTree::Name(item) => (
                item.ident.to_string(),
                join(prefix, &item.ident.to_string()),
            ),
            UseTree::Rename(item) => (
                item.rename.to_string(),
                join(prefix, &item.ident.to_string()),
            ),
            UseTree::Glob(_) if prefix == "super" => return Ok(()),
            UseTree::Glob(_) => return Err(Refusal::UnsupportedSource),
        };
        if self.imports.insert(join(module, &name), target).is_some() {
            return Err(Refusal::UnsupportedSource);
        }
        Ok(())
    }

    fn structures(&mut self, items: &'a [Item], module: &str) -> Result<(), Refusal> {
        for item in items {
            match item {
                Item::Struct(item) => {
                    let mut fields = BTreeMap::new();
                    for field in &item.fields {
                        let name = field
                            .ident
                            .as_ref()
                            .ok_or(Refusal::UnsupportedSource)?
                            .to_string();
                        fields.insert(name, self.ty(&field.ty, module)?);
                    }
                    if self
                        .fields
                        .insert(join(module, &item.ident.to_string()), fields)
                        .is_some()
                    {
                        return Err(Refusal::UnsupportedSource);
                    }
                }
                Item::Static(item) => {
                    let name = join(module, &item.ident.to_string());
                    let ty = self.ty(&item.ty, module)?;
                    let Ty::LazyRegex(_) = ty else {
                        return Err(Refusal::UnsupportedNestedLiteralOwner);
                    };
                    if !matches!(item.mutability, syn::StaticMutability::None) {
                        return Err(Refusal::UnsupportedSource);
                    }
                    // The parent program owner checks the complete constructor.
                    // This recognizer never walks it as per-validation work.
                    let syn::Expr::Call(call) = item.expr.as_ref() else {
                        return Err(Refusal::UnsupportedSource);
                    };
                    let syn::Expr::Path(callee) = call.func.as_ref() else {
                        return Err(Refusal::UnsupportedSource);
                    };
                    if self.canonical(module, &path(&callee.path))? != "std::sync::LazyLock::new"
                        || call.args.len() != 1
                        || !matches!(call.args.first(), Some(syn::Expr::Closure(closure)) if closure.inputs.is_empty())
                    {
                        return Err(Refusal::UnsupportedSource);
                    }
                    if self
                        .statics
                        .insert(name.clone(), Ty::LazyRegex(name))
                        .is_some()
                    {
                        return Err(Refusal::UnsupportedSource);
                    }
                }
                Item::Mod(item) => {
                    let (_, items) = item.content.as_ref().ok_or(Refusal::UnsupportedSource)?;
                    self.structures(items, &join(module, &item.ident.to_string()))?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn couple(&mut self, marker: &str) -> Result<(), Refusal> {
        let module = format!("__{}_impl", marker.to_lowercase());
        let wrapper = format!("{marker}SearchCache");
        let inner = join(&module, "__SearchCache");
        let outer_fields = self.fields.get(&wrapper).ok_or(Refusal::ReceiverMismatch)?;
        if outer_fields.len() != 1 || outer_fields.get("__inner") != Some(&Ty::Cache(inner.clone()))
        {
            return Err(Refusal::ReceiverMismatch);
        }
        let inner_fields = self.fields.get(&inner).ok_or(Refusal::ReceiverMismatch)?;
        let programs: Vec<_> = self
            .statics
            .keys()
            .filter(|name| {
                name.rsplit_once("::")
                    .is_some_and(|(scope, _)| scope == module)
            })
            .cloned()
            .collect();
        // The selected schemas have no reference/URI mutable state. A new field
        // needs a supported reset/construction/drop lemma, not an empty reset.
        if inner_fields.len() != programs.len() {
            return Err(Refusal::UnsupportedSource);
        }
        for program in &programs {
            let symbol = program
                .rsplit("::")
                .next()
                .ok_or(Refusal::UnsupportedSource)?;
            let field = symbol
                .strip_suffix("_REGEX")
                .ok_or(Refusal::UnsupportedSource)?
                .to_lowercase()
                + "_cache";
            if inner_fields.get(&field)
                != Some(&Ty::Cache("regex_automata::meta::Cache".to_owned()))
            {
                return Err(Refusal::ReceiverMismatch);
            }
        }
        for (public, private) in [
            ("is_valid", "entry_is_valid"),
            ("new_search_cache", "new_search_cache"),
            ("initialize_regexes", "initialize_regexes"),
        ] {
            let id = self
                .function(&join(marker, public))
                .ok_or(Refusal::UnclassifiedCall)?;
            let function = self.functions.get(id).ok_or(Refusal::UnclassifiedCall)?;
            let target = join(&module, private);
            let mut found = false;
            // Only wrapper nesting (block/Option::map) is accepted here; the
            // ordinary lowerer checks all calls, arguments and closure bodies.
            wrapper_target(function.body, &target, &mut found);
            if !found || self.function(&target).is_none() {
                return Err(Refusal::UnclassifiedCall);
            }
            if public != "initialize_regexes" {
                self.roots.push(id);
            }
        }
        self.initialization
            .insert(join(&module, "initialize_regexes"), programs);
        Ok(())
    }

    pub(super) fn function(&self, name: &str) -> Option<usize> {
        self.functions
            .iter()
            .position(|function| function.identity == name)
    }

    pub(super) fn canonical(&self, module: &str, input: &str) -> Result<String, Refusal> {
        let mut current = input.to_owned();
        for _ in 0..=self.imports.len() {
            let (first, rest) = current.split_once("::").unwrap_or((&current, ""));
            let replacement = self
                .imports
                .get(&join(module, first))
                .or_else(|| self.imports.get(first));
            let Some(replacement) = replacement else {
                if let Some(rest) = current.strip_prefix("self::") {
                    return Ok(join(module, rest));
                }
                if let Some(rest) = current.strip_prefix("super::") {
                    let parent = module.rsplit_once("::").map_or("", |(parent, _)| parent);
                    return Ok(join(parent, rest));
                }
                return Ok(current);
            };
            let next = join(replacement, rest);
            if next == current {
                return Ok(current);
            }
            current = next;
        }
        Err(Refusal::UnsupportedSource)
    }

    pub(super) fn ty(&self, ty: &Type, module: &str) -> Result<Ty, Refusal> {
        self.ty_inner(ty, module, 0)
    }

    fn ty_inner(&self, ty: &Type, module: &str, depth: usize) -> Result<Ty, Refusal> {
        if depth > self.aliases.len() + 1 {
            return Err(Refusal::UnsupportedSource);
        }
        match ty {
            Type::Reference(ty) => self.ty_inner(&ty.elem, module, depth),
            Type::Slice(ty) if self.ty_inner(&ty.elem, module, depth)? == Ty::Value => Ok(Ty::Array),
            Type::Slice(_) => Ok(Ty::Named("slice".to_owned())),
            Type::Tuple(ty) => Ok(if ty.elems.is_empty() { Ty::Unit } else { Ty::Tuple(ty.elems.iter().map(|ty| self.ty_inner(ty, module, depth)).collect::<Result<_, _>>()?) }),
            Type::TraitObject(ty) if ty.bounds.iter().any(|bound| matches!(bound, syn::TypeParamBound::Trait(bound) if bound.path.segments.last().is_some_and(|segment| segment.ident == "FnMut"))) => Ok(Ty::Callback(String::new())),
            Type::Path(ty) if ty.qself.is_none() => {
                let raw = path(&ty.path);
                if let Some(alias) = self.aliases.get(&join(module, &raw)) {
                    return self.ty_inner(alias, module, depth + 1);
                }
                let name = self.canonical(module, &raw)?;
                let last = ty.path.segments.last().ok_or(Refusal::UnsupportedSource)?;
                let generic = |position: usize| -> Result<Ty, Refusal> {
                    let PathArguments::AngleBracketed(arguments) = &last.arguments else { return Err(Refusal::ReceiverMismatch); };
                    let Some(GenericArgument::Type(ty)) = arguments.args.iter().nth(position) else { return Err(Refusal::ReceiverMismatch); };
                    self.ty_inner(ty, module, depth)
                };
                Ok(match name.as_str() {
                    "serde_json::Value" | "jsonschema::__private::serde_json::Value" => Ty::Value,
                    "String" | "str" | "std::string::String" => Ty::String,
                    "serde_json::Number" | "jsonschema::__private::serde_json::Number" => Ty::Number,
                    "bool" => Ty::Bool,
                    "usize" | "u64" | "u8" | "i64" | "f64" => Ty::Scalar,
                    "serde_json::Map" | "jsonschema::__private::serde_json::Map" if generic(0)? == Ty::String && generic(1)? == Ty::Value => Ty::Map,
                    "Option" | "std::option::Option" => Ty::Option(Box::new(generic(0)?)),
                    "std::sync::LazyLock" if generic(0)? == Ty::Option(Box::new(Ty::Regex)) => Ty::LazyRegex(String::new()),
                    "regex_automata::meta::Regex" => Ty::Regex,
                    "regex_automata::meta::Cache" => Ty::Cache(name),
                    "regex_automata::Input" => Ty::Input,
                    _ if self.fields.contains_key(&join(module, &name)) || name.ends_with("SearchCache") => Ty::Cache(if name.contains("::") { name } else { join(module, &name) }),
                    _ => Ty::Named(name),
                })
            }
            _ => Err(Refusal::ReceiverMismatch),
        }
    }

    pub(super) fn parameters(&self, id: usize) -> Result<Vec<Binding>, Refusal> {
        let function = self.functions.get(id).ok_or(Refusal::UnclassifiedCall)?;
        function
            .signature
            .inputs
            .iter()
            .enumerate()
            .map(|(ordinal, argument)| match argument {
                FnArg::Typed(argument) => {
                    let Pat::Ident(name) = argument.pat.as_ref() else {
                        return Err(Refusal::ReceiverMismatch);
                    };
                    let mut ty = self.ty(&argument.ty, &function.module)?;
                    if let Ty::Callback(binding) = &mut ty {
                        *binding = name.ident.to_string();
                    }
                    Ok(Binding {
                        declaration: None,
                        ty,
                        place: name.ident.to_string(),
                        borrowed: matches!(argument.ty.as_ref(), Type::Reference(_)),
                        origin: Origin::Parameter {
                            function: id,
                            ordinal,
                        },
                    })
                }
                FnArg::Receiver(receiver) => Ok(Binding {
                    declaration: None,
                    ty: Ty::Cache(function.receiver.clone().ok_or(Refusal::ReceiverMismatch)?),
                    place: "self".to_owned(),
                    borrowed: receiver.reference.is_some(),
                    origin: Origin::Parameter {
                        function: id,
                        ordinal,
                    },
                }),
            })
            .collect()
    }

    pub(super) fn result(&self, id: usize) -> Result<Ty, Refusal> {
        let function = self.functions.get(id).ok_or(Refusal::UnclassifiedCall)?;
        match &function.signature.output {
            ReturnType::Default => Ok(Ty::Unit),
            ReturnType::Type(_, ty) => self.ty(ty, &function.module),
        }
    }
}

// This visits the complete parsed AST, including unreachable declarations,
// signatures, fields, locals, patterns and expressions. It never expands an
// attribute or mutates the pristine caller syntax. Only selected-emitter forms
// are benign; a known attribute name alone is not an exemption.
struct Attributes {
    refused: bool,
}

impl Attributes {
    fn finish(self) -> Result<(), Refusal> {
        if self.refused {
            Err(Refusal::UnsupportedSource)
        } else {
            Ok(())
        }
    }
}

impl visit_mut::VisitMut for Attributes {
    fn visit_attribute_mut(&mut self, _attribute: &mut syn::Attribute) {
        self.refused = true;
    }

    fn visit_item_mod_mut(&mut self, module: &mut syn::ItemMod) {
        self.refused |= !module.attrs.iter().all(module_attribute);
        module.attrs.clear();
        visit_mut::visit_item_mod_mut(self, module);
    }

    fn visit_item_fn_mut(&mut self, function: &mut syn::ItemFn) {
        self.refused |= !function.attrs.iter().all(|attribute| {
            matches!(attribute.style, syn::AttrStyle::Outer)
                && matches!(&attribute.meta, syn::Meta::Path(path) if path.is_ident("inline"))
        });
        function.attrs.clear();
        visit_mut::visit_item_fn_mut(self, function);
    }

    fn visit_impl_item_fn_mut(&mut self, method: &mut syn::ImplItemFn) {
        self.refused |= !method.attrs.is_empty() && !initializer_documentation(method);
        method.attrs.clear();
        visit_mut::visit_impl_item_fn_mut(self, method);
    }
}

// emit_root.rs's public initialization wrapper is the sole selected method
// carrying doc-name/value attributes. Preserve the four exact strings (including
// doc-comment leading spaces) and its exact non-generic, argument-free signature.
fn initializer_documentation(method: &syn::ImplItemFn) -> bool {
    const DOCS: [&str; 4] = [
        " Initializes every generated pattern regex, including after a",
        " failed constructor. Returns false if any cached construction",
        " failed. Repeated calls do not reconstruct or retry regexes.",
        " This does not initialize custom formats or bound search caches.",
    ];
    let signature = &method.sig;
    matches!(method.vis, syn::Visibility::Public(_))
        && method.defaultness.is_none()
        && signature.ident == "initialize_regexes"
        && signature.constness.is_none()
        && signature.asyncness.is_none()
        && signature.unsafety.is_none()
        && signature.abi.is_none()
        && signature.generics.params.is_empty()
        && signature.generics.where_clause.is_none()
        && signature.inputs.is_empty()
        && signature.variadic.is_none()
        && matches!(&signature.output, ReturnType::Type(_, ty)
            if matches!(ty.as_ref(), Type::Path(ty) if ty.qself.is_none() && ty.path.is_ident("bool")))
        && method.attrs.len() == DOCS.len()
        && method.attrs.iter().zip(DOCS).all(|(attribute, expected)| {
            matches!(attribute.style, syn::AttrStyle::Outer)
                && matches!(&attribute.meta, syn::Meta::NameValue(meta)
                    if meta.path.is_ident("doc")
                        && matches!(&meta.value, syn::Expr::Lit(value)
                            if value.attrs.is_empty()
                                && matches!(&value.lit, syn::Lit::Str(text) if text.value() == expected)))
        })
}

fn module_attribute(attribute: &syn::Attribute) -> bool {
    if !matches!(attribute.style, syn::AttrStyle::Outer) {
        return false;
    }
    let syn::Meta::List(list) = &attribute.meta else {
        return false;
    };
    let expected: &[&str] = if list.path.is_ident("doc") {
        &["hidden"]
    } else if list.path.is_ident("allow") {
        &[
            "non_snake_case",
            "dead_code",
            "unused_imports",
            "unused_variables",
            "unreachable_code",
            "clippy::all",
        ]
    } else {
        return false;
    };
    let Ok(arguments) = list.parse_args_with(
        syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
    ) else {
        return false;
    };
    arguments.len() == expected.len()
        && !arguments.trailing_punct()
        && arguments.iter().zip(expected).all(|(argument, expected)| {
            argument.leading_colon.is_none()
                && argument
                    .segments
                    .iter()
                    .all(|segment| matches!(segment.arguments, PathArguments::None))
                && path(argument) == *expected
        })
}

// `matches!` is the one supported executable macro with AST-bearing arguments.
// Its parsed arguments need the same boundary; opaque macro tokens are not an
// attribute interpreter or an alternative way to approve source semantics.
pub(super) fn check_match_attributes(
    expression: &mut syn::Expr,
    pattern: &mut syn::Pat,
) -> Result<(), Refusal> {
    let mut attributes = Attributes { refused: false };
    attributes.visit_expr_mut(expression);
    attributes.visit_pat_mut(pattern);
    attributes.finish()
}

fn wrapper_target(block: &syn::Block, target: &str, found: &mut bool) {
    fn expr(value: &syn::Expr, target: &str, found: &mut bool) {
        match value {
            syn::Expr::Block(block) => wrapper_target(&block.block, target, found),
            syn::Expr::Call(call) => {
                if matches!(call.func.as_ref(), syn::Expr::Path(callee) if path(&callee.path) == target)
                {
                    *found = true;
                }
            }
            syn::Expr::MethodCall(call) if call.method == "map" => {
                expr(&call.receiver, target, found)
            }
            _ => {}
        }
    }
    for statement in &block.stmts {
        if let syn::Stmt::Expr(value, _) = statement {
            expr(value, target, found);
        }
    }
}
