//! Tests-first current-source consumption window, not semantic/cost authority.
//!
//! The sealed seed/view supply the expected compiler identity. Probe results,
//! paths, bytes and digests are data, not permission to approve changed bodies.
//! The fixed roster below is the single actual-source roster for the primitive
//! adapter; its existing supplied-facts collector is not an authority producer.
//! This window does not discharge builtin resolution, helper recipes, resource
//! bounds, target facts, cleanup or qualification. No serving API is introduced.

use crate::schema_build::application::CapturedSeed;
use crate::schema_residency::descriptor::ValidationView;
use std::path::PathBuf;

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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceRoot {
    Workspace,
    RustLibrary,
}

struct SourceSpec {
    unit: SourceUnit,
    root: SourceRoot,
    relative: &'static str,
}

// These are source identities, not workload limits or a discovered owner census.
const ROSTER: [SourceSpec; 12] = [
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
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompilerData {
    executable: PathBuf,
    verbose: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceData {
    // Untrusted labels deliberately allow unknown/extra records in controls.
    unit: String,
    path: PathBuf,
    bytes: Vec<u8>,
    sha256: String,
}

struct SourceInputs<'a> {
    compiler: &'a CompilerData,
    sources: &'a [SourceData],
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
    MissingSourceProof,
    Binding,
    CompilerBinding,
    SourceCoverage,
    SourcePath(SourceUnit),
    SourceRead(SourceUnit),
    SourceChanged(SourceUnit),
    Interpretation(E),
}

// Both entrypoints remain explicit refusal stubs until executed hosted RED.
// Later GREEN must use probe_compiler/read_regular, not another loader/build.
// The actual facade must delegate to the same kernel below with real I/O.
fn with_current_sources<T, E>(
    _seed: &CapturedSeed,
    _view: ValidationView<'_>,
    _interpret: impl FnOnce(&SourceInputs<'_>) -> Result<T, E>,
) -> Result<T, SourceError<E>> {
    Err(SourceError::MissingSourceProof)
}

// The observer reports completed real seed/view/runtime checks; it supplies no
// approval or replacement recheck result. Returned interpretation errors must
// still close the window. Integrity refusals take precedence over that error.
fn with_source_data<T, E>(
    _seed: &CapturedSeed,
    _view: ValidationView<'_>,
    _reads: &mut impl SourceReads,
    _observe_check: impl FnMut(Phase),
    _interpret: impl FnOnce(&SourceInputs<'_>) -> Result<T, E>,
) -> Result<T, SourceError<E>> {
    Err(SourceError::MissingSourceProof)
}

#[cfg(test)]
mod tests {
    use super::{
        CompilerData, Phase, ROSTER, SourceData, SourceError, SourceInputs, SourceReads,
        SourceRoot, SourceUnit, with_current_sources, with_source_data,
    };
    use crate::schema_build::{application::CapturedSeed, selected::probe_compiler};
    use crate::schema_residency::{
        descriptor::ValidationView, read_regular, with_current_descriptor,
    };
    use crate::schema_validation::handoff;
    use serde_json::Value;
    use sha2::{Digest as _, Sha256};
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Seek as _, Write as _};
    use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
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
        let sources = ROSTER
            .iter()
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
        for (source, spec) in inputs.sources.iter().zip(&ROSTER) {
            assert_eq!(source.unit, format!("{:?}", spec.unit));
            assert!(source.path.is_absolute());
            assert!(source.path.ends_with(spec.relative));
            assert_eq!(source.sha256, digest(&source.bytes));
        }
        inputs.sources.len()
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

    fn assert_roster() {
        let independent = [
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
        ];
        assert_eq!(
            ROSTER
                .iter()
                .map(|spec| (spec.unit, spec.root, spec.relative))
                .collect::<Vec<_>>(),
            independent
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
            assert!(metadata.file_type().is_file());
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
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            let data = fixture(seed, view);
            assert_roster();
            let actual = with_current_sources(seed, view, |inputs| {
                Ok::<_, ConsumerError>(inspect_inputs(inputs, &data))
            });
            assert_eq!(actual, Ok(12));
            let mut reads = replay(&data);
            assert_eq!(run_data(seed, view, &data, &mut reads, false), Ok(12));
            assert_eq!(*reads.events.borrow(), expected_events());
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
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn compiler_identity_must_match_the_sealed_seed() {
        with_current_descriptor(|seed, descriptor| {
            let view = descriptor.validation_view().unwrap();
            let data = fixture(seed, view);
            let mut valid = replay(&data);
            assert_eq!(run_data(seed, view, &data, &mut valid, false), Ok(12));
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
        for (index, spec) in ROSTER.iter().enumerate() {
            for phase in [Phase::Before, Phase::After] {
                let mut reads = replay(data);
                let round = match phase {
                    Phase::Before => &mut reads.before,
                    Phase::After => &mut reads.after,
                };
                let source = round.sources.as_mut().unwrap().get_mut(index).unwrap();
                let original = data.sources.get(index).unwrap();
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
            assert_eq!(run_data(seed, view, &data, &mut valid, false), Ok(12));
            assert_coverage_controls(seed, view, &data);
            assert_hash_controls(seed, view, &data);
            for (index, spec) in ROSTER.iter().enumerate() {
                for phase in [Phase::Before, Phase::After] {
                    let mut reads = replay(&data);
                    let round = match phase {
                        Phase::Before => &mut reads.before,
                        Phase::After => &mut reads.after,
                    };
                    let source = round.sources.as_mut().unwrap().get_mut(index).unwrap();
                    source.path = source.path.with_file_name("unmatched-source.rs");
                    assert_ne!(source.path, data.sources.get(index).unwrap().path);
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
                changed.bytes.push(b' ');
                changed.sha256 = digest(&changed.bytes);
                assert_ne!(changed.bytes, data.sources.get(index).unwrap().bytes);
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
            assert_eq!(run_data(seed, view, &data, &mut positive, false), Ok(12));
            assert_eq!(*positive.events.borrow(), expected_events());
            let mut refused = replay(&data);
            assert_eq!(
                run_data(seed, view, &data, &mut refused, true),
                Err(SourceError::Interpretation(ConsumerError::UnreviewedSource))
            );
            assert_eq!(*refused.events.borrow(), expected_events());
            for (index, spec) in ROSTER.iter().enumerate() {
                let mut drift = replay(&data);
                let changed = drift
                    .after
                    .sources
                    .as_mut()
                    .unwrap()
                    .get_mut(index)
                    .unwrap();
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
            Ok(())
        })
        .unwrap();
    }
}
