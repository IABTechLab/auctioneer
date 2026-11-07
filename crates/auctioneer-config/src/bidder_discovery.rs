//! Build-only discovery at a crate's fixed `src/bidders` root.

use crate::{
    bidder_reconcile, build_catalog, codegen::publication_gate, types::BidderMetadataDeclaration,
};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

#[derive(Debug)]
struct Declaration {
    key: String,
    required_feature: Option<String>,
    native_only: bool,
    has_settings: bool,
    allocation_conformance: Option<PathBuf>,
    response_conformance: Option<PathBuf>,
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
}

#[derive(Debug)]
pub struct Discovery {
    manifests: BTreeMap<String, build_catalog::Manifest>,
    declarations: Vec<(PathBuf, Declaration)>,
    uid_policies: Vec<(String, PathBuf)>,
    features: BTreeMap<String, Vec<String>>,
    rerun_paths: BTreeSet<PathBuf>,
}

/// Discover bidder-owned factory and conformance declarations in the fixed source root.
///
/// # Errors
/// Rejects invalid paths, unreadable inputs, malformed manifests, and ownership conflicts.
pub fn scan(crate_dir: &Path) -> Result<Discovery, String> {
    if !crate_dir.is_absolute()
        || crate_dir.components().any(|component| {
            !matches!(
                component,
                Component::RootDir | Component::Prefix(_) | Component::Normal(_)
            )
        })
    {
        return Err(format!(
            "non-normal absolute crate path: {}",
            crate_dir.display()
        ));
    }
    require_directory(crate_dir)?;
    let canonical_crate = crate_dir
        .canonicalize()
        .map_err(|error| input_error(crate_dir, &error))?;
    let root = canonical_crate.join("src/bidders");
    require_directory(&root)?;
    let manifest_path = canonical_crate.join("Cargo.toml");
    let manifest: Manifest = toml::from_str(&input_text(&manifest_path)?).map_err(|error| {
        format!(
            "invalid Cargo manifest {}: {}",
            manifest_path.display(),
            error.message()
        )
    })?;
    let declared_features = manifest.features.keys().cloned().collect();
    let catalog =
        build_catalog::scan(&root, &declared_features).map_err(|error| error.to_string())?;
    let mut rerun_paths = catalog.input_paths;
    rerun_paths.insert(manifest_path);
    let mut declarations = Vec::new();
    let mut uid_policies = Vec::new();
    for bidder in catalog.manifests.values() {
        let directory = root.join(&bidder.key);
        let policy_source = directory.join("uid_policy.rs");
        if rerun_paths.contains(&policy_source) {
            require_file(&policy_source)?;
            uid_policies.push((bidder.key.clone(), policy_source));
        }
        if !bidder.factory {
            continue;
        }
        let allocation_source = directory.join("tests/allocation_conformance.rs");
        let allocation_conformance = if rerun_paths.contains(&allocation_source) {
            require_file(&allocation_source)?;
            Some(allocation_source)
        } else {
            None
        };
        let response_source = directory.join("tests/response_conformance.rs");
        let response_conformance = if rerun_paths.contains(&response_source) {
            require_file(&response_source)?;
            Some(response_source)
        } else {
            None
        };
        declarations.push((
            directory,
            Declaration {
                key: bidder.key.clone(),
                required_feature: bidder.required_feature.clone(),
                native_only: bidder.native_only,
                has_settings: bidder.settings.is_some(),
                allocation_conformance,
                response_conformance,
            },
        ));
    }
    Ok(Discovery {
        manifests: catalog.manifests,
        declarations,
        uid_policies,
        features: manifest.features,
        rerun_paths,
    })
}

fn input_text(path: &Path) -> Result<String, String> {
    require_file(path)?;
    fs::read_to_string(path).map_err(|error| input_error(path, &error))
}

fn input_error(path: &Path, error: &io::Error) -> String {
    if error.kind() == io::ErrorKind::NotFound {
        format!("{}: missing input", path.display())
    } else {
        format!("{}: cannot read input", path.display())
    }
}

fn input_metadata(path: &Path) -> Result<fs::Metadata, String> {
    // Check ancestors too: symlink_metadata on a leaf alone follows parent links.
    let mut prefix = PathBuf::new();
    for component in path.components() {
        prefix.push(component);
        let metadata =
            fs::symlink_metadata(&prefix).map_err(|error| input_error(&prefix, &error))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("{}: symlink input", prefix.display()));
        }
        if !metadata.is_file() && !metadata.is_dir() {
            return Err(format!("{}: nonregular input", prefix.display()));
        }
    }
    let metadata = fs::symlink_metadata(path).map_err(|error| input_error(path, &error))?;
    if !metadata.is_file() && !metadata.is_dir() {
        return Err(format!("{}: nonregular input", path.display()));
    }
    Ok(metadata)
}

fn require_file(path: &Path) -> Result<(), String> {
    if !input_metadata(path)?.is_file() {
        return Err(format!("{}: expected regular file", path.display()));
    }
    Ok(())
}

fn require_directory(path: &Path) -> Result<(), String> {
    if !input_metadata(path)?.is_dir() {
        return Err(format!("{}: expected directory", path.display()));
    }
    Ok(())
}

fn render_published_registry_builder(
    arms: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    quote::quote! {
        #[doc = "Construct the published registry from build-validated bindings only."]
        #[doc = "Unpublished candidate factories are not referenced by this path."]
        #[doc = "# Errors"]
        #[doc = "Returns a structural error before construction if published metadata is inconsistent."]
        pub(crate) fn configured_registry() -> Result<crate::BidderRegistry, crate::RegistryBuildError> {
            let factory = |name: &'static str| {
                let constructor: Option<fn() -> Box<dyn crate::Bidder>> = match name {
                    #(#arms)*
                    _ => None,
                };
                constructor.ok_or(crate::RegistryBuildError::UndeclaredBidder { bidder: name })
            };
            let infos = auctioneer_config_data::bidders();
            let bindings = auctioneer_config_data::bidder_implementation_bindings();
            // Preflight borrowed rows without retaining or allocating catalog indexes.
            // Published bindings and feature predicates are checked at build time.
            for (index, info) in infos.iter().enumerate() {
                let name = info.name.as_str();
                if infos.iter().take(index).any(|prior| prior.name.as_str().eq_ignore_ascii_case(name)) {
                    return Err(crate::RegistryBuildError::DuplicateBidder { bidder: name });
                }
                factory(name)?;
                if !bindings.iter().any(|binding| binding.bidder == name) {
                    return Err(crate::RegistryBuildError::MissingBinding { bidder: name });
                }
            }
            for binding in bindings {
                if !infos.iter().any(|info| info.name.as_str() == binding.bidder) {
                    return Err(crate::RegistryBuildError::DanglingBinding { bidder: binding.bidder });
                }
            }
            let mut registry = crate::BidderRegistry::new();
            for info in infos {
                let binding = bindings.iter().find(|binding| binding.bidder == info.name.as_str())
                    .ok_or(crate::RegistryBuildError::MissingBinding { bidder: info.name.as_str() })?;
                registry.register_with_implementation(info, binding.implementation, factory(info.name.as_str())?())?;
            }
            Ok(registry)
        }
    }
}

impl Discovery {
    /// Complete declarations for typed settings and endpoints, independent of factory eligibility.
    #[must_use]
    pub fn manifests(&self) -> &BTreeMap<String, build_catalog::Manifest> {
        &self.manifests
    }

    fn native_only(&self, implementation: &str) -> bool {
        self.declarations
            .binary_search_by(|(_, declaration)| declaration.key.as_str().cmp(implementation))
            .ok()
            .and_then(|index| self.declarations.get(index))
            .is_some_and(|(_, declaration)| declaration.native_only)
    }

    fn publication_gate(&self, row: &BidderMetadataDeclaration) -> proc_macro2::TokenStream {
        publication_gate(self.native_only(row.implementation), row.required_feature)
    }

    /// Reconcile all metadata declarations before filtering disabled features.
    ///
    /// # Errors
    /// Rejects missing factories, mismatched feature ownership, or missing config forwarding.
    pub fn validate_metadata(&self, metadata: &[BidderMetadataDeclaration]) -> Result<(), String> {
        bidder_reconcile::validate(
            metadata,
            self.declarations.iter().map(|(_, declaration)| {
                (
                    declaration.key.as_str(),
                    declaration.required_feature.as_deref(),
                )
            }),
            &self.features,
        )
    }

    /// Every scanned input, including directories needed to track additions and removals.
    #[must_use]
    pub fn rerun_paths(&self) -> &BTreeSet<PathBuf> {
        &self.rerun_paths
    }

