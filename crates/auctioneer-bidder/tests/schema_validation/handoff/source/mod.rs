//! Current-source consumption window, not semantic/cost authority.
//!
//! The sealed seed/view supply the expected compiler identity. Probe results,
//! paths, bytes and digests are data, not permission to approve changed bodies.
//! The helper prefix and owner-declared context/caller paths share one roster.
//! Supplied-facts collectors and checked bytes are not authority producers.
//! This window does not discharge builtin resolution, helper recipes, resource
//! bounds, target facts, cleanup or qualification. No serving API is introduced.

use crate::schema_build::application::CapturedSeed;
use crate::schema_build::selected::probe_compiler;
use crate::schema_residency::descriptor::ValidationView;
use crate::schema_residency::read_regular;
use crate::schema_validation::work::{self, CallerSourceUnit, SourceRole};
use serde_json::Value;
use sha2::{Digest as _, Sha256};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceUnit {
    Nfa,
    Sparse,
    Ids,
    Arc,
    Vec,
    RawVec,
    SliceIndex,
    NonNull,
    Unique,
    MutPtr,
    Intrinsics,
    Macros,
    CoreOption,
    CoreLib,
    CorePrelude,
    CorePreludeV1,
    RegexRoot,
    RegexMacros,
    RegexUtil,
    RegexManifest,
    Context(SourceRole),
    Caller(CallerSourceUnit),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceRoot {
    Workspace,
    RustLibrary,
}

#[derive(Clone, Copy)]
struct SourceSpec {
    unit: SourceUnit,
    root: SourceRoot,
    relative: &'static str,
}

// Primitive identities only; context/caller paths stay with their owning modules.
// The combined census is not a workload limit or a compiled-source closure.
const ROSTER: [SourceSpec; 20] = [
    SourceSpec {
        unit: SourceUnit::Nfa,
        root: SourceRoot::Workspace,
        relative: "vendor/regex-automata/src/nfa/thompson/nfa.rs",
    },
    SourceSpec {
        unit: SourceUnit::Sparse,
        root: SourceRoot::Workspace,
        relative: "vendor/regex-automata/src/util/sparse_set.rs",
    },
    SourceSpec {
        unit: SourceUnit::Ids,
        root: SourceRoot::Workspace,
        relative: "vendor/regex-automata/src/util/primitives.rs",
    },
    SourceSpec {
        unit: SourceUnit::Arc,
        root: SourceRoot::RustLibrary,
        relative: "alloc/src/sync.rs",
    },
    SourceSpec {
        unit: SourceUnit::Vec,
        root: SourceRoot::RustLibrary,
        relative: "alloc/src/vec/mod.rs",
    },
    SourceSpec {
        unit: SourceUnit::RawVec,
        root: SourceRoot::RustLibrary,
        relative: "alloc/src/raw_vec/mod.rs",
    },
    SourceSpec {
        unit: SourceUnit::SliceIndex,
        root: SourceRoot::RustLibrary,
        relative: "core/src/slice/index.rs",
    },
    SourceSpec {
        unit: SourceUnit::NonNull,
        root: SourceRoot::RustLibrary,
        relative: "core/src/ptr/non_null.rs",
    },
    SourceSpec {
        unit: SourceUnit::Unique,
        root: SourceRoot::RustLibrary,
        relative: "core/src/ptr/unique.rs",
    },
    SourceSpec {
        unit: SourceUnit::MutPtr,
        root: SourceRoot::RustLibrary,
        relative: "core/src/ptr/mut_ptr.rs",
    },
    SourceSpec {
        unit: SourceUnit::Intrinsics,
        root: SourceRoot::RustLibrary,
        relative: "core/src/intrinsics/mod.rs",
    },
    SourceSpec {
        unit: SourceUnit::Macros,
        root: SourceRoot::RustLibrary,
        relative: "core/src/macros/mod.rs",
    },
    // These four records are preparation DATA only, not Option-cost or
    // complete namespace-resolution authority.
    SourceSpec {
        unit: SourceUnit::CoreOption,
        root: SourceRoot::RustLibrary,
        relative: "core/src/option.rs",
    },
    SourceSpec {
        unit: SourceUnit::CoreLib,
        root: SourceRoot::RustLibrary,
        relative: "core/src/lib.rs",
    },
    SourceSpec {
        unit: SourceUnit::CorePrelude,
        root: SourceRoot::RustLibrary,
        relative: "core/src/prelude/mod.rs",
    },
    SourceSpec {
        unit: SourceUnit::CorePreludeV1,
        root: SourceRoot::RustLibrary,
        relative: "core/src/prelude/v1.rs",
    },
    SourceSpec {
        unit: SourceUnit::RegexRoot,
        root: SourceRoot::Workspace,
        relative: "vendor/regex-automata/src/lib.rs",
    },
    SourceSpec {
        unit: SourceUnit::RegexMacros,
        root: SourceRoot::Workspace,
        relative: "vendor/regex-automata/src/macros.rs",
    },
    SourceSpec {
        unit: SourceUnit::RegexUtil,
        root: SourceRoot::Workspace,
        relative: "vendor/regex-automata/src/util/mod.rs",
    },
    SourceSpec {
        unit: SourceUnit::RegexManifest,
        root: SourceRoot::Workspace,
        relative: "vendor/regex-automata/Cargo.toml",
    },
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompilerData {
    executable: PathBuf,
    verbose: String,
}

fn source_roster() -> impl Iterator<Item = SourceSpec> {
    let contexts = work::context_source_paths()
        .iter()
        .map(|&(role, relative)| SourceSpec {
            unit: SourceUnit::Context(role),
            root: SourceRoot::Workspace,
            relative,
        });
    let callers = work::caller_source_paths()
        .iter()
        .map(|&(unit, relative)| SourceSpec {
            unit: SourceUnit::Caller(unit),
            root: SourceRoot::Workspace,
            relative,
        });
    ROSTER.iter().copied().chain(contexts).chain(callers)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceData {
    // Untrusted labels deliberately allow unknown/extra records in controls.
    unit: String,
    path: PathBuf,
    bytes: Vec<u8>,
    sha256: String,
}

pub(in crate::schema_validation) struct SourceInputs<'a> {
    compiler: &'a CompilerData,
    sources: &'a [SourceData],
}

// Borrowed consumed DATA only. There is no constructor, mutable access or
// filesystem selection here; the existing window owns its fixed roster.
impl SourceInputs<'_> {
    pub(in crate::schema_validation) fn compiler(&self) -> (&Path, &str) {
        (&self.compiler.executable, &self.compiler.verbose)
    }

    pub(in crate::schema_validation) fn source_count(&self) -> usize {
        self.sources.len()
    }

    pub(in crate::schema_validation) fn source(&self, unit: &str) -> Option<&[u8]> {
        self.sources
            .iter()
            .find(|source| source.unit == unit)
            .map(|source| source.bytes.as_slice())
    }

    // Borrow checked DATA by an owning roster path, never a proof label or I/O.
    pub(in crate::schema_validation) fn source_record(
        &self,
        relative: &str,
    ) -> Option<(&Path, &[u8], &str)> {
        let mut declared = source_roster().filter(|spec| spec.relative == relative);
        let expected = declared.next()?;
        if declared.next().is_some() {
            return None;
        }
        let unit = format!("{:?}", expected.unit);
        let mut matching = self.sources.iter().filter(|source| source.unit == unit);
        let record = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        Some((
            record.path.as_path(),
            record.bytes.as_slice(),
            record.sha256.as_str(),
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Before,
    After,
}

// Private data seam only. It cannot replace a seed/view/runtime check, choose
// expected identity, construct a descriptor or seal an analysis result.
trait SourceReads {
    fn compiler(&mut self, phase: Phase) -> Result<CompilerData, ()>;
    fn sources(&mut self, phase: Phase) -> Result<Vec<SourceData>, SourceUnit>;
}

#[derive(Debug, Eq, PartialEq)]
enum SourceError<E> {
    Binding,
    CompilerBinding,
    SourceCoverage,
    SourcePath(SourceUnit),
    SourceRead(SourceUnit),
    SourceChanged(SourceUnit),
    Interpretation(E),
}

fn binding_is_current(seed: &CapturedSeed, view: ValidationView<'_>) -> bool {
    let seed_check = super::check_seed(seed, view);
    let runtime_check = view.recheck();
    seed_check.is_ok() && runtime_check.is_ok()
}

fn expected_compiler(seed: &CapturedSeed) -> Result<CompilerData, ()> {
    let manifest: Value = serde_json::from_slice(&seed.manifest_bytes).map_err(|_error| ())?;
    let executable = manifest
        .pointer("/build_environment/RUSTC")
        .and_then(Value::as_str)
        .filter(|path| !path.is_empty())
        .ok_or(())?;
    let verbose = manifest
        .pointer("/build/compiler_verbose")
        .and_then(Value::as_str)
        .filter(|version| !version.is_empty())
        .ok_or(())?;
    if !Path::new(executable).is_absolute() {
        return Err(());
    }
    Ok(CompilerData {
        executable: executable.into(),
        verbose: verbose.to_owned(),
    })
}

fn source_paths(compiler: &CompilerData) -> Result<Vec<PathBuf>, ()> {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .ok_or(())?;
    let library = compiler
        .executable
        .parent()
        .and_then(Path::parent)
        .ok_or(())?
        .join("lib/rustlib/src/rust/library");
    Ok(source_roster()
        .map(|spec| match spec.root {
            SourceRoot::Workspace => workspace.join(spec.relative),
            SourceRoot::RustLibrary => library.join(spec.relative),
        })
        .collect())
}

fn source_digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

struct CurrentSources {
    paths: Vec<PathBuf>,
}

impl SourceReads for CurrentSources {
    fn compiler(&mut self, _phase: Phase) -> Result<CompilerData, ()> {
        probe_compiler()
            .map(|(executable, verbose)| CompilerData {
                executable,
                verbose,
            })
            .map_err(|_error| ())
    }

    fn sources(&mut self, _phase: Phase) -> Result<Vec<SourceData>, SourceUnit> {
        let mut sources = Vec::with_capacity(self.paths.len());
        let mut first_error = None;
        for (spec, path) in source_roster().zip(&self.paths) {
            match read_regular(path) {
                Ok(bytes) => sources.push(SourceData {
                    unit: format!("{:?}", spec.unit),
                    path: path.clone(),
                    sha256: source_digest(&bytes),
                    bytes,
                }),
                Err(_error) => {
                    if first_error.is_none() {
                        first_error = Some(spec.unit);
                    }
                }
            }
        }
        match first_error {
            Some(unit) => Err(unit),
            None => Ok(sources),
        }
    }
}

fn source_row(sources: &[SourceData], unit: SourceUnit) -> Option<&SourceData> {
    let name = format!("{unit:?}");
    sources.iter().find(|source| source.unit == name)
}

fn checked_sources<'a, E>(
    records: &'a Result<Vec<SourceData>, SourceUnit>,
    paths: &[PathBuf],
) -> Result<&'a [SourceData], SourceError<E>> {
    let sources = records
        .as_ref()
        .map_err(|unit| SourceError::SourceRead(*unit))?;
    let expected_count = source_roster().count();
    if sources.len() != expected_count
        || paths.len() != expected_count
        || source_roster().any(|spec| {
            let name = format!("{:?}", spec.unit);
            sources.iter().filter(|source| source.unit == name).count() != 1
        })
    {
        return Err(SourceError::SourceCoverage);
    }
    for (spec, path) in source_roster().zip(paths) {
        let source = source_row(sources, spec.unit).ok_or(SourceError::SourceCoverage)?;
        if source.path != *path {
            return Err(SourceError::SourcePath(spec.unit));
        }
    }
    for spec in source_roster() {
        let source = source_row(sources, spec.unit).ok_or(SourceError::SourceCoverage)?;
        if source.sha256 != source_digest(&source.bytes) {
            return Err(SourceError::SourceChanged(spec.unit));
        }
    }
    Ok(sources)
}

// The facade only supplies real I/O to the same kernel as the private data seam.
fn with_current_sources<T, E>(
    seed: &CapturedSeed,
    view: ValidationView<'_>,
    interpret: impl FnOnce(&SourceInputs<'_>) -> Result<T, E>,
) -> Result<T, SourceError<E>> {
    // Bind before deriving paths; the kernel rechecks at the actual opening.
    if !binding_is_current(seed, view) {
        return Err(SourceError::Binding);
    }
    let expected = expected_compiler(seed).map_err(|()| SourceError::CompilerBinding)?;
    let paths = source_paths(&expected).map_err(|()| SourceError::CompilerBinding)?;
    with_source_data(
        seed,
        view,
        &mut CurrentSources { paths },
        |_phase| {},
        interpret,
    )
}

// The observer reports completed real seed/view/runtime checks; it supplies no
// approval or replacement recheck result. Returned interpretation errors must
// still close the window. Integrity refusals take precedence over that error.
fn with_source_data<T, E>(
    seed: &CapturedSeed,
    view: ValidationView<'_>,
    reads: &mut impl SourceReads,
    mut observe_check: impl FnMut(Phase),
    interpret: impl FnOnce(&SourceInputs<'_>) -> Result<T, E>,
) -> Result<T, SourceError<E>> {
    let opening_binding = binding_is_current(seed, view);
    observe_check(Phase::Before);
    if !opening_binding {
        return Err(SourceError::Binding);
    }
    let expected = expected_compiler(seed).map_err(|()| SourceError::CompilerBinding)?;
    let paths = source_paths(&expected).map_err(|()| SourceError::CompilerBinding)?;
    let compiler_before = reads.compiler(Phase::Before);
    let sources_before = reads.sources(Phase::Before);
    let before_check = checked_sources(&sources_before, &paths);
    let provisional = if compiler_before.as_ref() == Ok(&expected) {
        before_check.map(|sources| {
            interpret(&SourceInputs {
                compiler: &expected,
                sources,
            })
        })
    } else {
        Err(SourceError::CompilerBinding)
    };

    // No returned read/probe/consumer error may skip another closing check.
    let sources_after = reads.sources(Phase::After);
    let after_check = checked_sources(&sources_after, &paths);
    let compiler_after = reads.compiler(Phase::After);
    let closing_binding = binding_is_current(seed, view);
    observe_check(Phase::After);

    // Local priority: binding, either compiler, Before/After source refusals,
    // then consumer refusal. This ordering grants no semantic/cost authority.
    if !closing_binding {
        return Err(SourceError::Binding);
    }
    if compiler_before.as_ref() != Ok(&expected) || compiler_after.as_ref() != Ok(&expected) {
        return Err(SourceError::CompilerBinding);
    }
    let interpreted = provisional?;
    let after = after_check?;
    let before = sources_before
        .as_ref()
        .map_err(|unit| SourceError::SourceRead(*unit))?;
    for spec in source_roster() {
        let prior = source_row(before, spec.unit).ok_or(SourceError::SourceCoverage)?;
        let current = source_row(after, spec.unit).ok_or(SourceError::SourceCoverage)?;
        if prior != current {
            return Err(SourceError::SourceChanged(spec.unit));
        }
    }
    interpreted.map_err(SourceError::Interpretation)
}

#[cfg(test)]
mod tests {
    mod core_routes;
    mod leaf_imports;

    use super::{
        CompilerData, Phase, ROSTER, SourceData, SourceError, SourceInputs, SourceReads,
        SourceRoot, SourceUnit, source_roster, with_current_sources, with_source_data,
    };
    use crate::schema_build::{application::CapturedSeed, selected::probe_compiler};
    use crate::schema_residency::{
        descriptor::ValidationView, read_regular, with_current_descriptor,
    };
    use crate::schema_validation::handoff;
    use crate::schema_validation::work::{
        self, CallerSourceUnit, ConditionalHelperCostProjection, HelperInterpretationRefusal,
        HelperSourceJoinRefusal, HelperSourceUnit, SourceRole,
    };
    use quote::ToTokens as _;
    use serde_json::Value;
    use sha2::{Digest as _, Sha256};
    use std::collections::BTreeSet;
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Seek as _, Write as _};
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
    use std::str::from_utf8;
    use std::{cell::RefCell, path::Path, rc::Rc};

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum ConsumerError {
        UnreviewedSource,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum Event {
        Check(Phase),
        Compiler(Phase),
        Sources(Phase),
        Interpret,
    }

    #[derive(Clone)]
    struct Round {
        compiler: Result<CompilerData, ()>,
        sources: Result<Vec<SourceData>, SourceUnit>,
    }

    struct Replay {
        before: Round,
        after: Round,
        events: Rc<RefCell<Vec<Event>>>,
    }

    impl SourceReads for Replay {
        fn compiler(&mut self, phase: Phase) -> Result<CompilerData, ()> {
            self.events.borrow_mut().push(Event::Compiler(phase));
            match phase {
                Phase::Before => self.before.compiler.clone(),
                Phase::After => self.after.compiler.clone(),
            }
        }

        fn sources(&mut self, phase: Phase) -> Result<Vec<SourceData>, SourceUnit> {
            self.events.borrow_mut().push(Event::Sources(phase));
            match phase {
                Phase::Before => self.before.sources.clone(),
                Phase::After => self.after.sources.clone(),
            }
        }
    }

    struct Fixture {
        expected: CompilerData,
        observed: CompilerData,
        sources: Vec<SourceData>,
    }

    struct SnapshotRestore {
        file: File,
        original: Vec<u8>,
    }

    impl SnapshotRestore {
        fn restore(&mut self) -> io::Result<()> {
            self.file.rewind()?;
            self.file.write_all(&self.original)
        }
    }

    impl Drop for SnapshotRestore {
        fn drop(&mut self) {
            self.restore()
                .expect("restore disposable captured schema snapshot on scope exit");
        }
    }

    fn digest(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    fn fixture(seed: &CapturedSeed, view: ValidationView<'_>) -> Fixture {
        handoff::check_seed(seed, view).expect("actual accepted capture before source setup");
        view.recheck()
            .expect("actual runtime witness before source setup");
        let manifest: Value =
            serde_json::from_slice(&seed.manifest_bytes).expect("accepted manifest JSON");
        let expected = CompilerData {
            executable: manifest
                .pointer("/build_environment/RUSTC")
                .and_then(Value::as_str)
                .expect("captured compiler path")
                .into(),
            verbose: manifest
                .pointer("/build/compiler_verbose")
                .and_then(Value::as_str)
                .expect("captured compiler identity")
                .to_owned(),
        };
        let (executable, verbose) = probe_compiler().expect("existing bounded compiler probe");
        let observed = CompilerData {
            executable,
            verbose,
        };
        assert_eq!(
            (&observed.executable, &observed.verbose),
            (&expected.executable, &expected.verbose),
            "expected identity comes from the seed, not a probe clone"
        );
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace parent");
        let library = observed
            .executable
            .parent()
            .and_then(Path::parent)
            .expect("matched compiler sysroot")
            .join("lib/rustlib/src/rust/library");
        // Setup snapshots are data only, not a source-consumption window.
        let sources = source_roster()
            .map(|spec| {
                let root = match spec.root {
                    SourceRoot::Workspace => workspace,
                    SourceRoot::RustLibrary => &library,
                };
                let path = root.join(spec.relative);
                let bytes = read_regular(&path).expect("existing bounded regular source reader");
                assert!(!bytes.is_empty());
                SourceData {
                    unit: format!("{:?}", spec.unit),
                    path,
                    sha256: digest(&bytes),
                    bytes,
                }
            })
            .collect();
        Fixture {
            expected,
            observed,
            sources,
        }
    }

    fn replay(data: &Fixture) -> Replay {
        let round = Round {
            compiler: Ok(data.observed.clone()),
            sources: Ok(data.sources.clone()),
        };
        let mut reads = Replay {
            before: round.clone(),
            after: round,
            events: Rc::default(),
        };
        // Verify the supplied replay data before asking the stub to evaluate it.
        assert_eq!(reads.compiler(Phase::Before), Ok(data.expected.clone()));
        assert_eq!(reads.sources(Phase::Before), Ok(data.sources.clone()));
        reads.events.borrow_mut().clear();
        reads
    }

    fn inspect_inputs(inputs: &SourceInputs<'_>, data: &Fixture) -> usize {
        assert_eq!(inputs.compiler, &data.expected);
        assert_eq!(inputs.sources, data.sources.as_slice());
        assert_eq!(inputs.source_count(), 44);
        for (source, spec) in inputs.sources.iter().zip(source_roster()) {
            assert_eq!(source.unit, format!("{:?}", spec.unit));
            assert!(source.path.is_absolute());
            assert!(source.path.ends_with(spec.relative));
            assert_eq!(source.sha256, digest(&source.bytes));
        }
        inputs.source_count()
    }

    fn run_data(
        seed: &CapturedSeed,
        view: ValidationView<'_>,
        data: &Fixture,
        reads: &mut Replay,
        fail: bool,
    ) -> Result<usize, SourceError<ConsumerError>> {
        let checks = Rc::clone(&reads.events);
        let interpretation = Rc::clone(&reads.events);
        with_source_data(
            seed,
            view,
            reads,
            |phase| checks.borrow_mut().push(Event::Check(phase)),
            |inputs| {
                interpretation.borrow_mut().push(Event::Interpret);
                let count = inspect_inputs(inputs, data);
                if fail {
                    Err(ConsumerError::UnreviewedSource)
                } else {
                    Ok(count)
                }
            },
        )
    }

    fn expected_events() -> Vec<Event> {
        vec![
            Event::Check(Phase::Before),
            Event::Compiler(Phase::Before),
            Event::Sources(Phase::Before),
            Event::Interpret,
            Event::Sources(Phase::After),
            Event::Compiler(Phase::After),
            Event::Check(Phase::After),
        ]
    }

    const INDEPENDENT_SOURCE_ROSTER: [(SourceUnit, SourceRoot, &str); 20] = [
        (
            SourceUnit::Nfa,
            SourceRoot::Workspace,
            "vendor/regex-automata/src/nfa/thompson/nfa.rs",
        ),
        (
            SourceUnit::Sparse,
            SourceRoot::Workspace,
            "vendor/regex-automata/src/util/sparse_set.rs",
        ),
        (
            SourceUnit::Ids,
            SourceRoot::Workspace,
            "vendor/regex-automata/src/util/primitives.rs",
        ),
        (
            SourceUnit::Arc,
            SourceRoot::RustLibrary,
            "alloc/src/sync.rs",
        ),
        (
            SourceUnit::Vec,
            SourceRoot::RustLibrary,
            "alloc/src/vec/mod.rs",
        ),
        (
            SourceUnit::RawVec,
            SourceRoot::RustLibrary,
            "alloc/src/raw_vec/mod.rs",
        ),
        (
            SourceUnit::SliceIndex,
            SourceRoot::RustLibrary,
            "core/src/slice/index.rs",
        ),
        (
            SourceUnit::NonNull,
            SourceRoot::RustLibrary,
            "core/src/ptr/non_null.rs",
        ),
        (
            SourceUnit::Unique,
            SourceRoot::RustLibrary,
            "core/src/ptr/unique.rs",
        ),
        (
            SourceUnit::MutPtr,
            SourceRoot::RustLibrary,
            "core/src/ptr/mut_ptr.rs",
        ),
        (
            SourceUnit::Intrinsics,
            SourceRoot::RustLibrary,
            "core/src/intrinsics/mod.rs",
        ),
        (
            SourceUnit::Macros,
            SourceRoot::RustLibrary,
            "core/src/macros/mod.rs",
        ),
        (
            SourceUnit::CoreOption,
            SourceRoot::RustLibrary,
            "core/src/option.rs",
        ),
        (
            SourceUnit::CoreLib,
            SourceRoot::RustLibrary,
            "core/src/lib.rs",
        ),
        (
            SourceUnit::CorePrelude,
            SourceRoot::RustLibrary,
            "core/src/prelude/mod.rs",
        ),
        (
            SourceUnit::CorePreludeV1,
            SourceRoot::RustLibrary,
            "core/src/prelude/v1.rs",
        ),
        (
            SourceUnit::RegexRoot,
            SourceRoot::Workspace,
            "vendor/regex-automata/src/lib.rs",
        ),
        (
            SourceUnit::RegexMacros,
            SourceRoot::Workspace,
            "vendor/regex-automata/src/macros.rs",
        ),
        (
            SourceUnit::RegexUtil,
            SourceRoot::Workspace,
            "vendor/regex-automata/src/util/mod.rs",
        ),
        (
            SourceUnit::RegexManifest,
            SourceRoot::Workspace,
            "vendor/regex-automata/Cargo.toml",
        ),
    ];

    const INDEPENDENT_ADDITIONAL_SOURCES: [(SourceUnit, &str); 24] = [
        (
            SourceUnit::Context(SourceRole::Native),
            "crates/auctioneer-bidder/src/schema/native.rs",
        ),
        (
            SourceUnit::Context(SourceRole::Budget),
            "crates/auctioneer-bidder/src/schema/mod.rs",
        ),
        (
            SourceUnit::Context(SourceRole::Input),
            "crates/auctioneer-bidder/src/schema/input.rs",
        ),
        (
            SourceUnit::Context(SourceRole::JsonSize),
            "crates/auctioneer-runtime/src/json_size.rs",
        ),
        (
            SourceUnit::Context(SourceRole::ExchangeSchema),
            "crates/auctioneer-exchange/src/run/schema.rs",
        ),
        (
            SourceUnit::Context(SourceRole::ExchangeRun),
            "crates/auctioneer-exchange/src/run/mod.rs",
        ),
        (
            SourceUnit::Context(SourceRole::ExchangeSplit),
            "crates/auctioneer-exchange/src/split/mod.rs",
        ),
        (
            SourceUnit::Context(SourceRole::ExchangeLimits),
            "crates/auctioneer-exchange/src/limits.rs",
        ),
        (
            SourceUnit::Context(SourceRole::DispatchGenerator),
            "crates/auctioneer-config/src/schema_staging_build.rs",
        ),
        (
            SourceUnit::Context(SourceRole::Formats),
            "crates/auctioneer-bidder/src/schema/staging.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::CorePrepare),
            "crates/auctioneer-core/src/auction/prepare.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::CoreServices),
            "crates/auctioneer-core/src/auction/services.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::CoreOperation),
            "crates/auctioneer-core/src/auction/operation.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::PolicyHost),
            "crates/auctioneer-core/src/config/policy_host.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::PolicyResolve),
            "crates/auctioneer-core/src/config/resolve.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::NormalizedPrepare),
            "crates/auctioneer-openrtb/src/prepare/mod.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::ImplicitFill),
            "crates/auctioneer-openrtb/src/prepare/implicit_fill.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::ExchangeContext),
            "crates/auctioneer-exchange/src/context.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::RuntimeBudget),
            "crates/auctioneer-runtime/src/budget.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::CoreAuctionRoot),
            "crates/auctioneer-core/src/auction/mod.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::CoreConfigRoot),
            "crates/auctioneer-core/src/config/mod.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::OpenRtbRoot),
            "crates/auctioneer-openrtb/src/lib.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::RuntimeRoot),
            "crates/auctioneer-runtime/src/lib.rs",
        ),
        (
            SourceUnit::Caller(CallerSourceUnit::ExchangeRoot),
            "crates/auctioneer-exchange/src/lib.rs",
        ),
    ];

    fn assert_roster() {
        assert_eq!(
            ROSTER
                .iter()
                .map(|spec| (spec.unit, spec.root, spec.relative))
                .collect::<Vec<_>>(),
            INDEPENDENT_SOURCE_ROSTER
        );
        let complete: Vec<_> = source_roster().collect();
        assert_eq!(
            complete
                .iter()
                .map(|spec| (spec.unit, spec.root, spec.relative))
                .collect::<Vec<_>>(),
            INDEPENDENT_SOURCE_ROSTER
                .into_iter()
                .chain(
                    INDEPENDENT_ADDITIONAL_SOURCES
                        .into_iter()
                        .map(|(unit, path)| (unit, SourceRoot::Workspace, path))
                )
                .collect::<Vec<_>>()
        );
        assert_eq!(complete.len(), 44);
        assert_eq!(
            complete
                .iter()
                .filter(|spec| matches!(spec.unit, SourceUnit::Context(_)))
                .count(),
            10
        );
        assert_eq!(
            complete
                .iter()
                .filter(|spec| matches!(spec.unit, SourceUnit::Caller(_)))
                .count(),
            14
        );
        let units: BTreeSet<_> = complete
            .iter()
            .map(|spec| format!("{:?}", spec.unit))
            .collect();
        let paths: BTreeSet<_> = complete.iter().map(|spec| spec.relative).collect();
        assert_eq!(units.len(), 44);
        assert_eq!(paths.len(), 44);
    }

    fn assert_opening_coverage_control(
        seed: &CapturedSeed,
        view: ValidationView<'_>,
        data: &Fixture,
    ) {
        let mut reads = replay(data);
        assert_eq!(run_data(seed, view, data, &mut reads, false), Ok(44));
        assert_eq!(*reads.events.borrow(), expected_events());
        // CoreOption is not a helper dependency; global coverage still
        // gates consumer entry and retains every closing check.
        let mut missing = replay(data);
        let sources = missing.before.sources.as_mut().unwrap();
        let option = sources
            .iter()
            .position(|source| source.unit == "CoreOption")
            .expect("CoreOption present before independent opening coverage control");
        assert_eq!(sources[option], data.sources[option]);
        assert_eq!(sources.remove(option), data.sources[option]);
        assert_eq!(
            run_data(seed, view, data, &mut missing, false),
            Err(SourceError::SourceCoverage)
        );
        assert_eq!(
            *missing.events.borrow(),
            vec![
                Event::Check(Phase::Before),
                Event::Compiler(Phase::Before),
                Event::Sources(Phase::Before),
                Event::Sources(Phase::After),
                Event::Compiler(Phase::After),
                Event::Check(Phase::After),
            ]
        );
    }

    fn assert_postcapture_control(seed: &CapturedSeed, view: ValidationView<'_>, data: &Fixture) {
        for fail in [false, true] {
            handoff::check_seed(seed, view).expect("valid seed before disposable snapshot control");
            view.recheck()
                .expect("valid runtime before snapshot control");
            let root = seed.build_root.canonicalize().unwrap();
            let output = seed.out_dir.canonicalize().unwrap();
            assert_eq!(root, seed.build_root);
            assert_eq!(output, seed.out_dir);
            assert!(output.starts_with(&root));
            assert_eq!(
                fs::symlink_metadata(&root).unwrap().permissions().mode() & 0o777,
                0o700
            );
            let snapshot = seed
                .snapshots
                .first()
                .expect("captured disposable schema snapshot");
            let path = output.join(&snapshot.relative_path);
            assert_eq!(path.parent(), Some(output.as_path()));
            assert!(path.starts_with(&root));
            assert!(path.starts_with(&output));
            assert_eq!(path.canonicalize().unwrap(), path);
            let metadata = fs::symlink_metadata(&path).unwrap();
            let is_regular_file = metadata.is_file();
            assert!(is_regular_file);
            assert_eq!(metadata.nlink(), 1);
            let original = read_regular(&path).unwrap();
            assert_eq!(original, snapshot.bytes);
            assert_eq!(digest(&original), snapshot.sha256);
            let file = OpenOptions::new()
                .read(true)
                .write(true)
                .open(&path)
                .unwrap();
            let opened = file.metadata().unwrap();
            assert!(opened.is_file());
            assert_eq!(opened.nlink(), 1);
            assert_eq!(
                (opened.dev(), opened.ino()),
                (metadata.dev(), metadata.ino())
            );
            let mut restore = SnapshotRestore { file, original };
            let mut interpreted = false;
            let refused = with_current_sources(seed, view, |inputs| {
                let count = inspect_inputs(inputs, data);
                let changed = restore
                    .original
                    .first()
                    .copied()
                    .expect("nonempty schema snapshot")
                    ^ 1_u8;
                restore.file.rewind().unwrap();
                restore.file.write_all(&[changed]).unwrap();
                interpreted = true;
                assert_ne!(read_regular(&path).unwrap(), restore.original);
                assert_eq!(
                    seed.recheck_snapshots(),
                    Err("stale application schema snapshot".to_owned())
                );
                if fail {
                    Err(ConsumerError::UnreviewedSource)
                } else {
                    Ok::<_, ConsumerError>(count)
                }
            });
            // Restore before result assertions; the guard also restores on unwind.
            restore
                .restore()
                .expect("restore after source interpretation");
            assert_eq!(read_regular(&path).unwrap(), restore.original);
            handoff::check_seed(seed, view).expect("restored captured seed");
            view.recheck().expect("restored captured runtime");
            assert!(interpreted);
            assert_eq!(refused, Err(SourceError::Binding));
        }
    }

    #[test]
    fn actual_sources_are_consumed_inside_the_bound_window() {
        let checked = with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            let data = fixture(seed, view);
            assert_roster();
            // Explicit fictional invocation/receiver/invariant DATA, not
            // evidence of actual caller counts, buffers, ownership or epochs.
            let demands = work::conditional_demands();
            let controls = work::conditional_controls();
            let mut semantic_setup = None;
            let mut helper_costs = None;
            let mut refused_costs = None;
            let mut caller_setup = None;
            let mut caller_projection = None;
            let mut runtime_sources = None;
            let actual = with_current_sources(seed, view, |inputs| {
                let count = inspect_inputs(inputs, &data);
                runtime_sources = Some(
                    [
                        "vendor/regex-automata/src/meta/regex.rs",
                        "vendor/regex-automata/src/meta/strategy.rs",
                        "vendor/regex-automata/src/meta/wrappers.rs",
                        "vendor/regex-automata/src/hybrid/regex.rs",
                        "vendor/regex-automata/src/hybrid/dfa.rs",
                        "vendor/regex-automata/src/dfa/onepass.rs",
                        "vendor/regex-automata/src/nfa/thompson/pikevm.rs",
                        "vendor/regex-automata/src/nfa/thompson/backtrack.rs",
                        "vendor/regex-automata/src/util/captures.rs",
                        "vendor/regex-automata/src/util/determinize/state.rs",
                    ]
                    .map(|relative| {
                        (
                            relative,
                            inputs.source_record(relative).map(|(path, bytes, sha256)| {
                                (
                                    path.ends_with(relative),
                                    !bytes.is_empty(),
                                    digest(bytes) == sha256,
                                )
                            }),
                        )
                    }),
                );
                semantic_setup = Some(work::check_helper_source_setup(inputs));
                helper_costs = Some(
                    demands
                        .iter()
                        .map(|demand| work::join_helper_costs(inputs, demand))
                        .collect::<Vec<_>>(),
                );
                refused_costs = Some(
                    controls
                        .iter()
                        .map(|(demand, _)| work::join_helper_costs(inputs, demand))
                        .collect::<Vec<_>>(),
                );
                caller_setup = Some(work::consumed_source_setup(inputs, seed, view));
                caller_projection = Some([
                    work::join_caller_sources(inputs, seed, view, 1),
                    work::join_caller_sources(inputs, seed, view, 0),
                ]);
                Ok::<_, ConsumerError>(count)
            });
            assert_eq!(actual, Ok(44));
            assert_opening_coverage_control(seed, view, &data);
            assert_postcapture_control(seed, view, &data);
            let changed = CapturedSeed {
                identity_sha256: seed.identity_sha256.clone(),
                manifest_bytes: seed.manifest_bytes.clone(),
                rust: [seed.rust.as_slice(), b" "].concat(),
                build_root: seed.build_root.clone(),
                out_dir: seed.out_dir.clone(),
                snapshots: seed.snapshots.clone(),
            };
            assert_ne!(changed.rust, seed.rust);
            let refused =
                with_current_sources(&changed, view, |_inputs| -> Result<usize, ConsumerError> {
                    panic!("binding drift must precede interpretation")
                });
            assert_eq!(refused, Err(SourceError::Binding));
            // Only inspect semantic/join results after all prior real source,
            // compiler, seed/view checks and their existing controls complete.
            assert_eq!(semantic_setup, Some(Ok(())));
            let semantic_refusals = assert_semantic_source_control(seed, view, &data);
            work::assert_demand_setup(&demands);
            work::assert_cost_projections(&demands, helper_costs.as_ref().unwrap());
            let checked_refusals = refused_costs.as_ref().unwrap();
            assert_eq!(checked_refusals.len(), controls.len());
            for (observed_cost, (_, expected)) in checked_refusals.iter().zip(&controls) {
                assert_eq!(observed_cost.as_ref().err(), Some(expected));
            }
            for semantic_refusal in semantic_refusals {
                assert_eq!(
                    semantic_refusal,
                    Err(HelperSourceJoinRefusal::SourceInterpretation(
                        HelperInterpretationRefusal::UnsupportedBody(HelperSourceUnit::Nfa)
                    ))
                );
            }
            assert_vendor_ancestry_controls(seed, view, &data);
            core_routes::assert_controls(seed, view, &data);
            leaf_imports::assert_controls(seed, view, &data);
            Ok((caller_setup, caller_projection, runtime_sources))
        })
        .unwrap();
        // The outer descriptor/artifact checks must also finish before RED.
        let (caller_setup, caller_projection, runtime_sources) = checked;
        assert_eq!(caller_setup, Some(Ok(())));
        work::assert_consumed_projection(caller_projection.unwrap());
        // Consume only after both source and outer descriptor checks close.
        // These records grant identity/availability, not runtime cost authority.
        let runtime_sources = runtime_sources.expect("source interpretation executed");
        assert_eq!(runtime_sources.len(), 10);
        for (relative, observed) in runtime_sources {
            assert_eq!(
                observed,
                Some((true, true, true)),
                "runtime source: {relative}"
            );
        }
    }

    #[derive(Clone, Copy, Debug)]
    enum VendorDrift {
        RootShadow,
        ExternMacros,
        ProviderPath,
        ProviderShadow,
        ProviderBody,
        UtilShadow,
        SparsePath,
        Edition,
        LibraryPath,
        NoStd,
        ImplicitPrelude,
    }

    impl VendorDrift {
        fn unit(self) -> &'static str {
            match self {
                Self::RootShadow
                | Self::ExternMacros
                | Self::ProviderPath
                | Self::NoStd
                | Self::ImplicitPrelude => "RegexRoot",
                Self::ProviderShadow | Self::ProviderBody => "RegexMacros",
                Self::UtilShadow | Self::SparsePath => "RegexUtil",
                Self::Edition | Self::LibraryPath => "RegexManifest",
            }
        }

        fn manifest_fields(
            self,
        ) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
            match self {
                Self::Edition => Some(("package", "edition", "2021", "2024")),
                Self::LibraryPath => Some(("lib", "path", "src/lib.rs", "src/fake_lib.rs")),
                Self::RootShadow
                | Self::ExternMacros
                | Self::ProviderPath
                | Self::ProviderShadow
                | Self::ProviderBody
                | Self::UtilShadow
                | Self::SparsePath
                | Self::NoStd
                | Self::ImplicitPrelude => None,
            }
        }
    }

    fn module_mut<'a>(file: &'a mut syn::File, name: &str) -> &'a mut syn::ItemMod {
        assert_eq!(
            file.items
                .iter()
                .filter(|item| matches!(item,
            syn::Item::Mod(module) if module.ident == name))
                .count(),
            1
        );
        file.items
            .iter_mut()
            .find_map(|item| {
                let syn::Item::Mod(module) = item else {
                    return None;
                };
                (module.ident == name).then_some(module)
            })
            .unwrap()
    }

    fn import_std_macros(syntax: &mut syn::File) {
        let externs: Vec<_> = syntax
            .items
            .iter_mut()
            .filter_map(|item| {
                let syn::Item::ExternCrate(external) = item else {
                    return None;
                };
                (external.ident == "std").then_some(external)
            })
            .collect();
        assert_eq!(externs.len(), 1);
        for external in externs {
            assert!(
                !external
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("macro_use"))
            );
            external.attrs.push(syn::parse_quote!(#[macro_use]));
        }
    }

    fn change_log_provider(syntax: &mut syn::File) {
        let definitions: Vec<_> = syntax
            .items
            .iter_mut()
            .filter_map(|item| {
                let syn::Item::Macro(definition) = item else {
                    return None;
                };
                definition
                    .ident
                    .as_ref()
                    .is_some_and(|name| name == "log")
                    .then_some(definition)
            })
            .collect();
        assert_eq!(definitions.len(), 1);
        for definition in definitions {
            let before = definition.mac.tokens.clone();
            definition.mac.tokens = quote::quote! {
                ($($tt:tt)*) => { fake_extra_work(); $($tt)* }
            };
            assert_ne!(before.to_string(), definition.mac.tokens.to_string());
        }
    }

    fn vendor_source_drift(data: &Fixture, change: VendorDrift) -> Fixture {
        let mut changed = Fixture {
            expected: data.expected.clone(),
            observed: data.observed.clone(),
            sources: data.sources.clone(),
        };
        let record = changed
            .sources
            .iter_mut()
            .find(|record| record.unit == change.unit())
            .unwrap();
        let original = record.bytes.clone();
        if let Some((section, field, before, after)) = change.manifest_fields() {
            let mut manifest: toml::Value = toml::from_str(from_utf8(&original).unwrap()).unwrap();
            assert_eq!(manifest[section][field].as_str(), Some(before));
            manifest[section][field] = toml::Value::String(after.to_owned());
            record.bytes = toml::to_string(&manifest).unwrap().into_bytes();
        } else {
            let mut syntax = syn::parse_file(from_utf8(&original).unwrap()).unwrap();
            match change {
                VendorDrift::RootShadow | VendorDrift::UtilShadow | VendorDrift::ProviderShadow => {
                    syntax.items.insert(
                        0,
                        syn::parse_quote!(
                            macro_rules! assert {
                                ($($argument:tt)*) => {
                                    fake_extra_work();
                                };
                            }
                        ),
                    );
                }
                VendorDrift::ExternMacros => import_std_macros(&mut syntax),
                VendorDrift::ProviderPath | VendorDrift::SparsePath => {
                    let name = if matches!(change, VendorDrift::ProviderPath) {
                        "macros"
                    } else {
                        "sparse_set"
                    };
                    let module = module_mut(&mut syntax, name);
                    assert!(
                        !module
                            .attrs
                            .iter()
                            .any(|attribute| attribute.path().is_ident("path"))
                    );
                    module
                        .attrs
                        .push(syn::parse_quote!(#[path = "fake_provider.rs"]));
                }
                VendorDrift::ProviderBody => change_log_provider(&mut syntax),
                VendorDrift::NoStd => {
                    let before = syntax.attrs.len();
                    syntax
                        .attrs
                        .retain(|attribute| !attribute.path().is_ident("no_std"));
                    assert_eq!(syntax.attrs.len().checked_add(1), Some(before));
                }
                VendorDrift::ImplicitPrelude => {
                    assert!(
                        !syntax
                            .attrs
                            .iter()
                            .any(|attribute| attribute.path().is_ident("no_implicit_prelude"))
                    );
                    syntax
                        .attrs
                        .push(syn::parse_quote!(#![no_implicit_prelude]));
                }
                VendorDrift::Edition | VendorDrift::LibraryPath => {
                    panic!("manifest changes use TOML, not the Rust syntax control")
                }
            }
            record.bytes = syntax.to_token_stream().to_string().into_bytes();
        }
        assert_ne!(
            record.bytes, original,
            "{change:?} changes source before rehashing"
        );
        record.sha256 = digest(&record.bytes);
        for unit in ["Nfa", "Sparse", "Ids"] {
            assert_eq!(
                changed.sources.iter().find(|row| row.unit == unit),
                data.sources.iter().find(|row| row.unit == unit),
                "helper bodies stay unchanged"
            );
        }
        changed
    }

    fn assert_vendor_ancestry_controls(
        seed: &CapturedSeed,
        view: ValidationView<'_>,
        data: &Fixture,
    ) {
        let demands = work::conditional_demands();
        work::assert_demand_setup(&demands);
        let mut checked = 0_usize;
        for change in [
            VendorDrift::RootShadow,
            VendorDrift::ExternMacros,
            VendorDrift::ProviderPath,
            VendorDrift::ProviderShadow,
            VendorDrift::ProviderBody,
            VendorDrift::UtilShadow,
            VendorDrift::SparsePath,
            VendorDrift::Edition,
            VendorDrift::LibraryPath,
            VendorDrift::NoStd,
            VendorDrift::ImplicitPrelude,
        ] {
            let changed = vendor_source_drift(data, change);
            let mut reads = replay(&changed);
            let checks = Rc::clone(&reads.events);
            let interpretation = Rc::clone(&reads.events);
            let mut costs = None;
            let actual = with_source_data(
                seed,
                view,
                &mut reads,
                |phase| checks.borrow_mut().push(Event::Check(phase)),
                |inputs| {
                    interpretation.borrow_mut().push(Event::Interpret);
                    // Positive and zero sparse invocations have the same source duties.
                    costs = Some([
                        work::join_helper_costs(inputs, &demands[3]),
                        work::join_helper_costs(inputs, &demands[5]),
                    ]);
                    work::check_helper_source_setup(inputs)
                },
            );
            let expected = HelperSourceJoinRefusal::SourceInterpretation(
                HelperInterpretationRefusal::VendorAncestry(change.unit()),
            );
            assert_eq!(
                *reads.events.borrow(),
                expected_events(),
                "{change:?}: close on every refusal"
            );
            assert_eq!(
                actual,
                Err(SourceError::Interpretation(expected)),
                "{change:?}"
            );
            for cost in costs.unwrap() {
                assert_eq!(
                    cost,
                    Err(HelperSourceJoinRefusal::SourceInterpretation(
                        HelperInterpretationRefusal::VendorAncestry(change.unit())
                    )),
                    "{change:?}"
                );
                checked = checked.checked_add(1).unwrap();
            }
        }
        assert_eq!(checked, 22);
    }

    fn semantic_source_drift(data: &Fixture) -> Fixture {
        let mut changed = Fixture {
            expected: data.expected.clone(),
            observed: data.observed.clone(),
            sources: data.sources.clone(),
        };
        let record = changed
            .sources
            .iter_mut()
            .find(|source| source.unit == "Nfa")
            .unwrap();
        let mut syntax = syn::parse_file(from_utf8(&record.bytes).unwrap()).unwrap();
        let mut hits = 0_usize;
        for item in &mut syntax.items {
            let syn::Item::Impl(implementation) = item else {
                continue;
            };
            if implementation.self_ty.to_token_stream().to_string() != "State" {
                continue;
            }
            for member in &mut implementation.items {
                let syn::ImplItem::Fn(method) = member else {
                    continue;
                };
                if method.sig.ident != "is_epsilon" {
                    continue;
                }
                let [syn::Stmt::Expr(syn::Expr::Match(decision), None)] =
                    method.block.stmts.as_mut_slice()
                else {
                    panic!("actual State::is_epsilon match before-state");
                };
                assert_eq!(decision.arms.len(), 2);
                let syn::Expr::Lit(literal) = decision.arms[0].body.as_mut() else {
                    panic!("actual non-epsilon false arm");
                };
                assert!(matches!(&literal.lit, syn::Lit::Bool(value) if !value.value));
                literal.lit = syn::parse_quote!(true);
                hits = hits.checked_add(1).unwrap();
            }
        }
        assert_eq!(hits, 1);
        let original = record.bytes.clone();
        record.bytes = syntax.to_token_stream().to_string().into_bytes();
        assert_ne!(record.bytes, original);
        record.sha256 = hex::encode(Sha256::digest(&record.bytes));
        changed
    }

    fn assert_semantic_source_control(
        seed: &CapturedSeed,
        view: ValidationView<'_>,
        data: &Fixture,
    ) -> [Result<ConditionalHelperCostProjection, HelperSourceJoinRefusal>; 2] {
        // Only owned replay bytes change. No vendor/std/build source is written.
        let changed = semantic_source_drift(data);
        let mut reads = replay(&changed);
        let events = Rc::clone(&reads.events);
        let interpreted = Rc::clone(&reads.events);
        let demands = work::conditional_demands();
        let mut cost_results = None;
        let refused = with_source_data(
            seed,
            view,
            &mut reads,
            |phase| events.borrow_mut().push(Event::Check(phase)),
            |inputs| {
                interpreted.borrow_mut().push(Event::Interpret);
                cost_results = Some([
                    work::join_helper_costs(inputs, &demands[0]),
                    work::join_helper_costs(inputs, &demands[4]),
                ]);
                work::check_helper_source_setup(inputs)
            },
        );
        assert_eq!(
            refused,
            Err(SourceError::Interpretation(
                HelperSourceJoinRefusal::SourceInterpretation(
                    HelperInterpretationRefusal::UnsupportedBody(HelperSourceUnit::Nfa)
                )
            ))
        );
        assert_eq!(*reads.events.borrow(), expected_events());
        cost_results.unwrap()
    }

    #[test]
    fn compiler_identity_must_match_the_sealed_seed() {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            let data = fixture(seed, view);
            let mut valid = replay(&data);
            assert_eq!(run_data(seed, view, &data, &mut valid, false), Ok(44));
            for phase in [Phase::Before, Phase::After] {
                for change_path in [true, false] {
                    let mut reads = replay(&data);
                    let round = match phase {
                        Phase::Before => &mut reads.before,
                        Phase::After => &mut reads.after,
                    };
                    let compiler = round.compiler.as_mut().unwrap();
                    if change_path {
                        compiler.executable = compiler.executable.with_file_name("unmatched-rustc");
                    } else {
                        compiler.verbose.push_str("\nunmatched compiler identity\n");
                    }
                    assert_ne!(*compiler, data.expected);
                    assert_eq!(
                        run_data(seed, view, &data, &mut reads, false),
                        Err(SourceError::CompilerBinding)
                    );
                }
            }
            // Probe availability is independent of path/version correspondence.
            for (phase, fail) in [
                (Phase::Before, false),
                (Phase::After, false),
                (Phase::After, true),
            ] {
                let mut unavailable = replay(&data);
                let round = match phase {
                    Phase::Before => &mut unavailable.before,
                    Phase::After => &mut unavailable.after,
                };
                assert_eq!(round.compiler, Ok(data.expected.clone()));
                round.compiler = Err(());
                assert_eq!(
                    run_data(seed, view, &data, &mut unavailable, fail),
                    Err(SourceError::CompilerBinding)
                );
                if phase == Phase::After {
                    assert_eq!(*unavailable.events.borrow(), expected_events());
                } else {
                    assert!(!unavailable.events.borrow().contains(&Event::Interpret));
                }
            }
            let mut agreeing = replay(&data);
            agreeing
                .before
                .compiler
                .as_mut()
                .unwrap()
                .verbose
                .push_str("\nunmatched compiler identity\n");
            agreeing
                .after
                .compiler
                .clone_from(&agreeing.before.compiler);
            assert_eq!(agreeing.before.compiler, agreeing.after.compiler);
            assert_ne!(agreeing.before.compiler, Ok(data.expected.clone()));
            assert_eq!(
                run_data(seed, view, &data, &mut agreeing, false),
                Err(SourceError::CompilerBinding)
            );
            Ok(())
        })
        .unwrap();
    }

    fn assert_coverage_controls(seed: &CapturedSeed, view: ValidationView<'_>, data: &Fixture) {
        for phase in [Phase::Before, Phase::After] {
            for (index, original) in data.sources.iter().enumerate() {
                let mut missing = replay(data);
                let missing_round = match phase {
                    Phase::Before => &mut missing.before,
                    Phase::After => &mut missing.after,
                };
                assert_eq!(
                    missing_round.sources.as_mut().unwrap().remove(index),
                    *original
                );
                assert_eq!(
                    run_data(seed, view, data, &mut missing, false),
                    Err(SourceError::SourceCoverage)
                );
                let mut duplicate = replay(data);
                let duplicate_round = match phase {
                    Phase::Before => &mut duplicate.before,
                    Phase::After => &mut duplicate.after,
                };
                duplicate_round
                    .sources
                    .as_mut()
                    .unwrap()
                    .push(original.clone());
                assert_eq!(
                    run_data(seed, view, data, &mut duplicate, false),
                    Err(SourceError::SourceCoverage)
                );
            }
            let mut extra = replay(data);
            let mut unknown = data.sources.first().unwrap().clone();
            unknown.unit = "UnreviewedSourceUnit".to_owned();
            let extra_round = match phase {
                Phase::Before => &mut extra.before,
                Phase::After => &mut extra.after,
            };
            extra_round.sources.as_mut().unwrap().push(unknown);
            assert_eq!(
                run_data(seed, view, data, &mut extra, false),
                Err(SourceError::SourceCoverage)
            );
        }
    }

    fn assert_hash_controls(seed: &CapturedSeed, view: ValidationView<'_>, data: &Fixture) {
        for (index, spec) in source_roster().enumerate() {
            for phase in [Phase::Before, Phase::After] {
                let mut reads = replay(data);
                let round = match phase {
                    Phase::Before => &mut reads.before,
                    Phase::After => &mut reads.after,
                };
                let source = round.sources.as_mut().unwrap().get_mut(index).unwrap();
                let original = &data.sources[index];
                assert_eq!(*source, *original);
                assert_eq!(source.sha256, digest(&source.bytes));
                assert_eq!(read_regular(&source.path).unwrap(), original.bytes);
                let replacement = if source.sha256.starts_with('0') {
                    "1"
                } else {
                    "0"
                };
                source.sha256.replace_range(..1, replacement);
                assert_eq!(source.path, original.path);
                assert_eq!(source.bytes, original.bytes);
                assert_ne!(source.sha256, digest(&source.bytes));
                assert_eq!(read_regular(&source.path).unwrap(), original.bytes);
                assert_eq!(
                    run_data(seed, view, data, &mut reads, false),
                    Err(SourceError::SourceChanged(spec.unit))
                );
            }
        }
    }

    #[test]
    fn source_roster_and_consumed_bytes_refuse_drift() {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            let data = fixture(seed, view);
            let mut valid = replay(&data);
            assert_eq!(run_data(seed, view, &data, &mut valid, false), Ok(44));
            assert_coverage_controls(seed, view, &data);
            assert_hash_controls(seed, view, &data);
            for (index, spec) in source_roster().enumerate() {
                for phase in [Phase::Before, Phase::After] {
                    let mut reads = replay(&data);
                    let round = match phase {
                        Phase::Before => &mut reads.before,
                        Phase::After => &mut reads.after,
                    };
                    let source = round.sources.as_mut().unwrap().get_mut(index).unwrap();
                    assert_eq!(*source, data.sources[index]);
                    source.path = source.path.with_file_name("unmatched-source.rs");
                    assert_ne!(source.path, data.sources[index].path);
                    assert_eq!(
                        run_data(seed, view, &data, &mut reads, false),
                        Err(SourceError::SourcePath(spec.unit))
                    );
                }
                let mut drift = replay(&data);
                let changed = drift
                    .after
                    .sources
                    .as_mut()
                    .unwrap()
                    .get_mut(index)
                    .unwrap();
                assert_eq!(*changed, data.sources[index]);
                changed.bytes.push(b' ');
                changed.sha256 = digest(&changed.bytes);
                assert_ne!(changed.bytes, data.sources[index].bytes);
                assert_eq!(
                    changed.sha256,
                    digest(&changed.bytes),
                    "self-consistent rehash is not approval"
                );
                assert_eq!(
                    run_data(seed, view, &data, &mut drift, false),
                    Err(SourceError::SourceChanged(spec.unit))
                );
                for phase in [Phase::Before, Phase::After] {
                    let mut unreadable = replay(&data);
                    let round = match phase {
                        Phase::Before => &mut unreadable.before,
                        Phase::After => &mut unreadable.after,
                    };
                    round.sources = Err(spec.unit);
                    assert_eq!(
                        run_data(seed, view, &data, &mut unreadable, false),
                        Err(SourceError::SourceRead(spec.unit))
                    );
                }
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn interpretation_errors_still_close_the_source_window() {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            let data = fixture(seed, view);
            let mut positive = replay(&data);
            assert_eq!(run_data(seed, view, &data, &mut positive, false), Ok(44));
            assert_eq!(*positive.events.borrow(), expected_events());
            let mut refused = replay(&data);
            assert_eq!(
                run_data(seed, view, &data, &mut refused, true),
                Err(SourceError::Interpretation(ConsumerError::UnreviewedSource))
            );
            assert_eq!(*refused.events.borrow(), expected_events());
            for (index, spec) in source_roster().enumerate() {
                let mut drift = replay(&data);
                let changed = drift
                    .after
                    .sources
                    .as_mut()
                    .unwrap()
                    .get_mut(index)
                    .unwrap();
                assert_eq!(*changed, data.sources[index]);
                changed.bytes.push(b' ');
                changed.sha256 = digest(&changed.bytes);
                assert_eq!(
                    run_data(seed, view, &data, &mut drift, true),
                    Err(SourceError::SourceChanged(spec.unit))
                );
                assert_eq!(*drift.events.borrow(), expected_events());
                let mut unreadable = replay(&data);
                unreadable.after.sources = Err(spec.unit);
                assert_eq!(
                    run_data(seed, view, &data, &mut unreadable, true),
                    Err(SourceError::SourceRead(spec.unit))
                );
                assert_eq!(*unreadable.events.borrow(), expected_events());
            }
            let mut stale_compiler = replay(&data);
            stale_compiler
                .after
                .compiler
                .as_mut()
                .unwrap()
                .verbose
                .push_str("\nunmatched compiler identity\n");
            assert_eq!(
                run_data(seed, view, &data, &mut stale_compiler, true),
                Err(SourceError::CompilerBinding)
            );
            assert_eq!(*stale_compiler.events.borrow(), expected_events());
            let mut simultaneous = replay(&data);
            assert_eq!(simultaneous.after.compiler, Ok(data.expected.clone()));
            assert_eq!(simultaneous.after.sources, Ok(data.sources.clone()));
            simultaneous.after.sources = Err(SourceUnit::Nfa);
            simultaneous.after.compiler = Err(());
            assert_eq!(
                run_data(seed, view, &data, &mut simultaneous, true),
                Err(SourceError::CompilerBinding)
            );
            assert_eq!(*simultaneous.events.borrow(), expected_events());
            Ok(())
        })
        .unwrap();
    }
}
