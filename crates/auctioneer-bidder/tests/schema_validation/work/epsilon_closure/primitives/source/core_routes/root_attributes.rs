//! Supported Rust 1.98.1 root-attribute recipe data, not compiled-source proof.

pub(super) fn core_lib() -> Vec<syn::Attribute> {
    let mut attributes = core_metadata_and_library_features();
    attributes.extend(core_language_features());
    attributes.extend(core_target_features());
    attributes
}

fn core_metadata_and_library_features() -> Vec<syn::Attribute> {
    let file: syn::File = syn::parse_quote! {
        #![stable(feature = "core", since = "1.6.0")]
        #![doc(
            html_playground_url = "https://play.rust-lang.org/",
            issue_tracker_base_url = "https://github.com/rust-lang/rust/issues/",
            test(no_crate_inject, attr(deny(warnings))),
            test(attr(allow(dead_code, deprecated, unused_variables, unused_mut, duplicate_features)))
        )]
        #![doc(rust_logo)]
        #![doc(auto_cfg(
            hide(no_fp_fmt_parse),
            hide(target_pointer_width, values("16", "32", "64")),
            hide(
                target_has_atomic,
                target_has_atomic_primitive_alignment,
                target_has_atomic_load_store,
                values("8", "16", "32", "64", "ptr"),
            ),
        ))]
        #![no_core]
        #![rustc_coherence_is_core]
        #![rustc_preserve_ub_checks]
        #![deny(rust_2021_incompatible_or_patterns)]
        #![deny(unsafe_op_in_unsafe_fn)]
        #![deny(implicit_provenance_casts)]
        #![warn(deprecated_in_future)]
        #![warn(missing_debug_implementations)]
        #![warn(missing_docs)]
        #![allow(explicit_outlives_requirements)]
        #![allow(incomplete_features)]
        #![warn(multiple_supertrait_upcastable)]
        #![allow(internal_features)]
        #![allow(unused_features)]
        #![deny(ffi_unwind_calls)]
        #![warn(unreachable_pub)]
        #![allow(rustdoc::redundant_explicit_links)]
        #![warn(rustdoc::unescaped_backticks)]
        #![feature(asm_experimental_arch)]
        #![feature(bstr_internals)]
        #![feature(cfg_target_has_reliable_f16_f128)]
        #![feature(const_carrying_mul_add)]
        #![feature(const_cmp)]
        #![feature(const_destruct)]
        #![feature(const_eval_select)]
        #![feature(const_select_unpredictable)]
        #![feature(core_intrinsics)]
        #![feature(coverage_attribute)]
        #![feature(disjoint_bitor)]
        #![feature(io_const_error)]
        #![feature(offset_of_enum)]
        #![feature(panic_internals)]
        #![feature(pattern_type_macro)]
        #![feature(ub_checks)]
    };
    file.attrs
}

fn core_language_features() -> Vec<syn::Attribute> {
    let file: syn::File = syn::parse_quote! {
        #![feature(abi_unadjusted)]
        #![feature(adt_const_params)]
        #![feature(allow_internal_unsafe)]
        #![feature(allow_internal_unstable)]
        #![feature(auto_traits)]
        #![feature(cfg_sanitize)]
        #![feature(cfg_target_has_atomic)]
        #![feature(cfg_ub_checks)]
        #![feature(const_closures)]
        #![feature(const_precise_live_drops)]
        #![feature(const_trait_impl)]
        #![feature(decl_macro)]
        #![feature(deprecated_suggestion)]
        #![feature(derive_const)]
        #![feature(diagnostic_on_const)]
        #![feature(diagnostic_on_unmatched_args)]
        #![feature(doc_cfg)]
        #![feature(doc_notable_trait)]
        #![feature(extern_types)]
        #![feature(f16)]
        #![feature(f128)]
        #![feature(field_projections)]
        #![feature(final_associated_functions)]
        #![feature(freeze_impls)]
        #![feature(fundamental)]
        #![feature(funnel_shifts)]
        #![feature(impl_restriction)]
        #![feature(intra_doc_pointers)]
        #![feature(intrinsics)]
        #![feature(lang_items)]
        #![feature(link_cfg)]
        #![feature(link_llvm_intrinsics)]
        #![feature(macro_metavar_expr)]
        #![feature(macro_metavar_expr_concat)]
        #![feature(marker_trait_attr)]
        #![feature(min_specialization)]
        #![feature(multiple_supertrait_upcastable)]
        #![feature(must_not_suspend)]
        #![feature(negative_impls)]
        #![feature(never_type)]
        #![feature(no_core)]
        #![feature(optimize_attribute)]
        #![feature(pattern_types)]
        #![feature(pin_macro_internals)]
        #![feature(prelude_import)]
        #![feature(repr_simd)]
        #![feature(rustc_attrs)]
        #![feature(rustdoc_internals)]
        #![feature(simd_ffi)]
        #![feature(staged_api)]
        #![feature(stmt_expr_attributes)]
        #![feature(strict_provenance_lints)]
        #![feature(trait_alias)]
        #![feature(transparent_unions)]
        #![feature(try_blocks)]
        #![feature(uint_carryless_mul)]
        #![feature(unboxed_closures)]
        #![feature(unsized_fn_params)]
        #![feature(with_negative_coherence)]
    };
    file.attrs
}

fn core_target_features() -> Vec<syn::Attribute> {
    let file: syn::File = syn::parse_quote! {
        #![feature(aarch64_unstable_target_feature)]
        #![feature(arm_target_feature)]
        #![feature(avx10_target_feature)]
        #![feature(clflushopt_target_feature)]
        #![feature(hexagon_target_feature)]
        #![feature(loongarch_target_feature)]
        #![feature(mips_target_feature)]
        #![feature(movrs_target_feature)]
        #![feature(nvptx_target_feature)]
        #![feature(powerpc_target_feature)]
        #![feature(riscv_target_feature)]
        #![feature(rtm_target_feature)]
        #![feature(s390x_target_feature)]
        #![feature(wasm_target_feature)]
        #![feature(x86_amx_intrinsics)]
    };
    file.attrs
}

pub(super) fn prelude() -> Vec<syn::Attribute> {
    let file: syn::File = syn::parse_quote! {
        #![cfg_attr(rustfmt, rustfmt::skip)]
        #![stable(feature = "core_prelude", since = "1.4.0")]
    };
    file.attrs
}

pub(super) fn v1() -> Vec<syn::Attribute> {
    let file: syn::File = syn::parse_quote! {
        #![stable(feature = "core_prelude", since = "1.4.0")]
        #![cfg_attr(rustfmt, rustfmt::skip)]
    };
    file.attrs
}
