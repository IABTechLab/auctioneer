//! Requested-allocation census of the actual emitted metadata constructor.
//! Unsupported Rust expression shapes refuse rather than receive a zero cost.

use super::types::MetadataHeapShape;
use sha2::{Digest as _, Sha256};
use std::fmt::{self, Display, Formatter};
use syn::parse::Parser as _;
use syn::punctuated::Punctuated;
use syn::{Expr, Lit, MacroDelimiter, Path, PathArguments, Token};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageError {
    Syntax,
    UnsupportedExpression,
    UnsupportedVectorElement,
    MixedVectorElements,
    CountOverflow,
}

impl Display for StorageError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Syntax => "metadata storage: invalid constructor expression",
            Self::UnsupportedExpression => "metadata storage: unsupported constructor expression",
            Self::UnsupportedVectorElement => "metadata storage: unsupported vector element",
            Self::MixedVectorElements => "metadata storage: inconsistent vector element types",
            Self::CountOverflow => "metadata storage: count overflow",
        })
    }
}

/// Count requested heap storage in an emitted metadata constructor.
///
/// # Errors
/// Rejects malformed or unsupported expressions and overflowing counts.
pub fn constructor_heap(source: &str) -> Result<MetadataHeapShape, StorageError> {
    let expression = syn::parse_str::<Expr>(source).map_err(|_syntax| StorageError::Syntax)?;
    let mut heap = MetadataHeapShape::default();
    walk(&expression, &mut heap)?;
    Ok(heap)
}