    /// Render owner-local UID policies without promoting reference-only packages to factories.
    ///
    /// # Errors
    /// Rejects non-UTF-8 source paths or invalid owner identifiers.
    pub fn render_uid_policies(&self) -> Result<String, String> {
        let mut modules = Vec::new();
        let mut arms = Vec::new();
        for (key, source) in &self.uid_policies {
            let path = source
                .to_str()
                .ok_or_else(|| format!("non-UTF-8 UID policy source: {}", source.display()))?;
            let owner = syn::parse_str::<syn::Ident>(key).map_err(|error| error.to_string())?;
            modules.push(quote::quote!(#[path = #path] mod #owner;));
            arms.push(quote::quote!(#owner::FAMILY => #owner::accepts_uid(uid),));
        }
        let (parameters, body) = if self.uid_policies.is_empty() {
            (
                quote::quote!(_family: &str, _uid: &str),
                quote::quote!(true),
            )
        } else {
            (
                quote::quote!(family: &str, uid: &str),
                quote::quote!(match family { #(#arms)* _ => true }),
            )
        };
        Ok(quote::quote! {
            mod sync_uid_policies {
                #(#modules)*

                #[doc = "Apply owner-local UID exclusions; unknown families accept opaque UIDs."]
                #[doc = "Empty UID and opt-out behavior remain the caller's responsibility."]
                #[must_use]
                #[deny(unreachable_patterns)]
                pub fn accepts_sync_uid(#parameters) -> bool { #body }
            }
            pub use sync_uid_policies::accepts_sync_uid;
        }
        .to_string())
    }

    /// Render serving dispatch for validated published bindings only.
    ///
    /// # Errors
    /// Rejects invalid metadata/factory associations and invalid factory identifiers.
    pub fn render_registry(
        &self,
        metadata: &[BidderMetadataDeclaration],
    ) -> Result<String, String> {
        self.validate_metadata(metadata)?;
        let mut rows: Vec<_> = metadata.iter().collect();
        rows.sort_unstable_by_key(|row| row.bidder);
        let expected = rows.iter().map(|row| {
            let public = row.bidder;
            let implementation = row.implementation;
            let gate = self.publication_gate(row);
            quote::quote!(#gate (#public, #implementation),)
        });
        let target_declarations = rows.iter().map(|row| {
            let bidder = row.bidder;
            let implementation = row.implementation;
            // Keep inactive features for strict feature-parity validation. Only
            // source-declared target ineligibility removes a declaration here.
            let gate = publication_gate(self.native_only(implementation), None);
            let feature = row.required_feature.map_or_else(
                || quote::quote!(None),
                |feature| quote::quote!(Some(#feature)),
            );
            quote::quote!(#gate BidderMetadataDeclaration {
                bidder: #bidder, implementation: #implementation, required_feature: #feature,
            },)
        });
        let mut factories: BTreeMap<_, Vec<_>> = BTreeMap::new();
        for row in &rows {
            factories
                .entry((
                    row.implementation,
                    row.required_feature,
                    self.native_only(row.implementation),
                ))
                .or_default()
                .push(row.bidder);
        }
        let arms = factories
            .iter()
            .map(|((key, feature, native_only), public)| {
                // Reconciliation has checked the factory key and feature predicate.
                let implementation =
                    syn::parse_str::<syn::Ident>(key).map_err(|error| error.to_string())?;
                let gate = publication_gate(*native_only, *feature);
                Ok::<_, String>(
                    quote::quote!(#gate #(#public)|* => Some(#implementation::create_bidder),),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let registry_builder = render_published_registry_builder(&arms);
        Ok(quote::quote! {
            use auctioneer_config_data::types::BidderMetadataDeclaration;

            #[doc = "Complete target-eligible metadata declarations, before feature activation."]
            #[doc = "Generated from source publication gates, never inferred from active metadata or bindings."]
            #[must_use]
            pub fn target_metadata_declarations() -> &'static [BidderMetadataDeclaration] {
                &[#(#target_declarations)*]
            }
            // This is evaluated, not retained: no runtime catalog or heap index.
            // Compare target-crate features too, not just host build dependencies.
            const _: () = {
                const fn same(left: &str, right: &str) -> bool {
                    let mut left_bytes = left.as_bytes();
                    let mut right_bytes = right.as_bytes();
                    loop {
                        match (left_bytes, right_bytes) {
                            ([], []) => return true,
                            ([left_byte, left_tail @ ..], [right_byte, right_tail @ ..]) => {
                                if *left_byte != *right_byte { return false; }
                                left_bytes = left_tail;
                                right_bytes = right_tail;
                            }
                            _ => return false,
                        }
                    }
                }
                let mut actual = auctioneer_config_data::bidder_implementation_bindings();
                let mut expected: &[(&str, &str)] = &[#(#expected)*];
                let mut valid = true;
                loop {
                    match (actual, expected) {
                        ([], []) => break,
                        ([binding, actual_tail @ ..], [(public, implementation), expected_tail @ ..]) => {
                            if !same(binding.bidder, public) || !same(binding.implementation, implementation) {
                                valid = false;
                                break;
                            }
                            actual = actual_tail;
                            expected = expected_tail;
                        }
                        _ => { valid = false; break; }
                    }
                }
                assert!(valid, "compiled bidder bindings differ from config");
            };
            #registry_builder
        }.to_string())
    }

    /// Render native allocation tests and the nonempty enabled-coverage control.
    ///
    /// # Errors
    /// Rejects non-UTF-8 source paths or invalid owner identifiers.
    pub fn render_allocation_conformance(&self) -> Result<String, String> {
        let mut modules = Vec::new();
        let mut coverage = Vec::new();
        for (_, declaration) in &self.declarations {
            let Some(source) = &declaration.allocation_conformance else {
                continue;
            };
            let path = source
                .to_str()
                .ok_or_else(|| format!("non-UTF-8 allocation source path: {}", source.display()))?;
            let owner = syn::parse_str::<syn::Ident>(&declaration.key)
                .map_err(|error| error.to_string())?;
            let guard = declaration
                .required_feature
                .as_ref()
                .map(|feature| quote::quote!(#[cfg(feature = #feature)]));
            modules.push(quote::quote!(#guard #[path = #path] mod #owner;));
            coverage.push(quote::quote!(#guard ()));
        }
        Ok(quote::quote! {
            #[cfg(test)]
            #[cfg(not(target_arch = "wasm32"))]
            mod bidder_allocation_conformance {
                #(#modules)*
                #[test]
                fn discovered_allocation_conformance_is_nonempty() {
                    let discovered: &[()] = &[#(#coverage),*];
                    assert!(!discovered.is_empty(), "no enabled bidder allocation conformance was discovered");
                }
            }
        }.to_string())
    }

    /// Render native response tests and nonempty, nonzero case-count controls.
    ///
    /// # Errors
    /// Rejects non-UTF-8 source paths or invalid owner identifiers.
    pub fn render_response_conformance(&self) -> Result<String, String> {
        let mut modules = Vec::new();
        let mut coverage = Vec::new();
        for (_, declaration) in &self.declarations {
            let Some(source) = &declaration.response_conformance else {
                continue;
            };
            let path = source
                .to_str()
                .ok_or_else(|| format!("non-UTF-8 response source path: {}", source.display()))?;
            let owner = syn::parse_str::<syn::Ident>(&declaration.key)
                .map_err(|error| error.to_string())?;
            let guard = declaration
                .required_feature
                .as_ref()
                .map(|feature| quote::quote!(#[cfg(feature = #feature)]));
            modules.push(quote::quote!(#guard #[path = #path] mod #owner;));
            coverage.push(quote::quote!(#guard #owner::RESPONSE_CONFORMANCE_CASE_COUNT));
        }
        Ok(quote::quote! {
            #[cfg(test)]
            #[cfg(not(target_arch = "wasm32"))]
            mod bidder_response_conformance {
                #(#modules)*
                #[test]
                fn discovered_response_conformance_is_nonempty() {
                    let discovered: &[usize] = &[#(#coverage),*];
                    assert!(!discovered.is_empty(), "no enabled bidder response conformance was discovered");
                    assert!(discovered.iter().all(|count| *count > 0), "response conformance module declared zero cases");
                }
            }
        }.to_string())
    }

    /// Render discovered factories, unfiltered declarations, and native contract tests.
    ///
    /// # Errors
    /// Rejects non-UTF-8 source/fixture paths or invalid factory identifiers.
    pub fn render(&self) -> Result<String, String> {
        let mut modules = Vec::new();
        let mut declarations = Vec::new();
        let mut registrations = Vec::new();
        let mut contracts = Vec::new();
        for (directory, declaration) in &self.declarations {
            let source = directory.join("mod.rs");
            let path = source
                .to_str()
                .ok_or_else(|| format!("non-UTF-8 source path: {}", source.display()))?;
            let owner = syn::parse_str::<syn::Ident>(&declaration.key)
                .map_err(|error| error.to_string())?;
            let key = &declaration.key;
            let guard = declaration
                .required_feature
                .as_ref()
                .map(|feature| quote::quote!(#[cfg(feature = #feature)]));
            let feature = declaration.required_feature.as_ref().map_or_else(
                || quote::quote!(None),
                |feature| quote::quote!(Some(#feature)),
            );
            let has_settings = declaration.has_settings;
            let settings_obligation = match (has_settings, &declaration.required_feature) {
                (true, Some(required_feature)) => quote::quote!(cfg!(feature = #required_feature)),
                (true, None) => quote::quote!(true),
                (false, _) => quote::quote!(false),
            };
            let config_validator = if has_settings {
                quote::quote!(Some(#owner::validate_config))
            } else {
                quote::quote!(None)
            };
            modules.push(quote::quote!(#guard #[path = #path] mod #owner;));
            declarations.push(quote::quote!(BidderDeclaration {
                key: #key,
                required_feature: #feature,
                has_settings: #settings_obligation,
            }));
            registrations.push(quote::quote!(#guard BidderRegistration {
                key: #key,
                factory: #owner::create_bidder,
                config_validator: #config_validator,
            }));
            let fixture = directory.join("fixtures/factory.json");
            let fixture_path = fixture
                .to_str()
                .ok_or_else(|| "non-UTF-8 factory fixture path".to_owned())?;
            contracts.push(quote::quote! {
                #guard
                #[test]
                fn #owner() {
                    super::harness::assert_factory_contract(#key, include_str!(#fixture_path));
                }
            });
        }
        Ok(quote::quote! {
            #(#modules)*
            pub(crate) static ALL_DECLARATIONS: &[BidderDeclaration] = &[#(#declarations),*];
            static COMPILED_BIDDERS: &[BidderRegistration] = &[#(#registrations),*];
            #[doc = "Borrow compiled factories without constructing any bidders."]
            #[must_use]
            pub fn compiled_bidders() -> &'static [BidderRegistration] {
                COMPILED_BIDDERS
            }
            #[cfg(test)]
            #[cfg(not(target_arch = "wasm32"))]
            mod factory_contract_tests {
                #(#contracts)*
            }
        }
        .to_string())
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use super::scan;
    use crate::{bidder_endpoint_codegen, bidder_settings_codegen};
    use quote::ToTokens as _;
    use std::env::consts::EXE_SUFFIX;
    use std::error::Error;
    use std::fs;
    use std::io::{self, Read as _, Seek as _};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output, Stdio};
    use std::thread;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    type TestResult<T = ()> = Result<T, Box<dyn Error>>;

    fn contains_tokens(source: &str, expected: &proc_macro2::TokenStream) -> bool {
        source
            .parse::<proc_macro2::TokenStream>()
            .unwrap()
            .to_string()
            .contains(&expected.to_string())
    }

    fn external_modules(parsed: &syn::File) -> impl Iterator<Item = &syn::ItemMod> {
        parsed.items.iter().filter_map(|item| {
            if let syn::Item::Mod(module) = item
                && module.content.is_none()
            {
                Some(module)
            } else {
                None
            }
        })
    }

    struct Fixture {
        _temporary: TempDir,
        directory: PathBuf,
        root: PathBuf,
    }

    impl Fixture {
        fn new() -> TestResult<Self> {
            let temporary = tempfile::tempdir()?;
            let directory = temporary.path().canonicalize()?;
            let root = directory.join("src/bidders");
            fs::create_dir_all(&root)?;
            fs::write(
                directory.join("Cargo.toml"),
                "[features]\nexperimental = []\n",
            )?;
            Ok(Self {
                _temporary: temporary,
                directory,
                root,
            })
        }

        fn bidder(&self, directory: &str, declaration: &str) -> TestResult<PathBuf> {
            let path = self.root.join(directory);
            fs::create_dir_all(path.join("tests"))?;
            fs::write(path.join("bidder.toml"), declaration)?;
            fs::write(
                path.join("mod.rs"),
                "not parsed or executed during discovery",
            )?;
            Ok(path)
        }

        fn local_factory(&self, key: &str) -> TestResult<PathBuf> {
            self.bidder(
                key,
                &format!("key = '{key}'\nfactory = true\n[provenance]\norigin = 'local'\n"),
            )
        }
    }

    #[test]
    fn settings_catalog_retains_empty_reference_and_inactive_owners_without_factory_promotion() {
        let fixture = Fixture::new().unwrap();
        let empty = scan(&fixture.directory).unwrap();
        assert!(empty.manifests().is_empty());
        syn::parse_file(&bidder_settings_codegen::render(empty.manifests()).unwrap()).unwrap();
        for (key, factory, gate) in [
            ("fake_reference", false, ""),
            ("fake_disabled", true, "required_feature = 'experimental'\n"),
            ("fake_native", true, "native_only = true\n"),
        ] {
            let owner = fixture.bidder(key, &format!(
                "key = '{key}'\nfactory = {factory}\n{gate}[settings.fields]\nreference = 'optional_secret_reference'\n[endpoint.fields]\nroute = 'RouteID'\n[provenance]\norigin = 'local'\n"
            )).unwrap();
            if !factory {
                fs::remove_file(owner.join("mod.rs")).unwrap();
            }
        }
        let discovery = scan(&fixture.directory).unwrap();
        assert_eq!(discovery.manifests().len(), 3);
        let settings = bidder_settings_codegen::render(discovery.manifests()).unwrap();
        let endpoints = bidder_endpoint_codegen::render(discovery.manifests()).unwrap();
        for key in ["fake_reference", "fake_disabled", "fake_native"] {
            assert!(settings.contains(&format!("pub mod {key}")));
            assert!(endpoints.contains(&format!("pub mod {key}")));
        }
        assert!(!discovery.render().unwrap().contains("fake_reference"));
        assert_eq!(discovery.declarations.len(), 2);
    }

    #[test]
    fn uid_policy_discovery_tracks_all_manifest_roles_without_factory_promotion() {
        for factory in [false, true] {
            let fixture = Fixture::new().unwrap();
            let bidder = fixture
                .bidder(
                    "fake_uid_probe",
                    &format!("key = 'fake_uid_probe'\nfactory = {factory}\n[provenance]\norigin = 'local'\n"),
                )
                .unwrap();
            if !factory {
                fs::remove_file(bidder.join("mod.rs")).unwrap();
            }
            let source = bidder.join("uid_policy.rs");
            let initial = scan(&fixture.directory).unwrap();
            let factories = initial.render().unwrap();
            let fallback = initial.render_uid_policies().unwrap();
            assert!(initial.rerun_paths().contains(&bidder));
            assert!(!initial.rerun_paths().contains(&source));
            assert!(!fallback.contains(source.to_str().unwrap()));

            fs::write(
                &source,
                "not Rust: discovery must not interpret policy source",
            )
            .unwrap();
            let added = scan(&fixture.directory).unwrap();
            let generated = added.render_uid_policies().unwrap();
            assert!(added.rerun_paths().contains(&source));
            assert!(generated.contains(source.to_str().unwrap()));
            assert_eq!(added.render().unwrap(), factories);
            fs::write(&source, [0xff_u8]).unwrap();
            let edited = scan(&fixture.directory).unwrap();
            assert_eq!(edited.rerun_paths(), added.rerun_paths());
            assert_eq!(edited.render_uid_policies().unwrap(), generated);

            fs::remove_file(&source).unwrap();
            let removed = scan(&fixture.directory).unwrap();
            assert!(removed.rerun_paths().contains(&bidder));
            assert!(!removed.rerun_paths().contains(&source));
            assert_eq!(removed.render_uid_policies().unwrap(), fallback);
            assert_eq!(removed.render().unwrap(), factories);
        }
    }

    fn compile_uid_policy_probe(fixture: &Fixture, assertions: &str) -> Output {
        let generated = scan(&fixture.directory)
            .unwrap()
            .render_uid_policies()
            .unwrap();
        let source = fixture.directory.join("uid_probe.rs");
        let binary = fixture.directory.join(format!("uid_probe{EXE_SUFFIX}"));
        let generated_syntax = syn::parse_file(&generated).unwrap();
        let assertion_tokens: proc_macro2::TokenStream = assertions.parse().unwrap();
        fs::write(
            &source,
            quote::quote! {
                mod bidders { #generated_syntax }
                fn main() { #assertion_tokens }
            }
            .to_string(),
        )
        .unwrap();
        // Do not pass -Dwarnings: duplicate families must be denied by generated code.
        let compiled = child_output(
            Command::new("rustc")
                .args(["--edition=2024", "--crate-name", "uid_policy_probe"])
                .arg(&source)
                .arg("-o")
                .arg(&binary),
        )
        .unwrap();
        if compiled.status.success() {
            let run = child_output(&mut Command::new(&binary)).unwrap();
            assert!(
                run.status.success(),
                "{}",
                String::from_utf8_lossy(&run.stderr)
            );
        }
        compiled
    }

    #[test]
    fn uid_policy_dispatch_compiles_lifecycle_and_denies_duplicate_families() {
        let fixture = Fixture::new().unwrap();
        let reference = fixture
            .bidder(
                "fake_reference_uid",
                "key = 'fake_reference_uid'\nfactory = false\nrequired_feature = 'experimental'\n[provenance]\norigin = 'local'\n",
            )
            .unwrap();
        fs::remove_file(reference.join("mod.rs")).unwrap();
        let factory = fixture.local_factory("fake_factory_uid").unwrap();
        let baseline = scan(&fixture.directory).unwrap().render().unwrap();
        let fallback = "for uid in [\"\", \"0\", \"valid\", \"changed\"] { assert!(bidders::accepts_sync_uid(\"fake_family\", uid)); }";
        let initial = compile_uid_policy_probe(&fixture, fallback);
        assert!(
            initial.status.success(),
            "{}",
            String::from_utf8_lossy(&initial.stderr)
        );

        let policy = reference.join("uid_policy.rs");
        fs::write(&policy, "pub(super) const FAMILY: &str = \"fake_family\";\npub(super) fn accepts_uid(uid: &str) -> bool { uid != \"0\" }\n").unwrap();
        let added = compile_uid_policy_probe(
            &fixture,
            "assert!(!bidders::accepts_sync_uid(\"fake_family\", \"0\")); for uid in [\"\", \"0\", \"valid\"] { assert!(bidders::accepts_sync_uid(\"fake_unknown\", uid)); } assert!(bidders::accepts_sync_uid(\"fake_family\", \"valid\")); assert!(bidders::accepts_sync_uid(\"fake_family\", \"\"));",
        );
        assert!(
            added.status.success(),
            "{}",
            String::from_utf8_lossy(&added.stderr)
        );
        assert_eq!(
            scan(&fixture.directory).unwrap().render().unwrap(),
            baseline
        );

        fs::write(&policy, "pub(super) const FAMILY: &str = \"fake_family\";\npub(super) fn accepts_uid(uid: &str) -> bool { uid != \"changed\" }\n").unwrap();
        let edited = compile_uid_policy_probe(
            &fixture,
            "assert!(bidders::accepts_sync_uid(\"fake_family\", \"0\")); assert!(!bidders::accepts_sync_uid(\"fake_family\", \"changed\"));",
        );
        assert!(
            edited.status.success(),
            "{}",
            String::from_utf8_lossy(&edited.stderr)
        );

        let duplicate = factory.join("uid_policy.rs");
        fs::write(&duplicate, "pub(super) const FAMILY: &str = \"fake_family\";\npub(super) fn accepts_uid(_uid: &str) -> bool { true }\n").unwrap();
        let refused = compile_uid_policy_probe(&fixture, fallback);
        assert!(!refused.status.success(), "duplicate policy compiled");
        let diagnostic = String::from_utf8_lossy(&refused.stderr);
        assert!(diagnostic.contains("unreachable pattern"), "{diagnostic}");
        assert!(
            diagnostic.contains("lint level is defined here"),
            "{diagnostic}"
        );

        fs::remove_file(duplicate).unwrap();
        let restored = compile_uid_policy_probe(
            &fixture,
            "assert!(!bidders::accepts_sync_uid(\"fake_family\", \"changed\"));",
        );
        assert!(
            restored.status.success(),
            "{}",
            String::from_utf8_lossy(&restored.stderr)
        );
        fs::remove_file(policy).unwrap();
        let removed = compile_uid_policy_probe(&fixture, fallback);
        assert!(
            removed.status.success(),
            "{}",
            String::from_utf8_lossy(&removed.stderr)
        );
        assert_eq!(
            scan(&fixture.directory).unwrap().render().unwrap(),
            baseline
        );
    }

    #[test]
    fn allocation_conformance_tracks_synthetic_add_edit_and_remove_without_registration() {
        let fixture = Fixture::new().unwrap();
        let bidder = fixture.local_factory("fake_allocation_probe").unwrap();
        let source = bidder.join("tests/allocation_conformance.rs");
        let initial = scan(&fixture.directory).unwrap();
        assert!(initial.rerun_paths().contains(&bidder));
        assert!(!initial.rerun_paths().contains(&source));
        let initial_render = initial.render_allocation_conformance().unwrap();
        assert!(!contains_tokens(
            &initial_render,
            &quote::quote!(
                mod fake_allocation_probe;
            )
        ));

        // Discovery must not parse, compile or execute bidder source.
        fs::write(&source, "not Rust: the allocation runner owns compilation").unwrap();
        let added = scan(&fixture.directory).unwrap();
        assert!(added.rerun_paths().contains(&source));
        let rendered = added.render_allocation_conformance().unwrap();
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(
                mod fake_allocation_probe;
            )
        ));
        assert!(rendered.contains(source.to_str().unwrap()));
        fs::write(&source, [0xff_u8]).unwrap();
        let edited = scan(&fixture.directory).unwrap();
        assert_eq!(added.rerun_paths(), edited.rerun_paths());
        assert_eq!(rendered, edited.render_allocation_conformance().unwrap());

        fs::remove_file(&source).unwrap();
        let removed = scan(&fixture.directory).unwrap();
        assert!(removed.rerun_paths().contains(&bidder));
        assert!(!removed.rerun_paths().contains(&source));
        assert_eq!(
            initial_render,
            removed.render_allocation_conformance().unwrap()
        );
    }

    #[test]
    fn allocation_conformance_preserves_factory_features_and_native_test_ownership() {
        let fixture = Fixture::new().unwrap();
        let plain = fixture.local_factory("fake_alpha_probe").unwrap();
        let gated = fixture.bidder("fake_beta_probe", "key = 'fake_beta_probe'\nfactory = true\nrequired_feature = 'experimental'\n[provenance]\norigin = 'local'\n").unwrap();
        let reference = fixture
            .bidder(
                "fake_reference_probe",
                "key = 'fake_reference_probe'\nfactory = false\n[provenance]\norigin = 'local'\n",
            )
            .unwrap();
        fs::remove_file(reference.join("mod.rs")).unwrap();
        for bidder in [&plain, &gated, &reference] {
            fs::write(
                bidder.join("tests/allocation_conformance.rs"),
                "not executed",
            )
            .unwrap();
        }
        fixture.local_factory("fake_no_allocation_probe").unwrap();
        let rendered = scan(&fixture.directory)
            .unwrap()
            .render_allocation_conformance()
            .unwrap();
        let parsed = syn::parse_file(&rendered).unwrap();
        let syn::Item::Mod(runner) = &parsed.items[0] else {
            panic!("expected a native allocation runner module");
        };
        assert_eq!(runner.ident, "bidder_allocation_conformance");
        let cfgs: Vec<_> = runner
            .attrs
            .iter()
            .filter_map(|attr| {
                if let syn::Meta::List(list) = &attr.meta
                    && list.path.is_ident("cfg")
                {
                    Some(list.tokens.to_string())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(cfgs, ["test", "not (target_arch = \"wasm32\")"]);
        let modules: Vec<_> = runner
            .content
            .as_ref()
            .unwrap()
            .1
            .iter()
            .filter_map(|item| {
                if let syn::Item::Mod(module) = item {
                    Some(module)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0].ident, "fake_alpha_probe");
        assert_eq!(modules[1].ident, "fake_beta_probe");
        assert!(
            !modules[0]
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("cfg"))
        );
        assert!(modules[1].attrs.iter().any(|attr| {
            matches!(&attr.meta, syn::Meta::List(list) if list.path.is_ident("cfg") && list.tokens.to_string() == "feature = \"experimental\"")
        }));
        assert!(!rendered.contains("fake_reference_probe"));
        assert!(!rendered.contains("fake_no_allocation_probe"));
        assert!(rendered.contains("discovered_allocation_conformance_is_nonempty"));
    }

    fn compile_conformance(
        source: &Path,
        generated: &Path,
        binary: &Path,
        crate_name: &str,
        enabled: bool,
    ) -> Output {
        let mut compiler = Command::new("rustc");
        compiler
            .env("OUT_DIR", generated)
            .args([
                "--edition=2024",
                "--test",
                "-Dwarnings",
                "--crate-name",
                crate_name,
            ])
            .arg(source)
            .arg("-o")
            .arg(binary);
        if enabled {
            compiler.args(["--cfg", "feature=\"experimental\""]);
        }
        child_output(&mut compiler).unwrap()
    }

    #[test]
    fn rustc_runs_discovered_allocation_cases_after_edits_feature_changes_and_removal() {
        let fixture = Fixture::new().unwrap();
        let plain = fixture.local_factory("fake_alpha_probe").unwrap();
        let gated = fixture.bidder("fake_beta_probe", "key = 'fake_beta_probe'\nfactory = true\nrequired_feature = 'experimental'\n[provenance]\norigin = 'local'\n").unwrap();
        let plain_case = plain.join("tests/allocation_conformance.rs");
        let gated_case = gated.join("tests/allocation_conformance.rs");
        fs::write(&gated_case, "#[test] fn gated_allocation() { assert_eq!(crate::answer(), 7); println!(\"gated-case\"); }").unwrap();
        let source = fixture.directory.join("allocation_runner.rs");
        fs::write(&source, "fn answer() -> u32 { 7 }\ninclude!(concat!(env!(\"OUT_DIR\"), \"/bidder_allocation_conformance.rs\"));").unwrap();
        let generated = fixture.directory.join("output");
        fs::create_dir_all(&generated).unwrap();
        let binary = generated.join(format!("fake_allocation_probe{EXE_SUFFIX}"));
        for revision in ["original-case", "edited-case"] {
            fs::write(
                &plain_case,
                quote::quote! {
                    #[test]
                    fn plain_allocation() {
                        assert_eq!(crate::answer(), 7);
                        println!(#revision);
                    }
                }
                .to_string(),
            )
            .unwrap();
            let discovery = scan(&fixture.directory).unwrap();
            assert!(discovery.rerun_paths().contains(&plain_case));
            fs::write(
                generated.join("bidder_allocation_conformance.rs"),
                discovery.render_allocation_conformance().unwrap(),
            )
            .unwrap();
            for enabled in [false, true, false] {
                let compiled = compile_conformance(
                    &source,
                    &generated,
                    &binary,
                    "fake_allocation_probe",
                    enabled,
                );
                assert!(
                    compiled.status.success(),
                    "{}",
                    String::from_utf8_lossy(&compiled.stderr)
                );
                let output = child_output(Command::new(&binary).arg("--nocapture")).unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let stdout = String::from_utf8(output.stdout).unwrap();
                assert!(stdout.contains(revision), "{stdout}");
                assert_eq!(stdout.contains("gated-case"), enabled, "{stdout}");
                assert!(
                    stdout.contains(if enabled { "3 passed" } else { "2 passed" }),
                    "{stdout}"
                );
            }
        }
        fs::remove_file(&gated_case).unwrap();
        let removed = scan(&fixture.directory).unwrap();
        assert!(!removed.rerun_paths().contains(&gated_case));
        assert!(
            !removed
                .render_allocation_conformance()
                .unwrap()
                .contains("mod fake_beta_probe;")
        );
        fs::write(
            generated.join("bidder_allocation_conformance.rs"),
            removed.render_allocation_conformance().unwrap(),
        )
        .unwrap();
        let compiled =
            compile_conformance(&source, &generated, &binary, "fake_allocation_probe", true);
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = child_output(Command::new(&binary).arg("--nocapture")).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.contains("2 passed") && stdout.contains("edited-case"),
            "{stdout}"
        );
        assert!(!stdout.contains("gated-case"), "{stdout}");
    }

    #[test]
    fn allocation_conformance_requires_a_regular_adjacent_file() {
        let fixture = Fixture::new().unwrap();
        let bidder = fixture.local_factory("fake_allocation_probe").unwrap();
        let source = bidder.join("tests/allocation_conformance.rs");
        fs::create_dir_all(&source).unwrap();
        assert_eq!(
            scan(&fixture.directory).unwrap_err(),
            format!("{}: expected regular file", source.display()),
        );
    }

    #[test]
    fn allocation_runner_rejects_empty_or_entirely_disabled_conformance() {
        let fixture = Fixture::new().unwrap();
        let bidder = fixture.bidder("fake_gated_probe", "key = 'fake_gated_probe'\nfactory = true\nrequired_feature = 'experimental'\n[provenance]\norigin = 'local'\n").unwrap();
        let conformance = bidder.join("tests/allocation_conformance.rs");
        fs::write(&conformance, "#[test] fn synthetic_allocation_case() {}\n").unwrap();
        let source = fixture.directory.join("allocation_runner.rs");
        fs::write(
            &source,
            "include!(concat!(env!(\"OUT_DIR\"), \"/bidder_allocation_conformance.rs\"));\n",
        )
        .unwrap();
        let generated = fixture.directory.join("output");
        fs::create_dir_all(&generated).unwrap();
        let binary = generated.join(format!("fake_allocation_probe{EXE_SUFFIX}"));
        for present in [true, false] {
            if !present {
                fs::remove_file(&conformance).unwrap();
            }
            let discovery = scan(&fixture.directory).unwrap();
            fs::write(
                generated.join("bidder_allocation_conformance.rs"),
                discovery.render_allocation_conformance().unwrap(),
            )
            .unwrap();
            for enabled in [false, true] {
                let mut rustc = Command::new("rustc");
                rustc
                    .env("OUT_DIR", &generated)
                    .args(["--edition=2024", "--test", "-Dwarnings"])
                    .arg(&source)
                    .arg("-o")
                    .arg(&binary);
                if enabled {
                    rustc.args(["--cfg", "feature=\"experimental\""]);
                }
                let compiled = child_output(&mut rustc).unwrap();
                assert!(
                    compiled.status.success(),
                    "{}",
                    String::from_utf8_lossy(&compiled.stderr)
                );
                let output = child_output(Command::new(&binary).arg("--nocapture")).unwrap();
                assert_eq!(output.status.success(), present && enabled);
                let stdout = String::from_utf8(output.stdout).unwrap();
                let stderr = String::from_utf8(output.stderr).unwrap();
                if present && enabled {
                    assert!(stdout.contains("2 passed"), "{stdout}");
                } else {
                    assert!(
                        stderr.contains("no enabled bidder allocation conformance was discovered"),
                        "{stdout}\n{stderr}"
                    );
                }
            }
        }
    }

    #[test]
    fn response_conformance_tracks_synthetic_add_edit_and_remove_without_registration() {
        let fixture = Fixture::new().unwrap();
        let bidder = fixture.local_factory("fake_response_probe").unwrap();
        let source = bidder.join("tests/response_conformance.rs");
        let initial = scan(&fixture.directory).unwrap();
        assert!(initial.rerun_paths().contains(&bidder));
        assert!(!initial.rerun_paths().contains(&source));
        let initial_render = initial.render_response_conformance().unwrap();
        assert!(!contains_tokens(
            &initial_render,
            &quote::quote!(
                mod fake_response_probe;
            )
        ));

        // Discovery must not parse, compile or execute bidder source.
        fs::write(&source, "not Rust: the response runner owns compilation").unwrap();
        let added = scan(&fixture.directory).unwrap();
        assert!(added.rerun_paths().contains(&source));
        let rendered = added.render_response_conformance().unwrap();
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(
                mod fake_response_probe;
            )
        ));
        assert!(rendered.contains(source.to_str().unwrap()));
        fs::write(&source, [0xff_u8]).unwrap();
        let edited = scan(&fixture.directory).unwrap();
        assert_eq!(added.rerun_paths(), edited.rerun_paths());
        assert_eq!(rendered, edited.render_response_conformance().unwrap());

        fs::remove_file(&source).unwrap();
        let removed = scan(&fixture.directory).unwrap();
        assert!(removed.rerun_paths().contains(&bidder));
        assert!(!removed.rerun_paths().contains(&source));
        assert_eq!(
            initial_render,
            removed.render_response_conformance().unwrap()
        );
    }

    #[test]
    fn response_conformance_preserves_factory_features_and_native_test_ownership() {
        let fixture = Fixture::new().unwrap();
        let plain = fixture.local_factory("fake_alpha_probe").unwrap();
        let gated = fixture.bidder("fake_beta_probe", "key = 'fake_beta_probe'\nfactory = true\nrequired_feature = 'experimental'\n[provenance]\norigin = 'local'\n").unwrap();
        let reference = fixture
            .bidder(
                "fake_reference_probe",
                "key = 'fake_reference_probe'\nfactory = false\n[provenance]\norigin = 'local'\n",
            )
            .unwrap();
        fs::remove_file(reference.join("mod.rs")).unwrap();
        for bidder in [&plain, &gated, &reference] {
            fs::write(bidder.join("tests/response_conformance.rs"), "not executed").unwrap();
        }
        fixture.local_factory("fake_no_response_probe").unwrap();
        let rendered = scan(&fixture.directory)
            .unwrap()
            .render_response_conformance()
            .unwrap();
        let parsed = syn::parse_file(&rendered).unwrap();
        let syn::Item::Mod(runner) = &parsed.items[0] else {
            panic!("expected a native response runner module");
        };
        assert_eq!(runner.ident, "bidder_response_conformance");
        let cfgs: Vec<_> = runner
            .attrs
            .iter()
            .filter_map(|attr| {
                if let syn::Meta::List(list) = &attr.meta
                    && list.path.is_ident("cfg")
                {
                    Some(list.tokens.to_string())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(cfgs, ["test", "not (target_arch = \"wasm32\")"]);
        let modules: Vec<_> = runner
            .content
            .as_ref()
            .unwrap()
            .1
            .iter()
            .filter_map(|item| {
                if let syn::Item::Mod(module) = item {
                    Some(module)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(modules.len(), 2);
        assert_eq!(modules[0].ident, "fake_alpha_probe");
        assert_eq!(modules[1].ident, "fake_beta_probe");
        assert!(
            !modules[0]
                .attrs
                .iter()
                .any(|attr| attr.path().is_ident("cfg"))
        );
        assert!(modules[1].attrs.iter().any(|attr| {
            matches!(&attr.meta, syn::Meta::List(list) if list.path.is_ident("cfg") && list.tokens.to_string() == "feature = \"experimental\"")
        }));
        assert!(!rendered.contains("fake_reference_probe"));
        assert!(!rendered.contains("fake_no_response_probe"));
        assert!(rendered.contains("discovered_response_conformance_is_nonempty"));
    }

    #[test]
    fn rustc_runs_discovered_response_cases_after_edits_feature_changes_and_removal() {
        let fixture = Fixture::new().unwrap();
        let plain = fixture.local_factory("fake_alpha_probe").unwrap();
        let gated = fixture.bidder("fake_beta_probe", "key = 'fake_beta_probe'\nfactory = true\nrequired_feature = 'experimental'\n[provenance]\norigin = 'local'\n").unwrap();
        let plain_case = plain.join("tests/response_conformance.rs");
        let gated_case = gated.join("tests/response_conformance.rs");
        fs::write(&gated_case, "pub(super) const RESPONSE_CONFORMANCE_CASE_COUNT: usize = 1; #[test] fn gated_response() { assert_eq!(crate::answer(), 7); println!(\"gated-case\"); }").unwrap();
        let source = fixture.directory.join("response_runner.rs");
        fs::write(&source, "fn answer() -> u32 { 7 }\ninclude!(concat!(env!(\"OUT_DIR\"), \"/bidder_response_conformance.rs\"));").unwrap();
        let generated = fixture.directory.join("output");
        fs::create_dir_all(&generated).unwrap();
        let binary = generated.join(format!("fake_response_probe{EXE_SUFFIX}"));
        for revision in ["original-case", "edited-case"] {
            fs::write(
                &plain_case,
                quote::quote! {
                    pub(super) const RESPONSE_CONFORMANCE_CASE_COUNT: usize = 1;
                    #[test]
                    fn plain_response() {
                        assert_eq!(crate::answer(), 7);
                        println!(#revision);
                    }
                }
                .to_string(),
            )
            .unwrap();
            let discovery = scan(&fixture.directory).unwrap();
            assert!(discovery.rerun_paths().contains(&plain_case));
            fs::write(
                generated.join("bidder_response_conformance.rs"),
                discovery.render_response_conformance().unwrap(),
            )
            .unwrap();
            for enabled in [false, true, false] {
                let compiled = compile_conformance(
                    &source,
                    &generated,
                    &binary,
                    "fake_response_probe",
                    enabled,
                );
                assert!(
                    compiled.status.success(),
                    "{}",
                    String::from_utf8_lossy(&compiled.stderr)
                );
                let output = child_output(Command::new(&binary).arg("--nocapture")).unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let stdout = String::from_utf8(output.stdout).unwrap();
                assert!(stdout.contains(revision), "{stdout}");
                assert_eq!(stdout.contains("gated-case"), enabled, "{stdout}");
                assert!(
                    stdout.contains(if enabled { "3 passed" } else { "2 passed" }),
                    "{stdout}"
                );
            }
        }
        fs::remove_file(&gated_case).unwrap();
        let removed = scan(&fixture.directory).unwrap();
        assert!(!removed.rerun_paths().contains(&gated_case));
        assert!(
            !removed
                .render_response_conformance()
                .unwrap()
                .contains("mod fake_beta_probe;")
        );
        fs::write(
            generated.join("bidder_response_conformance.rs"),
            removed.render_response_conformance().unwrap(),
        )
        .unwrap();
        let compiled =
            compile_conformance(&source, &generated, &binary, "fake_response_probe", true);
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = child_output(Command::new(&binary).arg("--nocapture")).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            stdout.contains("2 passed") && stdout.contains("edited-case"),
            "{stdout}"
        );
        assert!(!stdout.contains("gated-case"), "{stdout}");
    }

    #[test]
    fn response_conformance_requires_a_regular_adjacent_file() {
        let fixture = Fixture::new().unwrap();
        let bidder = fixture.local_factory("fake_response_probe").unwrap();
        let source = bidder.join("tests/response_conformance.rs");
        fs::create_dir_all(&source).unwrap();
        assert_eq!(
            scan(&fixture.directory).unwrap_err(),
            format!("{}: expected regular file", source.display()),
        );
    }

    #[test]
    fn response_runner_rejects_empty_or_entirely_disabled_conformance() {
        let fixture = Fixture::new().unwrap();
        let bidder = fixture.bidder("fake_gated_probe", "key = 'fake_gated_probe'\nfactory = true\nrequired_feature = 'experimental'\n[provenance]\norigin = 'local'\n").unwrap();
        let conformance = bidder.join("tests/response_conformance.rs");
        fs::write(&conformance, "pub(super) const RESPONSE_CONFORMANCE_CASE_COUNT: usize = 1; #[test] fn synthetic_response_case() {}\n").unwrap();
        let source = fixture.directory.join("response_runner.rs");
        fs::write(
            &source,
            "include!(concat!(env!(\"OUT_DIR\"), \"/bidder_response_conformance.rs\"));\n",
        )
        .unwrap();
        let generated = fixture.directory.join("output");
        fs::create_dir_all(&generated).unwrap();
        let binary = generated.join(format!("fake_response_probe{EXE_SUFFIX}"));
        for case_count in [Some(1_usize), Some(0), None] {
            if let Some(count) = case_count {
                fs::write(
                    &conformance,
                    quote::quote! {
                        pub(super) const RESPONSE_CONFORMANCE_CASE_COUNT: usize = #count;
                        #[test]
                        fn synthetic_response_case() {}
                    }
                    .to_string(),
                )
                .unwrap();
            } else {
                fs::remove_file(&conformance).unwrap();
            }
            let discovery = scan(&fixture.directory).unwrap();
            fs::write(
                generated.join("bidder_response_conformance.rs"),
                discovery.render_response_conformance().unwrap(),
            )
            .unwrap();
            for enabled in [false, true] {
                let mut rustc = Command::new("rustc");
                rustc
                    .env("OUT_DIR", &generated)
                    .args(["--edition=2024", "--test", "-Dwarnings"])
                    .arg(&source)
                    .arg("-o")
                    .arg(&binary);
                if enabled {
                    rustc.args(["--cfg", "feature=\"experimental\""]);
                }
                let compiled = child_output(&mut rustc).unwrap();
                assert!(
                    compiled.status.success(),
                    "{}",
                    String::from_utf8_lossy(&compiled.stderr)
                );
                let output = child_output(Command::new(&binary).arg("--nocapture")).unwrap();
                let expected_success = case_count.is_some_and(|count| count > 0) && enabled;
                assert_eq!(output.status.success(), expected_success);
                let stdout = String::from_utf8(output.stdout).unwrap();
                let stderr = String::from_utf8(output.stderr).unwrap();
                if expected_success {
                    assert!(stdout.contains("2 passed"), "{stdout}");
                } else {
                    let expected = if case_count == Some(0) && enabled {
                        "response conformance module declared zero cases"
                    } else {
                        "no enabled bidder response conformance was discovered"
                    };
                    assert!(stderr.contains(expected), "{stdout}\n{stderr}");
                }
            }
        }
    }

    #[test]
    fn native_factory_contract_tests_follow_discovery_and_feature_ownership() {
        let fixture = Fixture::new().unwrap();
        fixture.local_factory("fake_alpha_probe").unwrap();
        fixture.bidder("fake_beta_probe", "key = 'fake_beta_probe'\nfactory = true\nrequired_feature = 'experimental'\n[provenance]\norigin = 'local'\n").unwrap();
        let rendered = scan(&fixture.directory).unwrap().render().unwrap();
        let parsed = syn::parse_file(&rendered).unwrap();
        let module = parsed
            .items
            .iter()
            .find_map(|item| {
                if let syn::Item::Mod(module) = item
                    && module.ident == "factory_contract_tests"
                {
                    Some(module)
                } else {
                    None
                }
            })
            .unwrap();
        let cfgs: Vec<_> = module
            .attrs
            .iter()
            .filter_map(|attr| {
                if let syn::Meta::List(list) = &attr.meta
                    && list.path.is_ident("cfg")
                {
                    Some(list.tokens.to_string())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(cfgs, ["test", "not (target_arch = \"wasm32\")"]);
        let functions: Vec<_> = module
            .content
            .as_ref()
            .unwrap()
            .1
            .iter()
            .filter_map(|item| {
                if let syn::Item::Fn(function) = item {
                    Some(function)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            functions.len(),
            2,
            "every factory needs an automatically registered contract test"
        );
        for (function, key, feature) in [
            (functions[0], "fake_alpha_probe", None),
            (functions[1], "fake_beta_probe", Some("experimental")),
        ] {
            assert_eq!(function.sig.ident, key);
            assert!(
                function
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("test"))
            );
            let cfg = function.attrs.iter().find_map(|attr| {
                if let syn::Meta::List(list) = &attr.meta
                    && list.path.is_ident("cfg")
                {
                    Some(list.tokens.to_string())
                } else {
                    None
                }
            });
            assert_eq!(
                cfg,
                feature.map(|name| quote::quote!(feature = #name).to_string())
            );
            let fixture_file = fixture.root.join(key).join("fixtures/factory.json");
            let fixture_path = fixture_file.to_str().unwrap();
            let expected: syn::Block = syn::parse_quote!({
                super::harness::assert_factory_contract(#key, include_str!(#fixture_path));
            });
            assert_eq!(
                function.block.to_token_stream().to_string(),
                expected.to_token_stream().to_string()
            );
        }
    }

    #[test]
    fn scans_explicit_unknown_keys_and_tracks_fixed_inputs() {
        let fixture = Fixture::new().unwrap();
        let bidder = fixture.local_factory("fake_zeta_probe").unwrap();
        let discovery = scan(&fixture.directory).unwrap();
        assert_eq!(
            discovery.rerun_paths(),
            &[
                fixture.directory.join("Cargo.toml"),
                fixture.root,
                bidder.clone(),
                bidder.join("bidder.toml"),
                bidder.join("mod.rs"),
                bidder.join("tests"),
            ]
            .into_iter()
            .collect()
        );
    }

    fn unified_metadata_fixture() -> TestResult<Fixture> {
        let fixture = Fixture::new()?;
        fs::write(
            fixture.directory.join("Cargo.toml"),
            "[features]\nexperimental = ['auctioneer-config-data/experimental']\n",
        )?;
        fixture.bidder(
            "fake_probe",
            "key = 'fake_probe'\nfactory = true\nrequired_feature = 'experimental'\n\
                 [provenance]\norigin = 'local'\n\
                 [definitions.fake_publicProbe]\nendpoint = 'https://probe.example/rtb'\n",
        )?;
        let reference = fixture.root.join("fake_reference_only");
        fs::create_dir_all(reference.join("notes"))?;
        let note = reference.join("notes/source.txt");
        fs::write(&note, "Reference evidence is not a callable factory.\n")?;
        fs::write(
            reference.join("bidder.toml"),
            "key = 'fake_reference_only'\nfactory = false\n\
             [provenance]\norigin = 'reference'\n\
             [provenance.sources.supplemental]\n\
             repository = 'https://example.test/reference.git'\n\
             revision = '1111111111111111111111111111111111111111'\n\
             [[provenance.trees]]\ncategory = 'implementation'\n\
             source = 'supplemental'\nsource_path = 'adapters/fake_reference_probe'\n\
             tree = '2222222222222222222222222222222222222222'\n",
        )?;

        Ok(fixture)
    }

    #[test]
    fn unified_metadata_and_reference_only_packages_use_native_reader_without_promotion() {
        use crate::build_catalog;
        use crate::types::BidderMetadataDeclaration;
        use std::collections::BTreeSet;

        let fixture = unified_metadata_fixture().unwrap();
        let reference = fixture.root.join("fake_reference_only");
        let note = reference.join("notes/source.txt");
        let native =
            build_catalog::scan(&fixture.root, &BTreeSet::from(["experimental".to_owned()]))
                .unwrap();
        assert_eq!(native.manifests.len(), 2);
        let owner = &native.manifests["fake_probe"];
        assert!(owner.factory);
        assert_eq!(owner.required_feature.as_deref(), Some("experimental"));
        assert_eq!(
            owner.definitions["fake_publicProbe"]["endpoint"].as_str(),
            Some("https://probe.example/rtb")
        );
        assert!(!owner.definitions["fake_publicProbe"].contains_key("implementation"));
        let data_only = &native.manifests["fake_reference_only"];
        assert!(!data_only.factory);
        assert!(data_only.definitions.is_empty());
        assert!(matches!(
            data_only.provenance,
            build_catalog::Provenance::Reference { .. }
        ));
        assert!(!reference.join("mod.rs").exists());

        let discovery = scan(&fixture.directory).unwrap();
        let mut expected_paths = native.input_paths;
        expected_paths.insert(fixture.directory.join("Cargo.toml"));
        assert_eq!(discovery.rerun_paths(), &expected_paths);
        assert!(discovery.rerun_paths().contains(&note));
        let metadata = [BidderMetadataDeclaration {
            bidder: "fake_publicProbe",
            implementation: "fake_probe",
            required_feature: Some("experimental"),
        }];
        assert_eq!(discovery.validate_metadata(&metadata), Ok(()));
        assert_eq!(
            discovery.validate_metadata(&[BidderMetadataDeclaration {
                bidder: "fake_unpromoted",
                implementation: "fake_reference_only",
                required_feature: None,
            }]),
            Err("metadata implementation has no factory: fake_unpromoted".into())
        );
        let rendered = discovery.render().unwrap();
        let parsed = syn::parse_file(&rendered).unwrap();
        let modules = external_modules(&parsed)
            .map(|module| module.ident.to_string())
            .collect::<Vec<_>>();
        assert_eq!(modules, ["fake_probe"]);
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(BidderDeclaration {
                key: "fake_probe",
                required_feature: Some("experimental"),
                has_settings: false,
            })
        ));
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(factory: fake_probe::create_bidder)
        ));
        assert_eq!(
            rendered
                .matches(&quote::quote!(#[cfg(feature = "experimental")]).to_string())
                .count(),
            3
        );
        assert!(!rendered.contains("fake_reference_only"));
        assert!(!rendered.contains("fake_publicProbe"));
    }

    #[test]
    fn metadata_validation_uses_the_scanned_manifest_and_all_declarations() {
        use crate::types::BidderMetadataDeclaration;

        let fixture = Fixture::new().unwrap();
        fixture
            .bidder(
                "fake_probe",
                "key = 'fake_probe'\nfactory = true\nrequired_feature = 'experimental'\n\
                 [provenance]\norigin = 'local'\n",
            )
            .unwrap();
        let metadata = [BidderMetadataDeclaration {
            bidder: "fake_public_probe",
            implementation: "fake_probe",
            required_feature: Some("experimental"),
        }];
        assert_eq!(
            scan(&fixture.directory)
                .unwrap()
                .validate_metadata(&metadata),
            Err("missing config feature forwarding: experimental".into())
        );
        fs::write(
            fixture.directory.join("Cargo.toml"),
            "[features]\nexperimental = ['auctioneer-config-data/experimental']\n",
        )
        .unwrap();
        assert_eq!(
            scan(&fixture.directory)
                .unwrap()
                .validate_metadata(&metadata),
            Ok(())
        );
    }

    #[test]
    fn declaration_errors_have_fixed_native_reader_diagnostics() {
        for declaration in [
            "key = [",
            "factory = true\n[provenance]\norigin = 'local'",
            "key = 17\nfactory = true\n[provenance]\norigin = 'local'",
            "key = 'fake_probe'\n[provenance]\norigin = 'local'",
            "key = 'fake_probe'\nfactory = 'true'\n[provenance]\norigin = 'local'",
            "key = 'fake_probe'\nfactory = true",
            "key = 'fake_probe'\nfactory = true\nrequired_feature = true\n[provenance]\norigin = 'local'",
            "key = 'fake_probe'\nfactory = true\nctor = 'run'\n[provenance]\norigin = 'local'",
            "key = 'fake_probe'\nfactory = true\npath = '../other.rs'\n[provenance]\norigin = 'local'",
            "key = 'fake_probe'\nkey = 'fake_probe'\nfactory = true\n[provenance]\norigin = 'local'",
        ] {
            let fixture = Fixture::new().unwrap();
            let bidder = fixture.bidder("fake_probe", declaration).unwrap();
            assert_eq!(
                scan(&fixture.directory).unwrap_err(),
                format!(
                    "{}: invalid bidder manifest",
                    bidder.join("bidder.toml").display()
                )
            );
        }
    }

    #[test]
    fn refuses_invalid_identifiers_keywords_and_mismatched_directory() {
        for key in [
            "",
            "_",
            "Probe",
            "1probe",
            "bad-key",
            "r#type",
            "../probe",
            "type",
            "async",
            "gen",
            "self",
            "true",
            "try",
            "yield",
            "\u{fc}ber",
        ] {
            let fixture = Fixture::new().unwrap();
            fixture
                .bidder(
                    "fake_probe",
                    &format!("key = '{key}'\nfactory = true\n[provenance]\norigin = 'local'"),
                )
                .unwrap();
            assert_eq!(
                scan(&fixture.directory).unwrap_err(),
                "bidder fake_probe: invalid bidder key"
            );
        }
        let fixture = Fixture::new().unwrap();
        fixture
            .bidder(
                "fake_different",
                "key = 'fake_probe'\nfactory = true\n[provenance]\norigin = 'local'",
            )
            .unwrap();
        assert_eq!(
            scan(&fixture.directory).unwrap_err(),
            "bidder fake_different: bidder directory does not match key"
        );
    }

    #[test]
    fn refuses_duplicate_public_names_before_feature_filtering() {
        let fixture = Fixture::new().unwrap();
        fixture
            .bidder(
                "fake_first",
                "key = 'fake_first'\nfactory = true\n[provenance]\norigin = 'local'\n\
                 [definitions.fake_PublicProbe]\nendpoint = 'https://probe.example/rtb'",
            )
            .unwrap();
        fixture
            .bidder(
                "fake_second",
                "key = 'fake_second'\nfactory = true\nrequired_feature = 'experimental'\n\
                 [provenance]\norigin = 'local'\n\
                 [definitions.fake_publicprobe]\nendpoint = 'https://probe.example/rtb'",
            )
            .unwrap();
        assert_eq!(
            scan(&fixture.directory).unwrap_err(),
            "duplicate public bidder identity: fake_first and fake_second"
        );
    }

    #[test]
    fn validates_feature_off_declarations_and_manifest_feature_types() {
        for feature in ["absent", ""] {
            let fixture = Fixture::new().unwrap();
            fixture
                .bidder(
                    "fake_probe",
                    &format!(
                        "key = 'fake_probe'\nfactory = true\nrequired_feature = '{feature}'\n\
                         [provenance]\norigin = 'local'"
                    ),
                )
                .unwrap();
            assert_eq!(
                scan(&fixture.directory).unwrap_err(),
                "bidder fake_probe: required_feature must name an allowed declared feature"
            );
        }
        let malformed = Fixture::new().unwrap();
        let bidder = malformed
            .bidder(
                "fake_probe",
                "key = 'fake_probe'\nfactory = true\nrequired_feature = 'experimental'\n\
                 constructor = 'anything'\n[provenance]\norigin = 'local'",
            )
            .unwrap();
        assert_eq!(
            scan(&malformed.directory).unwrap_err(),
            format!(
                "{}: invalid bidder manifest",
                bidder.join("bidder.toml").display()
            )
        );
        for text in [
            "[features]\nexperimental = true",
            "features = []",
            "[features]\nexperimental = [1]",
            "[",
        ] {
            let fixture = Fixture::new().unwrap();
            fs::write(fixture.directory.join("Cargo.toml"), text).unwrap();
            assert!(
                scan(&fixture.directory)
                    .unwrap_err()
                    .starts_with("invalid Cargo manifest ")
            );
        }
    }

    #[test]
    fn every_immediate_directory_requires_both_files_including_adapterstest() {
        for name in ["fake_probe", "adapterstest"] {
            for missing in ["bidder.toml", "mod.rs"] {
                let fixture = Fixture::new().unwrap();
                let bidder = fixture.local_factory(name).unwrap();
                fs::remove_file(bidder.join(missing)).unwrap();
                assert_eq!(
                    scan(&fixture.directory).unwrap_err(),
                    format!("{}: missing input", bidder.join(missing).display())
                );
            }
        }
        let fixture = Fixture::new().unwrap();
        fixture.local_factory("adapterstest").unwrap();
        scan(&fixture.directory).unwrap();
    }

    #[test]
    fn ordinary_helpers_remain_inputs_and_cannot_be_shadowed() {
        for name in ["url", "media_type", "unknown_helper"] {
            let fixture = Fixture::new().unwrap();
            let helper = fixture.root.join(format!("{name}.rs"));
            fs::write(&helper, "").unwrap();
            assert!(
                scan(&fixture.directory)
                    .unwrap()
                    .rerun_paths()
                    .contains(&helper)
            );
            fixture.local_factory(name).unwrap();
            assert_eq!(
                scan(&fixture.directory).unwrap_err(),
                format!("bidder {name}: bidder key collides with helper module")
            );
        }
        let fixture = Fixture::new().unwrap();
        fixture.local_factory("harness").unwrap();
        assert_eq!(
            scan(&fixture.directory).unwrap_err(),
            "bidder harness: reserved bidder key"
        );
    }

    #[test]
    fn refuses_reserved_unit_test_module() {
        let fixture = Fixture::new().unwrap();
        fixture.local_factory("tests").unwrap();
        assert_eq!(
            scan(&fixture.directory).unwrap_err(),
            "bidder tests: reserved bidder key"
        );
    }

    #[test]
    fn tracks_nested_inputs_for_addition_edit_and_deletion() {
        let fixture = Fixture::new().unwrap();
        let bidder = fixture.local_factory("fake_probe").unwrap();
        let nested = bidder.join("nested/deeper");
        fs::create_dir_all(&nested).unwrap();
        let source = nested.join("helper.rs");
        fs::write(&source, [0xff_u8]).unwrap();
        let first = scan(&fixture.directory).unwrap();
        for path in [&bidder.join("nested"), &nested, &source] {
            assert!(first.rerun_paths().contains(path), "{}", path.display());
        }
        fs::write(&source, "edited").unwrap();
        assert_eq!(
            first.rerun_paths(),
            scan(&fixture.directory).unwrap().rerun_paths()
        );
        fs::remove_file(&source).unwrap();
        assert!(
            !scan(&fixture.directory)
                .unwrap()
                .rerun_paths()
                .contains(&source)
        );
        fs::remove_dir_all(bidder).unwrap();
        let empty = scan(&fixture.directory).unwrap();
        assert_eq!(empty.rerun_paths().len(), 2);
        fixture.local_factory("fake_added").unwrap();
        assert_eq!(scan(&fixture.directory).unwrap().rerun_paths().len(), 6);
    }

    #[test]
    fn refuses_missing_fixed_inputs_and_nonregular_pair_files() {
        for input in ["src/bidders", "Cargo.toml"] {
            let fixture = Fixture::new().unwrap();
            let path = fixture.directory.join(input);
            if path.is_dir() {
                fs::remove_dir(&path).unwrap();
            } else {
                fs::remove_file(&path).unwrap();
            }
            assert_eq!(
                scan(&fixture.directory).unwrap_err(),
                format!("{}: missing input", path.display())
            );
        }
        for input in [
            "Cargo.toml",
            "src/bidders/fake_probe/bidder.toml",
            "src/bidders/fake_probe/mod.rs",
        ] {
            let fixture = Fixture::new().unwrap();
            fixture.local_factory("fake_probe").unwrap();
            let path = fixture.directory.join(input);
            fs::remove_file(&path).unwrap();
            fs::create_dir_all(&path).unwrap();
            assert_eq!(
                scan(&fixture.directory).unwrap_err(),
                format!("{}: expected regular file", path.display())
            );
        }
        let fixture = Fixture::new().unwrap();
        fs::remove_dir(&fixture.root).unwrap();
        fs::write(&fixture.root, "not a directory").unwrap();
        assert_eq!(
            scan(&fixture.directory).unwrap_err(),
            format!("{}: expected directory", fixture.root.display())
        );
    }

    #[test]
    fn refuses_escaping_and_relative_scan_paths() {
        let fixture = Fixture::new().unwrap();
        for path in [
            fixture.directory.join("../escape"),
            PathBuf::from("relative"),
        ] {
            assert_eq!(
                scan(&path).unwrap_err(),
                format!("non-normal absolute crate path: {}", path.display())
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinks_inside_outside_and_dangling_at_every_input_level() {
        use std::os::unix::fs::symlink;

        for outside in [false, true] {
            for input in [
                "src",
                "src/bidders",
                "Cargo.toml",
                "src/bidders/fake_probe",
                "src/bidders/fake_probe/bidder.toml",
                "src/bidders/fake_probe/mod.rs",
                "src/bidders/fake_probe/nested",
                "src/bidders/fake_probe/nested/helper.rs",
                "src/bidders/url.rs",
            ] {
                let fixture = Fixture::new().unwrap();
                let bidder = fixture.local_factory("fake_probe").unwrap();
                fs::create_dir_all(bidder.join("nested")).unwrap();
                fs::write(bidder.join("nested/helper.rs"), "").unwrap();
                fs::write(fixture.root.join("url.rs"), "").unwrap();
                let target = tempfile::tempdir().unwrap();
                let saved = if outside {
                    target.path().canonicalize().unwrap().join("saved")
                } else {
                    fixture.directory.join("saved")
                };
                let path = fixture.directory.join(input);
                fs::rename(&path, &saved).unwrap();
                symlink(&saved, &path).unwrap();
                assert_eq!(
                    scan(&fixture.directory).unwrap_err(),
                    format!("{}: symlink input", path.display())
                );
            }
        }
        let fixture = Fixture::new().unwrap();
        let path = fixture.root.join("dangling.rs");
        symlink(fixture.directory.join("absent"), &path).unwrap();
        assert_eq!(
            scan(&fixture.directory).unwrap_err(),
            format!("{}: symlink input", path.display())
        );
    }

    #[cfg(unix)]
    #[test]
    fn refuses_nonregular_root_and_nested_inputs() {
        use std::os::unix::net::UnixListener;

        for nested in [false, true] {
            let fixture = Fixture::new().unwrap();
            let parent = if nested {
                fixture.local_factory("fake_probe").unwrap()
            } else {
                fixture.root.clone()
            };
            // Socket paths have a small OS limit; a short name fits the fixture root.
            let path = parent.join("s");
            let _socket = UnixListener::bind(&path).unwrap();
            assert_eq!(
                scan(&fixture.directory).unwrap_err(),
                format!("{}: nonregular input", path.display())
            );
        }
    }

    #[test]
    fn render_uses_canonical_paths_direct_modules_and_sorted_declaration_data() {
        let fixture = Fixture::new().unwrap();
        fixture.local_factory("fake_zeta_probe").unwrap();
        let gated = fixture
            .bidder(
                "fake_adapterstest",
                "key = 'fake_adapterstest'\nfactory = true\nrequired_feature = 'experimental'\n\
                 [provenance]\norigin = 'local'",
            )
            .unwrap();
        fixture.local_factory("fake_a_signal").unwrap();
        fs::write(fixture.root.join("url.rs"), "").unwrap();
        let discovery = scan(&fixture.directory).unwrap();
        let rendered = discovery.render().unwrap();
        assert_eq!(
            rendered,
            scan(&fixture.directory).unwrap().render().unwrap()
        );
        let parsed = syn::parse_file(&rendered).unwrap();
        let modules: Vec<_> = external_modules(&parsed).collect();
        assert_eq!(
            modules
                .iter()
                .map(|module| module.ident.to_string())
                .collect::<Vec<_>>(),
            ["fake_a_signal", "fake_adapterstest", "fake_zeta_probe"]
        );
        assert!(modules.iter().all(|module| module.content.is_none()));
        let source = gated.join("mod.rs");
        let path = source.to_str().unwrap();
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(
                #[cfg(feature = "experimental")]
                #[path = #path]
                mod fake_adapterstest;
            )
        ));
        assert_eq!(
            rendered
                .matches(&quote::quote!(#[cfg(feature = "experimental")]).to_string())
                .count(),
            3
        );
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(BidderDeclaration {
                key: "fake_adapterstest",
                required_feature: Some("experimental"),
                has_settings: false,
            })
        ));
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(factory: fake_adapterstest::create_bidder)
        ));
        assert!(!contains_tokens(
            &rendered,
            &quote::quote!(
                mod url;
            )
        ));
    }

    #[test]
    fn empty_catalog_renders_valid_empty_tables() {
        let fixture = Fixture::new().unwrap();
        let rendered = scan(&fixture.directory).unwrap().render().unwrap();
        syn::parse_file(&rendered).unwrap();
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(
                static ALL_DECLARATIONS: &[BidderDeclaration] = &[];
            )
        ));
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(
                static COMPILED_BIDDERS: &[BidderRegistration] = &[];
            )
        ));
    }

    #[test]
    fn serving_dispatch_uses_only_validated_public_bindings_and_retains_feature_ownership() {
        use crate::types::BidderMetadataDeclaration;
        let fixture = Fixture::new().unwrap();
        fixture.local_factory("fake_shared_factory").unwrap();
        fixture.local_factory("fake_unused_candidate").unwrap();
        fixture.bidder("fake_gated_factory", "key = 'fake_gated_factory'\nfactory = true\nrequired_feature = 'experimental'\n[provenance]\norigin = 'local'\n").unwrap();
        fs::write(
            fixture.directory.join("Cargo.toml"),
            "[features]\nexperimental = ['auctioneer-config-data/experimental']\n",
        )
        .unwrap();
        let discovery = scan(&fixture.directory).unwrap();
        let rows = [
            BidderMetadataDeclaration {
                bidder: "fake_PublicBeta",
                implementation: "fake_shared_factory",
                required_feature: None,
            },
            BidderMetadataDeclaration {
                bidder: "fake_PublicAlpha",
                implementation: "fake_shared_factory",
                required_feature: None,
            },
            BidderMetadataDeclaration {
                bidder: "fake_OptionalPublic",
                implementation: "fake_gated_factory",
                required_feature: Some("experimental"),
            },
        ];
        let rendered = discovery.render_registry(&rows).unwrap();
        syn::parse_file(&rendered).unwrap();
        assert!(!rendered.contains("fake_unused_candidate"));
        assert!(!rendered.contains("compiled_bidders"));
        assert!(!rendered.contains("ValidatedRegistryPlan"));
        assert!(!rendered.contains("COMPILED_BIDDERS"));
        assert_eq!(
            rendered
                .matches(&quote::quote!(fake_shared_factory::create_bidder).to_string())
                .count(),
            1
        );
        assert!(contains_tokens(
            &rendered,
            &quote::quote!("fake_PublicAlpha" | "fake_PublicBeta" =>)
        ));
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(#[cfg(feature = "experimental")] "fake_OptionalPublic" =>)
        ));
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(fake_gated_factory::create_bidder)
        ));
        let mut reordered = rows;
        reordered.reverse();
        assert_eq!(rendered, discovery.render_registry(&reordered).unwrap());
        assert_eq!(
            discovery.render_registry(&[BidderMetadataDeclaration {
                implementation: "fake_missing",
                ..rows[0]
            }]),
            Err("metadata implementation has no factory: fake_PublicBeta".into())
        );
        assert_eq!(
            discovery.render_registry(&[BidderMetadataDeclaration {
                required_feature: Some("experimental"),
                ..rows[0]
            }]),
            Err("metadata/factory feature mismatch: fake_PublicBeta".into())
        );
    }

    // Local to bounded discovery fixtures; no Cargo child or qualification harness.
    fn child_output(command: &mut Command) -> io::Result<Output> {
        const OUTPUT_LIMIT: u64 = 64 * 1024;
        let mut stdout = tempfile::tempfile()?;
        let mut stderr = tempfile::tempfile()?;
        command
            .stdin(Stdio::null())
            .stdout(stdout.try_clone()?)
            .stderr(stderr.try_clone()?);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.process_group(0)
        };
        let mut child = command.spawn()?;
        let started = Instant::now();
        let result = (|| {
            loop {
                if stdout.metadata()?.len() > OUTPUT_LIMIT
                    || stderr.metadata()?.len() > OUTPUT_LIMIT
                {
                    return Err(io::Error::other("child output limit"));
                }
                if let Some(status) = child.try_wait()? {
                    return Ok(status);
                }
                if started.elapsed() >= Duration::from_secs(30) {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "discovery fixture deadline",
                    ));
                }
                thread::sleep(Duration::from_millis(10));
            }
        })();
        #[cfg(unix)]
        {
            use rustix::process::{Pid, Signal, kill_process_group};
            let _termination = kill_process_group(Pid::from_child(&child), Signal::KILL);
        }
        let _termination = child.kill();
        let _reaping = child.wait();
        let status = result?;
        stdout.rewind()?;
        stderr.rewind()?;
        let mut output = Output {
            status,
            stdout: Vec::new(),
            stderr: Vec::new(),
        };
        stdout.take(OUTPUT_LIMIT).read_to_end(&mut output.stdout)?;
        stderr.take(OUTPUT_LIMIT).read_to_end(&mut output.stderr)?;
        Ok(output)
    }

    #[cfg(unix)]
    #[test]
    fn source_tree_formatter_checks_unknown_bidder_and_nested_helper() {
        let temporary = tempfile::tempdir().unwrap();
        let workspace = temporary.path().canonicalize().unwrap();
        let crate_dir = workspace.join("crates/auctioneer-bidder");
        let root = crate_dir.join("src/bidders");
        let bidder = root.join("fake_zeta_format_probe");
        fs::create_dir_all(bidder.join("nested")).unwrap();
        fs::write(crate_dir.join("Cargo.toml"), "[features]\n").unwrap();
        fs::write(
            bidder.join("bidder.toml"),
            "key = 'fake_zeta_format_probe'\nfactory = true\n[provenance]\norigin = 'local'\n",
        )
        .unwrap();
        for (relative, source) in [
            (
                "mod.rs",
                "include!(concat!(env!(\"OUT_DIR\"), \"/discovery.rs\"));\n",
            ),
            (
                "fake_zeta_format_probe/mod.rs",
                "mod nested;\n\npub(crate) fn create_bidder() -> Box<dyn crate::Bidder> {\n    Box::new(crate::Answer(nested::number()))\n}\n",
            ),
            (
                "fake_zeta_format_probe/nested/mod.rs",
                "mod helper;\n\npub(super) fn number() -> u32 {\n    helper::number()\n}\n",
            ),
            (
                "fake_zeta_format_probe/nested/helper.rs",
                "pub(super) fn number() -> u32 {\n    7\n}\n",
            ),
        ] {
            fs::write(root.join(relative), source).unwrap();
        }
        scan(&crate_dir).unwrap();

        let format_sources = || {
            child_output(Command::new("find").current_dir(&workspace).args([
                "crates/auctioneer-bidder/src/bidders",
                "-type",
                "f",
                "-name",
                "*.rs",
                "-exec",
                "rustfmt",
                "--edition",
                "2024",
                "--check",
                "{}",
                "+",
            ]))
            .unwrap()
        };
        let control = format_sources();
        assert!(
            control.status.success(),
            "formatted control failed: {}{}",
            String::from_utf8_lossy(&control.stdout),
            String::from_utf8_lossy(&control.stderr)
        );
        for (relative, unformatted) in [
            (
                "fake_zeta_format_probe/mod.rs",
                "mod nested; pub(crate) fn create_bidder()->Box<dyn crate::Bidder>{Box::new(crate::Answer(nested::number()))}\n",
            ),
            (
                "fake_zeta_format_probe/nested/helper.rs",
                "pub(super) fn number( )->u32{7}\n",
            ),
        ] {
            let path = root.join(relative);
            let formatted = fs::read_to_string(&path).unwrap();
            fs::write(&path, unformatted).unwrap();
            let output = format_sources();
            let diagnostics = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(!output.status.success(), "formatter missed {relative}");
            assert!(diagnostics.contains(relative), "{diagnostics}");
            assert_eq!(fs::read_to_string(&path).unwrap(), unformatted);
            fs::write(&path, formatted).unwrap();
            let restored = format_sources();
            assert!(
                restored.status.success(),
                "restored control failed: {}{}",
                String::from_utf8_lossy(&restored.stdout),
                String::from_utf8_lossy(&restored.stderr)
            );
        }
    }

    fn compiler_bidder_sources(fixture: &Fixture) -> TestResult {
        let marker = proc_macro2::Literal::string(
            fixture
                .directory
                .join("constructed")
                .to_str()
                .ok_or("UTF-8 fixture")?,
        );
        for (key, value, counter, feature) in [
            ("fake_zeta_probe", 33_u32, "Z", ""),
            (
                "fake_adapterstest",
                22_u32,
                "G",
                "\nrequired_feature = 'experimental'",
            ),
            ("fake_a_signal", 11_u32, "A", ""),
        ] {
            let bidder = fixture.bidder(
                key,
                &format!("key = '{key}'\nfactory = true{feature}\n[provenance]\norigin = 'local'"),
            )?;
            let gate_check = (!feature.is_empty()).then(|| {
                quote::quote! {
                    #[cfg(not(feature = "experimental"))]
                    compile_error!("gated module was loaded");
                }
            });
            let counter_ident = syn::parse_str::<syn::Ident>(counter)?;
            fs::write(
                bidder.join("mod.rs"),
                quote::quote! {
                    #gate_check
                    mod nested;
                    pub(crate) fn create_bidder() -> Box<dyn crate::Bidder> {
                        crate::#counter_ident.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        std::fs::write(#marker, b"called").unwrap();
                        crate::bidders::harness::check();
                        Box::new(crate::Answer(super::url::number() + super::media_type::number() + nested::number()))
                    }
                }.to_string(),
            )?;
            fs::create_dir_all(bidder.join("nested"))?;
            fs::write(
                bidder.join("nested/mod.rs"),
                "mod leaf; pub(super) fn number() -> u32 { leaf::number() }",
            )?;
            fs::write(
                bidder.join("nested/leaf.rs"),
                quote::quote!(pub(super) fn number() -> u32 { #value }).to_string(),
            )?;
        }
        Ok(())
    }

    #[test]
    fn serving_dispatch_executes_shared_factories_without_activating_candidates() {
        use crate::types::BidderMetadataDeclaration;
        let fixture = Fixture::new().unwrap();
        fixture.local_factory("fake_shared_factory").unwrap();
        fixture.local_factory("fake_unused_candidate").unwrap();
        fixture.bidder("fake_gated_factory", "key = 'fake_gated_factory'\nfactory = true\nrequired_feature = 'experimental'\n[provenance]\norigin = 'local'\n").unwrap();
        fs::write(
            fixture.directory.join("Cargo.toml"),
            "[features]\nexperimental = ['auctioneer-config-data/experimental']\n",
        )
        .unwrap();
        let rows = [
            BidderMetadataDeclaration {
                bidder: "fake_PublicAlpha",
                implementation: "fake_shared_factory",
                required_feature: None,
            },
            BidderMetadataDeclaration {
                bidder: "fake_PublicBeta",
                implementation: "fake_shared_factory",
                required_feature: None,
            },
            BidderMetadataDeclaration {
                bidder: "fake_OptionalPublic",
                implementation: "fake_gated_factory",
                required_feature: Some("experimental"),
            },
        ];
        let source = fixture.directory.join("main.rs");
        let binary = fixture
            .directory
            .join(format!("registry_probe{EXE_SUFFIX}"));
        for mode in ["all", "empty", "gated_only"] {
            let metadata = match mode {
                "empty" => &rows[..0],
                "gated_only" => &rows[2..],
                _ => &rows[..],
            };
            let generated = scan(&fixture.directory)
                .unwrap()
                .render_registry(metadata)
                .unwrap();
            fs::write(&source, serving_probe_source(&generated, metadata.len())).unwrap();
            if mode == "all" {
                for mismatch in ["extra_feature", "wrong_public", "wrong_implementation"] {
                    let compilation = child_output(
                        Command::new("rustc")
                            .args(["--edition=2024", "-Dwarnings", "--cfg", mismatch])
                            .arg(&source)
                            .arg("-o")
                            .arg(&binary),
                    )
                    .unwrap();
                    assert!(
                        !compilation.status.success(),
                        "independent config/bidder mismatch accepted: {mismatch}"
                    );
                    assert!(
                        String::from_utf8_lossy(&compilation.stderr)
                            .contains("compiled bidder bindings differ from config"),
                        "{}",
                        String::from_utf8_lossy(&compilation.stderr)
                    );
                }
            }
            for enabled in [false, true, false] {
                let mut compiler = Command::new("rustc");
                compiler
                    .args(["--edition=2024", "-Dwarnings"])
                    .arg(&source)
                    .arg("-o")
                    .arg(&binary);
                if mode != "all" {
                    compiler.args(["--cfg", mode]);
                }
                if enabled {
                    compiler.args(["--cfg", "feature=\"experimental\""]);
                }
                let compilation = child_output(&mut compiler).unwrap();
                assert!(
                    compilation.status.success(),
                    "mode={mode} feature={enabled}: {}",
                    String::from_utf8_lossy(&compilation.stderr)
                );
                let run = child_output(&mut Command::new(&binary)).unwrap();
                assert!(
                    run.status.success(),
                    "{}",
                    String::from_utf8_lossy(&run.stderr)
                );
                assert_eq!(run.stdout, b"published-only\n");
            }
        }
    }

    #[test]
    fn native_publication_gates_dispatch_but_not_candidate_factories() {
        use crate::codegen::publication_gate;
        use crate::types::BidderMetadataDeclaration;

        let fixture = Fixture::new().unwrap();
        fixture.bidder("fake_native", "key = 'fake_native'\nfactory = true\nnative_only = true\nrequired_feature = 'experimental'\n[definitions.fake_public]\n[provenance]\norigin = 'local'\n").unwrap();
        fs::write(
            fixture.directory.join("Cargo.toml"),
            "[features]\nexperimental = ['auctioneer-config-data/experimental']\n",
        )
        .unwrap();
        let discovery = scan(&fixture.directory).unwrap();
        let registry = discovery
            .render_registry(&[BidderMetadataDeclaration {
                bidder: "fake_public",
                implementation: "fake_native",
                required_feature: Some("experimental"),
            }])
            .unwrap();
        let gate = publication_gate(true, Some("experimental")).to_string();
        assert_eq!(registry.matches(&gate).count(), 2);
        let syntax = syn::parse_file(&registry).unwrap();
        let target_function = syntax
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Fn(function) = item else {
                    return None;
                };
                (function.sig.ident == "target_metadata_declarations").then_some(function)
            })
            .expect("compiled validation needs an explicit target declaration view");
        let target_tokens = quote::quote!(#target_function).to_string();
        assert!(target_tokens.contains(&publication_gate(true, None).to_string()));
        assert!(!target_tokens.contains(&quote::quote!(cfg(feature = "experimental")).to_string()));
        assert!(
            target_tokens.contains("experimental"),
            "off-feature declarations keep their feature ownership"
        );
        let factories = syn::parse_file(&discovery.render().unwrap()).unwrap();
        let registrations = factories
            .items
            .iter()
            .find_map(|item| {
                let syn::Item::Static(registration) = item else {
                    return None;
                };
                (registration.ident == "COMPILED_BIDDERS").then_some(registration)
            })
            .expect("compiled candidate factories");
        let registration_tokens = quote::quote!(#registrations).to_string();
        assert!(!registration_tokens.contains("target_arch"));
        assert!(registration_tokens.contains("experimental"));
    }

    fn serving_probe_types() -> proc_macro2::TokenStream {
        quote::quote! {
            pub trait Bidder { fn marker(&self) -> u8; }
            pub struct Marker(u8);
            impl Bidder for Marker { fn marker(&self) -> u8 { self.0 } }
            pub struct Name(&'static str);
            impl Name { fn as_str(&self) -> &str { self.0 } }
            pub struct Info { name: Name }
            pub struct Binding { pub bidder: &'static str, pub implementation: &'static str }
            pub mod types {
                pub struct BidderMetadataDeclaration {
                    pub bidder: &'static str,
                    pub implementation: &'static str,
                    pub required_feature: Option<&'static str>,
                }
            }
        }
    }

    fn serving_probe_source(generated: &str, expected_declarations: usize) -> String {
        let generated_syntax = syn::parse_file(generated).unwrap();
        let types = serving_probe_types();
        quote::quote! {
            extern crate self as auctioneer_config_data;
            use std::sync::atomic::{AtomicUsize, Ordering};
            static SHARED: AtomicUsize = AtomicUsize::new(0);
            static GATED: AtomicUsize = AtomicUsize::new(0);
            #types
            pub const fn bidder_implementation_bindings() -> &'static [Binding] {
                static BINDINGS: &[Binding] = &[
                    #[cfg(all(not(empty), feature = "experimental"))]
                    Binding { bidder: "fake_OptionalPublic", implementation: "fake_gated_factory" },
                    #[cfg(not(any(empty, gated_only)))]
                    Binding { bidder: if cfg!(wrong_public) { "fake_DifferentPublic" } else { "fake_PublicAlpha" }, implementation: "fake_shared_factory" },
                    #[cfg(not(any(empty, gated_only)))]
                    Binding { bidder: "fake_PublicBeta", implementation: if cfg!(wrong_implementation) { "fake_different_factory" } else { "fake_shared_factory" } },
                    #[cfg(extra_feature)] Binding { bidder: "fake_ZZZUnmatched", implementation: "fake_shared_factory" }
                ];
                BINDINGS
            }
            pub fn bidders() -> &'static [Info] {
                #[cfg(empty)] return &[];
                #[cfg(gated_only)] return &[
                    #[cfg(feature = "experimental")] Info { name: Name("fake_OptionalPublic") }
                ];
                #[cfg(not(any(empty, gated_only)))]
                &[Info { name: Name("fake_PublicBeta") },
                  #[cfg(not(missing_info))] Info { name: Name("fake_PublicAlpha") },
                  #[cfg(duplicate_info)] Info { name: Name("fake_PublicBeta") },
                  #[cfg(unknown_info)] Info { name: Name("fake_UnknownPublic") },
                  #[cfg(feature = "experimental")] Info { name: Name("fake_OptionalPublic") }]
            }
            #[derive(Default)] pub struct BidderRegistry(Vec<(&'static Info, &'static str, Box<dyn Bidder>)>);
            #[derive(Debug, PartialEq)] pub enum RegistryBuildError {
                DuplicateBidder { bidder: &'static str },
                UndeclaredBidder { bidder: &'static str },
                MissingBinding { bidder: &'static str },
                DanglingBinding { bidder: &'static str },
            }
            impl BidderRegistry {
                fn new() -> Self { Self::default() }
                fn register_with_implementation(&mut self, info: &'static Info, implementation: &'static str, bidder: Box<dyn Bidder>) -> Result<(), RegistryBuildError> {
                    self.0.push((info, implementation, bidder));
                    Ok(())
                }
            }
            pub mod bidders {
                pub mod fake_shared_factory {
                    pub fn create_bidder() -> Box<dyn crate::Bidder> {
                        crate::SHARED.fetch_add(1, crate::Ordering::SeqCst);
                        Box::new(crate::Marker(17))
                    }
                }
                #[cfg(feature = "experimental")] pub mod fake_gated_factory {
                    pub fn create_bidder() -> Box<dyn crate::Bidder> {
                        crate::GATED.fetch_add(1, crate::Ordering::SeqCst);
                        Box::new(crate::Marker(29))
                    }
                }
                // No fake_unused_candidate symbol: referencing the inventory fails compilation.
                #generated_syntax
            }
            fn main() {
                let declarations = bidders::target_metadata_declarations();
                assert_eq!(declarations.len(), #expected_declarations);
                for row in declarations {
                    let gated = row.bidder == "fake_OptionalPublic";
                    assert_eq!(row.implementation, if gated {"fake_gated_factory"} else {"fake_shared_factory"});
                    assert_eq!(row.required_feature, if gated {Some("experimental")} else {None});
                }
                assert_eq!([SHARED.load(Ordering::SeqCst), GATED.load(Ordering::SeqCst)], [0, 0]);
                let registry = bidders::configured_registry();
                if cfg!(any(duplicate_info, unknown_info, missing_info)) {
                    assert_eq!([SHARED.load(Ordering::SeqCst), GATED.load(Ordering::SeqCst)], [0, 0]);
                    let expected = if cfg!(duplicate_info) {
                        RegistryBuildError::DuplicateBidder { bidder: "fake_PublicBeta" }
                    } else if cfg!(unknown_info) {
                        RegistryBuildError::UndeclaredBidder { bidder: "fake_UnknownPublic" }
                    } else {
                        RegistryBuildError::DanglingBinding { bidder: "fake_PublicAlpha" }
                    };
                    assert_eq!(registry.err(), Some(expected));
                    println!("refused");
                    return;
                }
                let registry = registry.unwrap();
                let enabled = usize::from(cfg!(feature = "experimental") && !cfg!(empty));
                let shared = if cfg!(any(empty, gated_only)) { 0 } else { 2 };
                assert_eq!([SHARED.load(Ordering::SeqCst), GATED.load(Ordering::SeqCst)], [shared, enabled]);
                assert_eq!(registry.0.len(), shared + enabled);
                for ((original, implementation, bidder), expected) in registry.0.iter().zip(bidders()) {
                    assert!(std::ptr::eq(*original, expected));
                    assert_eq!(*implementation, if expected.name.as_str() == "fake_OptionalPublic" { "fake_gated_factory" } else { "fake_shared_factory" });
                    assert_eq!(bidder.marker(), if expected.name.as_str() == "fake_OptionalPublic" { 29 } else { 17 });
                }
                println!("published-only");
            }
        }.to_string()
    }

    #[test]
    fn serving_dispatch_refuses_invalid_metadata_before_construction() {
        use crate::types::BidderMetadataDeclaration;
        let fixture = Fixture::new().unwrap();
        fixture.local_factory("fake_shared_factory").unwrap();
        let generated = scan(&fixture.directory)
            .unwrap()
            .render_registry(&[
                BidderMetadataDeclaration {
                    bidder: "fake_PublicAlpha",
                    implementation: "fake_shared_factory",
                    required_feature: None,
                },
                BidderMetadataDeclaration {
                    bidder: "fake_PublicBeta",
                    implementation: "fake_shared_factory",
                    required_feature: None,
                },
            ])
            .unwrap();
        let source = fixture.directory.join("main.rs");
        let binary = fixture.directory.join(format!("refusal_probe{EXE_SUFFIX}"));
        fs::write(&source, serving_probe_source(&generated, 2)).unwrap();
        for mode in ["duplicate_info", "unknown_info", "missing_info"] {
            let compilation = child_output(
                Command::new("rustc")
                    .args(["--edition=2024", "-Dwarnings", "--cfg", mode])
                    .arg(&source)
                    .arg("-o")
                    .arg(&binary),
            )
            .unwrap();
            assert!(
                compilation.status.success(),
                "{}",
                String::from_utf8_lossy(&compilation.stderr)
            );
            let run = child_output(&mut Command::new(&binary)).unwrap();
            assert!(
                run.status.success(),
                "{mode}: {}",
                String::from_utf8_lossy(&run.stderr)
            );
            assert_eq!(run.stdout, b"refused\n");
        }
    }

    #[test]
    fn config_callback_requirement_comes_only_from_executable_settings_ownership() {
        let fixture = Fixture::new().unwrap();
        fixture.local_factory("fake_plain").unwrap();
        fixture.bidder("fake_settings", "key = 'fake_settings'\nfactory = true\nnative_only = true\nrequired_feature = 'experimental'\n[settings.fields]\ntoken = 'optional_string'\n[provenance]\norigin = 'local'\n").unwrap();
        let reference = fixture.bidder("fake_reference", "key = 'fake_reference'\nfactory = false\n[settings.fields]\ntoken = 'optional_string'\n[provenance]\norigin = 'local'\n").unwrap();
        fs::remove_file(reference.join("mod.rs")).unwrap();
        let rendered = scan(&fixture.directory).unwrap().render().unwrap();
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(BidderDeclaration {
                key: "fake_settings",
                required_feature: Some("experimental"),
                has_settings: cfg!(feature = "experimental"),
            })
        ));
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(
                #[cfg(feature = "experimental")]
                BidderRegistration {
                    key: "fake_settings",
                    factory: fake_settings::create_bidder,
                    config_validator: Some(fake_settings::validate_config),
                }
            )
        ));
        assert!(contains_tokens(
            &rendered,
            &quote::quote!(BidderRegistration {
                key: "fake_plain",
                factory: fake_plain::create_bidder,
                config_validator: None,
            })
        ));
        assert!(!rendered.contains("fake_reference"));
        assert!(!rendered.contains("fake_settings :: config ::"));
    }

    #[test]
    fn settings_callback_is_compiler_checked_only_when_its_factory_is_eligible() {
        let fixture = Fixture::new().unwrap();
        let owner = fixture.bidder("fake_settings", "key = 'fake_settings'\nfactory = true\nrequired_feature = 'experimental'\n[settings.fields]\ntoken = 'optional_string'\n[provenance]\norigin = 'local'\n").unwrap();
        let output = fixture.directory.join("output");
        fs::create_dir_all(&output).unwrap();
        fs::write(
            output.join("discovery.rs"),
            scan(&fixture.directory).unwrap().render().unwrap(),
        )
        .unwrap();
        let source = fixture.directory.join("main.rs");
        fs::write(&source, quote::quote! {
            pub trait Bidder {}
            struct Fake;
            impl Bidder for Fake {}
            pub struct BidderConfigInput;
            pub struct BidderConfigError;
            pub type BidderConfigValidator = for<'a> fn(BidderConfigInput) -> Result<(), BidderConfigError>;
            mod bidders {
                pub struct BidderDeclaration {
                    pub key: &'static str, pub required_feature: Option<&'static str>, pub has_settings: bool,
                }
                pub struct BidderRegistration {
                    pub key: &'static str, pub factory: fn() -> Box<dyn crate::Bidder>,
                    pub config_validator: Option<crate::BidderConfigValidator>,
                }
                include!(concat!(env!("OUT_DIR"), "/discovery.rs"));
            }
            fn main() {
                assert_eq!(bidders::compiled_bidders().len(), usize::from(cfg!(feature = "experimental")));
                assert_eq!(bidders::ALL_DECLARATIONS.len(), 1);
                let declaration = &bidders::ALL_DECLARATIONS[0];
                assert_eq!(declaration.key, "fake_settings");
                assert_eq!(declaration.required_feature, Some("experimental"));
                assert_eq!(declaration.has_settings, cfg!(feature = "experimental"));
                if let Some(registration) = bidders::compiled_bidders().first() {
                    assert_eq!(registration.key, declaration.key);
                    assert!(registration.config_validator.is_some());
                }
                println!("settings eligibility preserved");
            }
        }.to_string()).unwrap();
        let factory = quote::quote!(
            pub(crate) fn create_bidder() -> Box<dyn crate::Bidder> {
                Box::new(crate::Fake)
            }
        );
        for (callback, expected) in [
            (quote::quote!(), Some("cannot find value `validate_config`")),
            (
                quote::quote!(
                    pub(crate) fn validate_config(_: u8) {}
                ),
                Some("mismatched types"),
            ),
            (
                quote::quote!(
                    pub(crate) fn validate_config(
                        _: crate::BidderConfigInput,
                    ) -> Result<(), crate::BidderConfigError> {
                        Ok(())
                    }
                ),
                None,
            ),
        ] {
            fs::write(
                owner.join("mod.rs"),
                quote::quote!(#factory #callback).to_string(),
            )
            .unwrap();
            for enabled in [false, true] {
                let mut command = Command::new("rustc");
                command
                    .args(["--edition=2024", "--crate-name", "fake_validation_probe"])
                    .env("OUT_DIR", &output)
                    .arg(&source)
                    .arg("-o")
                    .arg(fixture.directory.join("probe"));
                if enabled {
                    command.args(["--cfg", "feature=\"experimental\""]);
                }
                let compiled = child_output(&mut command).unwrap();
                let stderr = String::from_utf8_lossy(&compiled.stderr);
                if enabled && let Some(reason) = expected {
                    assert!(
                        !compiled.status.success(),
                        "missing/wrong callback compiled"
                    );
                    assert!(stderr.contains(reason), "{stderr}");
                } else {
                    assert!(compiled.status.success(), "{stderr}");
                    let run =
                        child_output(&mut Command::new(fixture.directory.join("probe"))).unwrap();
                    let diagnostic = String::from_utf8_lossy(&run.stderr);
                    assert!(run.status.success(), "{diagnostic}");
                    assert_eq!(run.stdout, b"settings eligibility preserved\n");
                }
            }
        }
    }

    fn compiler_fixture(fixture: &Fixture) -> TestResult<PathBuf> {
        compiler_bidder_sources(fixture)?;
        fs::write(
            fixture.root.join("url.rs"),
            "pub(super) fn number() -> u32 { 7 }",
        )?;
        fs::write(
            fixture.root.join("media_type.rs"),
            "pub(super) fn number() -> u32 { 13 }",
        )?;
        let scaffold = fixture.root.join("mod.rs");
        fs::write(
            &scaffold,
            r#"
            mod url;
            mod media_type;
            #[cfg(test)]
            pub(crate) mod harness { pub(crate) fn check() {} }
            pub struct BidderDeclaration {
                pub key: &'static str,
                pub required_feature: Option<&'static str>,
                pub has_settings: bool,
            }
            pub struct BidderRegistration {
                pub key: &'static str,
                pub factory: fn() -> Box<dyn crate::Bidder>,
                pub config_validator: Option<fn()>,
            }
            include!(concat!(env!("OUT_DIR"), "/discovery.rs"));
        "#,
        )?;
        let source = fixture.directory.join("src/main.rs");
        fs::write(
            &source,
            quote::quote! {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static A: AtomicUsize = AtomicUsize::new(0);
            static G: AtomicUsize = AtomicUsize::new(0);
            static Z: AtomicUsize = AtomicUsize::new(0);
            pub trait Bidder { fn answer(&self) -> u32; }
            struct Answer(u32);
            impl Bidder for Answer { fn answer(&self) -> u32 { self.0 } }
            mod bidders;
            fn main() {
                let counters = || [A.load(Ordering::SeqCst), G.load(Ordering::SeqCst), Z.load(Ordering::SeqCst)];
                assert_eq!(counters(), [0, 0, 0]);
                let all: Vec<_> = bidders::ALL_DECLARATIONS.iter().map(|row| (row.key, row.required_feature)).collect();
                assert_eq!(all, [("fake_a_signal", None), ("fake_adapterstest", Some("experimental")), ("fake_zeta_probe", None)]);
                assert!(bidders::ALL_DECLARATIONS.iter().all(|row| !row.has_settings));
                let factories = bidders::compiled_bidders();
                assert_eq!(counters(), [0, 0, 0]);
                let enabled = cfg!(feature = "experimental");
                let expected = if enabled { vec!["fake_a_signal", "fake_adapterstest", "fake_zeta_probe"] } else { vec!["fake_a_signal", "fake_zeta_probe"] };
                assert_eq!(factories.iter().map(|row| row.key).collect::<Vec<_>>(), expected);
                for row in factories {
                    assert!(row.config_validator.is_none());
                    let expected = match row.key { "fake_a_signal" => 31, "fake_adapterstest" => 42, "fake_zeta_probe" => 53, _ => panic!("unexpected bidder") };
                    assert_eq!((row.factory)().answer(), expected);
                }
                assert_eq!(counters(), [1, usize::from(enabled), 1]);
                for row in factories { drop((row.factory)()); }
                assert_eq!(counters(), [2, if enabled { 2 } else { 0 }, 2]);
                println!("{}", expected.join(","));
            }
            }.to_string(),
        )?;
        Ok(source)
    }

    #[test]
    fn rustc_executes_unknown_factories_with_siblings_nested_helpers_and_feature_off_on_off() {
        let fixture = Fixture::new().unwrap();
        let source = compiler_fixture(&fixture).unwrap();
        let generated = fixture.directory.join("output");
        fs::create_dir_all(&generated).unwrap();
        let discovery = scan(&fixture.directory).unwrap();
        let rendered = discovery.render().unwrap();
        let marker = fixture.directory.join("constructed");
        assert!(!marker.exists(), "scan/render executed a constructor");
        fs::write(generated.join("discovery.rs"), &rendered).unwrap();
        let other_cwd = tempfile::tempdir().unwrap();
        let binary = generated.join(format!("probe{EXE_SUFFIX}"));
        for enabled in [false, true, false] {
            assert_eq!(
                rendered,
                scan(&fixture.directory).unwrap().render().unwrap()
            );
            let mut rustc = Command::new("rustc");
            rustc
                .current_dir(other_cwd.path())
                .env("OUT_DIR", &generated)
                .args([
                    "--edition=2024",
                    "--cfg",
                    "test",
                    "-Dwarnings",
                    "--crate-name",
                    "discovery_probe",
                ])
                .arg(&source)
                .arg("-o")
                .arg(&binary);
            if enabled {
                rustc.args(["--cfg", "feature=\"experimental\""]);
            }
            let compilation = child_output(&mut rustc).unwrap();
            assert!(
                compilation.status.success(),
                "{}",
                String::from_utf8_lossy(&compilation.stderr)
            );
            assert!(!marker.exists(), "compilation executed a constructor");
            let output = child_output(Command::new(&binary).current_dir(other_cwd.path())).unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                if enabled {
                    "fake_a_signal,fake_adapterstest,fake_zeta_probe\n"
                } else {
                    "fake_a_signal,fake_zeta_probe\n"
                }
            );
            assert!(
                marker.exists(),
                "fixture did not observe actual constructors"
            );
            fs::remove_file(&marker).unwrap();
        }
    }
}