/// Render a storage descriptor bound to the constructors and declaration inputs.
///
/// # Errors
/// Rejects unsupported constructors and any overflowing storage or input count.
pub fn render_metadata_storage(
    constructors: &[String],
    bindings: &str,
    declarations: usize,
) -> Result<String, StorageError> {
    let mut heap = MetadataHeapShape::default();
    let mut digest = Sha256::new();
    digest.update(b"auctioneer-metadata-storage-v1\0");
    bind(&mut digest, bindings.as_bytes())?;
    bind_count(&mut digest, declarations)?;
    for constructor in constructors {
        add(&mut heap, constructor_heap(constructor)?)?;
        bind(&mut digest, constructor.as_bytes())?;
    }
    let hash: Vec<_> = digest
        .finalize()
        .iter()
        .map(|byte| proc_macro2::Literal::u8_unsuffixed(*byte))
        .collect();
    let active = super::typed_integer(&super::u64_digits(count(constructors.len())?), "u64");
    let declaration_literal = super::typed_integer(&super::u64_digits(count(declarations)?), "u64");
    let fields = [
        (quote::quote!(string_bytes), heap.string_bytes),
        (quote::quote!(nonempty_strings), heap.nonempty_strings),
        (quote::quote!(string_elements), heap.string_elements),
        (quote::quote!(media_type_elements), heap.media_type_elements),
        (quote::quote!(sync_type_elements), heap.sync_type_elements),
        (quote::quote!(nonempty_vectors), heap.nonempty_vectors),
    ]
    .map(|(name, value)| {
        let literal = super::typed_integer(&super::u64_digits(value), "u64");
        quote::quote!(#name: #literal,)
    });
    Ok(quote::quote! {
        use crate::types::{MetadataHeapShape, MetadataStorageDescriptor};
        #[must_use]
        pub const fn bidder_metadata_storage() -> MetadataStorageDescriptor {
            MetadataStorageDescriptor {
                active_bidders: #active, active_bindings: #active,
                declarations: #declaration_literal,
                heap: MetadataHeapShape { #(#fields)* }, source_sha256: [#(#hash),*]
            }
        }
    }
    .to_string())
}

/// Bind separate native and guest censuses to the same complete publication source.
///
/// # Errors
/// Rejects unsupported constructors, syntax, or overflowing counts.
pub fn render_target_metadata_storage(
    constructors: &[(bool, String)],
    bindings: &str,
    declarations: usize,
) -> Result<String, StorageError> {
    let mut output = proc_macro2::TokenStream::new();
    for native in [true, false] {
        let selected: Vec<_> = constructors
            .iter()
            .filter(|(native_only, _)| native || !native_only)
            .map(|(_, source)| source.clone())
            .collect();
        let source = render_metadata_storage(&selected, bindings, declarations)?;
        let syntax = syn::parse_file(&source).map_err(|_error| StorageError::Syntax)?;
        let gate = if native {
            quote::quote!(#[cfg(not(target_arch = "wasm32"))])
        } else {
            quote::quote!(#[cfg(target_arch = "wasm32")])
        };
        for item in syntax.items {
            output.extend(quote::quote!(#gate #item));
        }
    }
    Ok(output.to_string())
}

fn count(value: usize) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_population| StorageError::CountOverflow)
}

fn bind(digest: &mut Sha256, bytes: &[u8]) -> Result<(), StorageError> {
    bind_count(digest, bytes.len())?;
    digest.update(bytes);
    Ok(())
}

fn bind_count(digest: &mut Sha256, population: usize) -> Result<(), StorageError> {
    digest.update(count(population)?.to_string().as_bytes());
    digest.update(b":");
    Ok(())
}

fn add(left: &mut MetadataHeapShape, right: MetadataHeapShape) -> Result<(), StorageError> {
    let sum = |a: u64, b: u64| a.checked_add(b).ok_or(StorageError::CountOverflow);
    *left = MetadataHeapShape {
        string_bytes: sum(left.string_bytes, right.string_bytes)?,
        nonempty_strings: sum(left.nonempty_strings, right.nonempty_strings)?,
        string_elements: sum(left.string_elements, right.string_elements)?,
        media_type_elements: sum(left.media_type_elements, right.media_type_elements)?,
        sync_type_elements: sum(left.sync_type_elements, right.sync_type_elements)?,
        nonempty_vectors: sum(left.nonempty_vectors, right.nonempty_vectors)?,
    };
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Element {
    Scalar,
    String,
    MediaType,
    SyncType,
}

fn path_is(path: &Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path.segments.iter().zip(expected).all(|(segment, name)| {
            segment.ident == *name && matches!(segment.arguments, PathArguments::None)
        })
}

fn walk(expression: &Expr, heap: &mut MetadataHeapShape) -> Result<Element, StorageError> {
    if let Expr::Struct(node) = expression
        && node.attrs.is_empty()
        && node.rest.is_none()
    {
        for field in &node.fields {
            if !field.attrs.is_empty() {
                return Err(StorageError::UnsupportedExpression);
            }
            walk(&field.expr, heap)?;
        }
        return Ok(Element::Scalar);
    }
    if let Expr::Lit(node) = expression
        && node.attrs.is_empty()
        && matches!(node.lit, Lit::Bool(_) | Lit::Int(_) | Lit::Float(_))
    {
        return Ok(Element::Scalar);
    }
    if let Expr::Path(node) = expression
        && node.attrs.is_empty()
        && node.qself.is_none()
    {
        return enum_value(&node.path);
    }
    if let Expr::Call(node) = expression
        && node.attrs.is_empty()
    {
        return constructor_call(node, heap);
    }
    if let Expr::MethodCall(node) = expression
        && node.attrs.is_empty()
        && node.method == "to_owned"
        && node.turbofish.is_none()
        && node.args.is_empty()
    {
        return owned_string(&node.receiver, heap);
    }
    if let Expr::Macro(node) = expression
        && node.attrs.is_empty()
        && path_is(&node.mac.path, &["vec"])
        && matches!(node.mac.delimiter, MacroDelimiter::Bracket(_))
    {
        let values = Punctuated::<Expr, Token![,]>::parse_terminated
            .parse2(node.mac.tokens.clone())
            .map_err(|_syntax| StorageError::Syntax)?;
        return vector(&values, heap);
    }
    Err(StorageError::UnsupportedExpression)
}

fn constructor_call(
    node: &syn::ExprCall,
    heap: &mut MetadataHeapShape,
) -> Result<Element, StorageError> {
    let Expr::Path(function) = node.func.as_ref() else {
        return Err(StorageError::UnsupportedExpression);
    };
    if function.attrs.is_empty() && function.qself.is_none() {
        if path_is(&function.path, &["Some"]) && node.args.len() == 1 {
            for argument in &node.args {
                walk(argument, heap)?;
            }
            return Ok(Element::Scalar);
        }
        if path_is(&function.path, &["Vec", "new"]) && node.args.is_empty() {
            return Ok(Element::Scalar);
        }
    }
    Err(StorageError::UnsupportedExpression)
}

fn owned_string(receiver: &Expr, heap: &mut MetadataHeapShape) -> Result<Element, StorageError> {
    let Expr::Lit(literal) = receiver else {
        return Err(StorageError::UnsupportedExpression);
    };
    let Lit::Str(value) = &literal.lit else {
        return Err(StorageError::UnsupportedExpression);
    };
    if !literal.attrs.is_empty() {
        return Err(StorageError::UnsupportedExpression);
    }
    let bytes = count(value.value().len())?;
    add(
        heap,
        MetadataHeapShape {
            string_bytes: bytes,
            nonempty_strings: u64::from(bytes != 0),
            ..MetadataHeapShape::default()
        },
    )?;
    Ok(Element::String)
}

fn enum_value(path: &Path) -> Result<Element, StorageError> {
    if path_is(path, &["None"])
        || path_is(path, &["Compression", "Gzip"])
        || ["Image", "Blank"]
            .iter()
            .any(|variant| path_is(path, &["SyncResponseFormat", variant]))
    {
        return Ok(Element::Scalar);
    }
    if ["Banner", "Video", "Native", "Audio"]
        .iter()
        .any(|variant| path_is(path, &["MediaType", variant]))
    {
        return Ok(Element::MediaType);
    }
    if ["Iframe", "Redirect"]
        .iter()
        .any(|variant| path_is(path, &["SyncType", variant]))
    {
        return Ok(Element::SyncType);
    }
    Err(StorageError::UnsupportedExpression)
}

fn vector(
    values: &Punctuated<Expr, Token![,]>,
    heap: &mut MetadataHeapShape,
) -> Result<Element, StorageError> {
    let mut element = None;
    for value in values {
        let kind = walk(value, heap)?;
        if kind == Element::Scalar {
            return Err(StorageError::UnsupportedVectorElement);
        }
        if element.is_some_and(|previous| previous != kind) {
            return Err(StorageError::MixedVectorElements);
        }
        element = Some(kind);
    }
    let elements = count(values.len())?;
    let mut extra = MetadataHeapShape {
        nonempty_vectors: u64::from(elements != 0),
        ..MetadataHeapShape::default()
    };
    match element {
        Some(Element::String) => extra.string_elements = elements,
        Some(Element::MediaType) => extra.media_type_elements = elements,
        Some(Element::SyncType) => extra.sync_type_elements = elements,
        Some(Element::Scalar) => return Err(StorageError::UnsupportedVectorElement),
        None => {}
    }
    add(heap, extra)?;
    Ok(Element::Scalar)
}

#[cfg(test)]
mod tests {
    use super::{StorageError, add, constructor_heap, render_metadata_storage};
    use crate::codegen::{BidderInfoRaw, render_bidder_info};
    use crate::types::MetadataHeapShape;
    use quote::ToTokens as _;
    use std::slice::from_ref;

    fn checksum(source: &str) -> [u8; 32] {
        let file = syn::parse_file(source).unwrap();
        let function = file
            .items
            .into_iter()
            .find_map(|item| {
                if let syn::Item::Fn(function) = item
                    && function.sig.ident == "bidder_metadata_storage"
                {
                    Some(function)
                } else {
                    None
                }
            })
            .unwrap();
        let syn::Stmt::Expr(syn::Expr::Struct(body), None) = function.block.stmts.last().unwrap()
        else {
            panic!("descriptor must be a literal, not runtime work");
        };
        let field = body.fields.iter().find(|field| matches!(&field.member, syn::Member::Named(name) if name == "source_sha256")).unwrap();
        let syn::Expr::Array(bytes) = &field.expr else {
            panic!("literal digest array")
        };
        bytes
            .elems
            .iter()
            .map(|byte| {
                let syn::Expr::Lit(literal) = byte else {
                    panic!("literal digest byte")
                };
                let syn::Lit::Int(value) = &literal.lit else {
                    panic!("integer digest byte")
                };
                value.base10_parse::<u8>().unwrap()
            })
            .collect::<Vec<_>>()
            .try_into()
            .unwrap()
    }

    #[test]
    fn actual_constructor_census_counts_unequal_nested_owned_populations() {
        let source = r#"BidderInfo {
            name: "\u{e9}".to_owned(), endpoint: "https://example.test".to_owned(),
            host_allowlist: Some(vec!["a".to_owned(), "bbb".to_owned()]),
            endpoint_macros: vec!["AccountID".to_owned()],
            capabilities: Capabilities {
                site: vec![MediaType::Banner, MediaType::Video],
                app: vec![MediaType::Native], dooh: Vec::new()
            },
            user_sync: SyncerConfig {
                key: Some("family".to_owned()),
                iframe: Some(SyncerEndpoint { url: "iframe".to_owned(), user_macro: Some("".to_owned()) }),
                redirect: None,
                supports: vec![SyncType::Iframe, SyncType::Redirect],
                enabled: false,
                skip_when: SkipWhen { gdpr: true, gpp_sid: vec!["2".to_owned(), "6".to_owned(), "7".to_owned()] },
                default_response: Some(SyncResponseFormat::Image)
            },
            maintainer_email: Some("maintainer".to_owned()),
            openrtb: OpenRtbCapabilities { version: Some("2.6".to_owned()), gpp_supported: true },
            gvl_vendor_id: Some(7), endpoint_compression: Some(Compression::Gzip)
        }"#;
        assert_eq!(
            constructor_heap(source).unwrap(),
            MetadataHeapShape {
                string_bytes: 63,
                nonempty_strings: 12,
                string_elements: 6,
                media_type_elements: 3,
                sync_type_elements: 2,
                nonempty_vectors: 6,
            }
        );
    }

    #[test]
    fn empty_vectors_and_empty_strings_do_not_create_requested_allocations() {
        assert_eq!(
            constructor_heap("Row { a: Vec::new(), b: vec![], c: None, d: \"\".to_owned() }")
                .unwrap(),
            MetadataHeapShape::default()
        );
        let shape = constructor_heap("Row { a: vec![\"\".to_owned(), \"\".to_owned()] }").unwrap();
        assert_eq!(
            shape,
            MetadataHeapShape {
                string_elements: 2,
                nonempty_vectors: 1,
                ..MetadataHeapShape::default()
            }
        );
    }

    #[test]
    fn escaped_literals_are_counted_as_decoded_utf8_not_rust_source_bytes() {
        let shape = constructor_heap(r#"Row { a: "\u{1f600}\n\"\\".to_owned() }"#).unwrap();
        assert_eq!(shape.string_bytes, 7);
        assert_eq!(shape.nonempty_strings, 1);
    }

    #[test]
    fn unsupported_constructor_work_never_gets_an_empty_successful_census() {
        for (source, expected) in [
            ("Row { a: factory() }", StorageError::UnsupportedExpression),
            (
                "Row { a: value.clone() }",
                StorageError::UnsupportedExpression,
            ),
            (
                "Row { a: format!(\"{}\", 1) }",
                StorageError::UnsupportedExpression,
            ),
            (
                "Row { a: String::from(\"value\") }",
                StorageError::UnsupportedExpression,
            ),
            (
                "Row { a: { Vec::new() } }",
                StorageError::UnsupportedExpression,
            ),
            ("Row { a: vec![0; 4] }", StorageError::Syntax),
            ("Row { a: value }", StorageError::UnsupportedExpression),
            (
                "Row { ..Default::default() }",
                StorageError::UnsupportedExpression,
            ),
            (
                "Row { #[cfg(feature = \"optional\")] a: \"x\".to_owned() }",
                StorageError::UnsupportedExpression,
            ),
        ] {
            assert_eq!(
                constructor_heap(source).unwrap_err(),
                expected,
                "accepted {source}"
            );
        }
        assert_eq!(
            constructor_heap("not valid rust").unwrap_err(),
            StorageError::Syntax
        );
    }

    #[test]
    fn vector_element_layouts_cannot_be_unknown_or_mixed() {
        for (source, expected) in [
            (
                "Row { a: vec![Some(1)] }",
                StorageError::UnsupportedVectorElement,
            ),
            (
                "Row { a: vec![Unknown::Variant] }",
                StorageError::UnsupportedExpression,
            ),
            ("Row { a: vec![1] }", StorageError::UnsupportedVectorElement),
        ] {
            assert_eq!(
                constructor_heap(source).unwrap_err(),
                expected,
                "accepted {source}"
            );
        }
        assert_eq!(
            constructor_heap("Row { a: vec![MediaType::Banner, SyncType::Iframe] }").unwrap_err(),
            StorageError::MixedVectorElements
        );
    }

    #[test]
    fn descriptor_generation_binds_actual_constructor_bindings_and_inactive_declarations() {
        let raw: BidderInfoRaw =
            toml::from_str("implementation = 'fake_wire'\nendpoint = 'https://example.test'\n")
                .unwrap();
        let row = render_bidder_info("fake_demo", &raw).unwrap();
        let original = render_metadata_storage(from_ref(&row), "bindings-a", 3).unwrap();
        let parsed = syn::parse_file(&original).unwrap();
        let normalized = parsed.to_token_stream().to_string();
        for expected in [
            quote::quote!(pub const fn bidder_metadata_storage()),
            quote::quote!(active_bidders: 1_u64),
            quote::quote!(active_bindings: 1_u64),
            quote::quote!(declarations: 3_u64),
            quote::quote!(string_bytes: 29_u64),
        ] {
            assert!(normalized.contains(&expected.to_string()));
        }
        let changed_row = row.replace("fake_demo", "fake_faux");
        assert_eq!(
            constructor_heap(&row).unwrap(),
            constructor_heap(&changed_row).unwrap()
        );
        let changed = render_metadata_storage(&[changed_row], "bindings-a", 3).unwrap();
        assert_ne!(checksum(&original), checksum(&changed));
        assert_ne!(
            checksum(&original),
            checksum(&render_metadata_storage(from_ref(&row), "bindings-b", 3).unwrap())
        );
        assert_ne!(
            checksum(&original),
            checksum(&render_metadata_storage(&[row], "bindings-a", 4).unwrap())
        );
        assert_eq!(
            render_metadata_storage(&["factory()".into()], "bindings-a", 3).unwrap_err(),
            StorageError::UnsupportedExpression
        );
    }

    #[test]
    fn each_census_population_refuses_overflow_without_partial_publication() {
        let baseline = MetadataHeapShape {
            string_bytes: 1,
            nonempty_strings: 1,
            string_elements: 1,
            media_type_elements: 1,
            sync_type_elements: 1,
            nonempty_vectors: 1,
        };
        for extra in [
            MetadataHeapShape {
                string_bytes: u64::MAX,
                ..MetadataHeapShape::default()
            },
            MetadataHeapShape {
                nonempty_strings: u64::MAX,
                ..MetadataHeapShape::default()
            },
            MetadataHeapShape {
                string_elements: u64::MAX,
                ..MetadataHeapShape::default()
            },
            MetadataHeapShape {
                media_type_elements: u64::MAX,
                ..MetadataHeapShape::default()
            },
            MetadataHeapShape {
                sync_type_elements: u64::MAX,
                ..MetadataHeapShape::default()
            },
            MetadataHeapShape {
                nonempty_vectors: u64::MAX,
                ..MetadataHeapShape::default()
            },
        ] {
            let mut current = baseline;
            assert_eq!(
                add(&mut current, extra).unwrap_err(),
                StorageError::CountOverflow
            );
            assert_eq!(current, baseline);
        }
    }

    #[test]
    fn target_census_excludes_native_constructors_but_preserves_declarations() {
        let source = super::render_target_metadata_storage(
            &[
                (false, "Row { name: \"portable\".to_owned() }".into()),
                (true, "Row { name: \"native\".to_owned() }".into()),
            ],
            "complete-public-bindings",
            3,
        )
        .unwrap();
        let syntax = syn::parse_file(&source).unwrap();
        let normalized = quote::quote!(#syntax).to_string();
        assert_eq!(
            normalized
                .matches(&quote::quote!(declarations: 3_u64).to_string())
                .count(),
            2
        );
        assert!(normalized.contains(&quote::quote!(active_bidders: 2_u64).to_string()));
        assert!(normalized.contains(&quote::quote!(active_bidders: 1_u64).to_string()));
        assert!(normalized.contains(&quote::quote!(string_bytes: 14_u64).to_string()));
        assert!(normalized.contains(&quote::quote!(string_bytes: 8_u64).to_string()));
        assert_eq!(normalized.matches("target_arch").count(), 4);
    }
}
