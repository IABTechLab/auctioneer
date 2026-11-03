//! Offline `OpenRTB` catalogue generation and verification.
#![expect(
    clippy::print_stderr,
    reason = "operator-facing CLI errors and review-age warnings belong on stderr"
)]

use auctioneer_openrtb_catalogue::{
    Catalogue, SourceManifest, extract_catalogue, validate_source_digest,
};
#[cfg(test)]
use clap::Parser;
use clap::Subcommand;
use std::error::Error;
use std::fmt::Display;
use std::fs::{self, OpenOptions};
use std::io::{self, Write as _};
#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::process;
use std::str;
use std::sync::atomic::{AtomicU64, Ordering};
use time::macros::format_description;
use time::{Date, Duration, OffsetDateTime};

static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
#[derive(Debug, Parser)]
#[command(about, version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Generate deterministic membership from validated, pinned local inputs.
    Generate {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify pinned inputs and exact committed membership without writing.
    Check {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        catalogue: PathBuf,
    },
}

fn run(command: Command) -> Result<(), Box<dyn Error>> {
    match command {
        Command::Generate {
            manifest,
            source,
            output,
        } => {
            let catalogue = load_inputs(&manifest, &source)?;
            reject_input_alias(&output, &manifest, &source)?;
            let encoded = toml::to_string(&catalogue)?;
            write_atomic(&output, encoded.as_bytes())?;
        }
        Command::Check {
            manifest,
            source,
            catalogue,
        } => {
            let expected = load_inputs(&manifest, &source)?;
            let encoded = fs::read_to_string(&catalogue)
                .map_err(|error| path_error("read catalogue", &catalogue, error))?;
            let actual: Catalogue = toml::from_str(&encoded)
                .map_err(|error| path_error("parse catalogue", &catalogue, error))?;
            if actual != expected {
                return Err(io::Error::other(format!("catalogue {} differs from the pinned source; regenerate with generate --manifest {} --source {} --output {}",
                    catalogue.display(), manifest.display(), source.display(), catalogue.display())).into());
            }
        }
    }
    Ok(())
}

fn path_error<E: Display>(action: &str, path: &Path, error: E) -> io::Error {
    io::Error::other(format!("cannot {action} {}: {error}", path.display()))
}

fn load_inputs(manifest_path: &Path, source_path: &Path) -> Result<Catalogue, Box<dyn Error>> {
    load_inputs_at(manifest_path, source_path, OffsetDateTime::now_utc().date())
}

fn load_inputs_at(
    manifest_path: &Path,
    source_path: &Path,
    today: Date,
) -> Result<Catalogue, Box<dyn Error>> {
    let encoded = fs::read_to_string(manifest_path)
        .map_err(|error| path_error("read manifest", manifest_path, error))?;
    let manifest: SourceManifest = toml::from_str(&encoded)
        .map_err(|error| path_error("parse manifest", manifest_path, error))?;
    let bytes =
        fs::read(source_path).map_err(|error| path_error("read source", source_path, error))?;
    validate_source_digest(&manifest, &bytes)?;
    let source = str::from_utf8(&bytes)
        .map_err(|error| path_error("decode source as UTF-8", source_path, error))?;
    let catalogue = extract_catalogue(source)?;
    let review = Date::parse(
        &manifest.review_date,
        format_description!("[year]-[month]-[day]"),
    )?;
    if review_is_stale_at(review, today) {
        eprintln!(
            "auctioneer-openrtb-catalogue: warning: source review is over 180 days old ({}); membership remains usable",
            manifest.review_date
        );
    }
    Ok(catalogue)
}

fn review_is_stale_at(review: Date, today: Date) -> bool {
    review
        .checked_add(Duration::days(180_i64))
        .is_some_and(|deadline| today > deadline)
}

fn reject_input_alias(output: &Path, manifest: &Path, source: &Path) -> Result<(), Box<dyn Error>> {
    let resolved_output = match fs::canonicalize(output) {
        Ok(path) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let parent = output_parent(output);
            let name = output
                .file_name()
                .ok_or_else(|| io::Error::other("output requires a filename"))?;
            fs::canonicalize(parent)
                .map_err(|parent_error| {
                    path_error("resolve output directory", parent, parent_error)
                })?
                .join(name)
        }
        Err(error) => return Err(path_error("resolve output", output, error).into()),
    };
    for input in [manifest, source] {
        let resolved_input =
            fs::canonicalize(input).map_err(|error| path_error("resolve input", input, error))?;
        if resolved_output == resolved_input || is_hard_link(output, input)? {
            return Err(io::Error::other(format!(
                "output {} aliases input {}; source and review metadata must remain unchanged",
                output.display(),
                input.display()
            ))
            .into());
        }
    }
    Ok(())
}

#[cfg(unix)]
fn is_hard_link(output: &Path, input: &Path) -> io::Result<bool> {
    let output_metadata = match fs::metadata(output) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(path_error("read output identity", output, error)),
    };
    let input_metadata =
        fs::metadata(input).map_err(|error| path_error("read input identity", input, error))?;
    Ok(output_metadata.dev() == input_metadata.dev()
        && output_metadata.ino() == input_metadata.ino())
}

#[cfg(not(unix))]
fn is_hard_link(output: &Path, _input: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(output) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(path_error("read output identity", output, error)),
        Ok(_) => Err(io::Error::other(
            "cannot verify hard-link identity of an existing output on this host; use a new output path",
        )),
    }
}

fn output_parent(output: &Path) -> &Path {
    output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

fn write_atomic(output: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = output_parent(output);
    for _ in 0_u8..64_u8 {
        let temporary = parent.join(format!(
            ".auctioneer-openrtb-catalogue.{}.{}.tmp",
            process::id(),
            NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = match OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(path_error("create temporary catalogue", &temporary, error)),
        };
        let write_result = file
            .write_all(bytes)
            .map_err(|error| path_error("write temporary catalogue", &temporary, error))
            .and_then(|()| {
                file.sync_all()
                    .map_err(|error| path_error("sync temporary catalogue", &temporary, error))
            });
        drop(file);
        let replacement_result = write_result.and_then(|()| {
            fs::rename(&temporary, output)
                .map_err(|error| path_error("replace catalogue", output, error))
        });
        if let Err(error) = replacement_result {
            if let Err(cleanup) = fs::remove_file(&temporary) {
                return Err(io::Error::other(format!(
                    "{error}; cannot remove temporary catalogue {}: {cleanup}",
                    temporary.display()
                )));
            }
            return Err(error);
        }
        return Ok(());
    }
    Err(io::Error::other(
        "cannot reserve a temporary catalogue after 64 attempts",
    ))
}

pub(crate) fn dispatch(command: Command) -> i32 {
    match run(command) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("auctioneer-openrtb-catalogue: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, load_inputs_at, review_is_stale_at, run};
    use auctioneer_openrtb_catalogue::{Catalogue, SourceManifest};
    use clap::Parser as _;
    use sha2::{Digest as _, Sha256};
    use std::env;
    use std::error::Error;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::symlink;
    use std::panic::{self, AssertUnwindSafe};
    use std::path::{Path, PathBuf};
    use std::process;
    use std::str;
    use std::sync::atomic::{AtomicU64, Ordering};
    use time::Duration;
    use time::macros::date;

    const SOURCE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../auctioneer-config-data/data/openrtb-2.6-source.md"
    ));
    static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

    fn with_inputs<F: FnOnce(&Path)>(test: F) {
        let path = env::temp_dir().join(format!(
            "auctioneer-importer-{}-{}",
            process::id(),
            NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        let manifest = SourceManifest {
            url: "https://github.com/InteractiveAdvertisingBureau/openrtb2.x/blob/1debba8001f40eb14597102dec3205afddf811d4/2.6.md".to_owned(),
            release: "2.6-202606".to_owned(),
            commit: "1debba8001f40eb14597102dec3205afddf811d4".to_owned(),
            sha256: hex::encode(Sha256::digest(SOURCE.as_bytes())),
            reviewer: "CLI fixture reviewer, not source approval".to_owned(),
            review_date: "2026-10-09".to_owned(),
        };
        fs::write(path.join("source.md"), SOURCE).unwrap();
        fs::write(
            path.join("manifest.toml"),
            toml::to_string(&manifest).unwrap(),
        )
        .unwrap();
        let result = panic::catch_unwind(AssertUnwindSafe(|| test(&path)));
        fs::remove_dir_all(&path).unwrap();
        if let Err(panic) = result {
            panic::resume_unwind(panic);
        }
    }

    fn generate(path: &Path, output: PathBuf) -> Result<(), Box<dyn Error>> {
        run(Command::Generate {
            manifest: path.join("manifest.toml"),
            source: path.join("source.md"),
            output,
        })
    }

    fn check(path: &Path) -> Result<(), Box<dyn Error>> {
        run(Command::Check {
            manifest: path.join("manifest.toml"),
            source: path.join("source.md"),
            catalogue: path.join("catalogue.toml"),
        })
    }

    #[test]
    fn parses_explicit_generate_and_check_arguments() {
        for (mode, flag) in [("generate", "--output"), ("check", "--catalogue")] {
            Cli::try_parse_from([
                "auctioneer-openrtb-catalogue",
                mode,
                "--manifest",
                "review.toml",
                "--source",
                "source.md",
                flag,
                "catalogue.toml",
            ])
            .unwrap();
        }
    }

    #[test]
    fn rejects_missing_unknown_and_duplicate_cli_options() {
        for mode in ["generate", "check"] {
            let flag = if mode == "generate" {
                "--output"
            } else {
                "--catalogue"
            };
            let base = [
                "auctioneer-openrtb-catalogue",
                mode,
                "--manifest",
                "review.toml",
                "--source",
                "source.md",
                flag,
                "catalogue.toml",
            ];
            for duplicate in ["--manifest", "--source", flag, "--unknown"] {
                let mut args = base.to_vec();
                args.extend([duplicate, "extra"]);
                Cli::try_parse_from(args).unwrap_err();
            }
            for missing in ["--manifest", "--source", flag] {
                let args: Vec<&str> = base
                    .chunks(2)
                    .filter(|pair| pair.first().is_some_and(|name| *name != missing))
                    .flatten()
                    .copied()
                    .collect();
                Cli::try_parse_from(args).unwrap_err();
            }
        }
    }

    #[test]
    fn generates_twice_identically_and_checks_without_rewriting_inputs() {
        with_inputs(|path| {
            let before_manifest = fs::read(path.join("manifest.toml")).unwrap();
            let before_source = fs::read(path.join("source.md")).unwrap();
            generate(path, path.join("catalogue.toml")).unwrap();
            generate(path, path.join("second.toml")).unwrap();
            let bytes = fs::read(path.join("catalogue.toml")).unwrap();
            assert_eq!(bytes, fs::read(path.join("second.toml")).unwrap());
            check(path).unwrap();
            assert_eq!(bytes, fs::read(path.join("catalogue.toml")).unwrap());
            assert_eq!(
                before_manifest,
                fs::read(path.join("manifest.toml")).unwrap()
            );
            assert_eq!(before_source, fs::read(path.join("source.md")).unwrap());
            let encoded = str::from_utf8(&bytes).unwrap();
            let catalogue: Catalogue = toml::from_str(encoded).unwrap();
            assert_eq!(catalogue.fields.len(), 79);
            for forbidden in [
                "reviewer",
                "review_date",
                "generated_at",
                "sha256",
                "class",
                "domains",
            ] {
                assert!(!encoded.contains(forbidden), "{forbidden}");
            }
        });
    }

    #[test]
    fn generate_and_check_reject_source_mutation_and_invalid_digest() {
        with_inputs(|path| {
            generate(path, path.join("catalogue.toml")).unwrap();
            let original = fs::read(path.join("catalogue.toml")).unwrap();
            fs::write(path.join("source.md"), format!("{SOURCE}\n")).unwrap();
            assert!(check(path).unwrap_err().to_string().contains("digest"));
            generate(path, path.join("catalogue.toml")).unwrap_err();
            assert_eq!(original, fs::read(path.join("catalogue.toml")).unwrap());
            fs::write(path.join("source.md"), SOURCE).unwrap();
            let encoded = fs::read_to_string(path.join("manifest.toml")).unwrap();
            let mut manifest: SourceManifest = toml::from_str(&encoded).unwrap();
            manifest.sha256 = "bad digest".to_owned();
            fs::write(
                path.join("manifest.toml"),
                toml::to_string(&manifest).unwrap(),
            )
            .unwrap();
            check(path).unwrap_err();
            generate(path, path.join("absent.toml")).unwrap_err();
            assert!(!path.join("absent.toml").exists());
        });
    }

    #[test]
    fn check_rejects_mutated_duplicate_missing_and_extra_catalogue_rows() {
        with_inputs(|path| {
            generate(path, path.join("catalogue.toml")).unwrap();
            let encoded = fs::read_to_string(path.join("catalogue.toml")).unwrap();
            let original: Catalogue = toml::from_str(&encoded).unwrap();
            let mut mutations = Vec::new();
            for key in ["object", "field", "type", "section"] {
                let mut value = toml::Value::try_from(&original).unwrap();
                value["fields"][0][key] = toml::Value::String("wrong".to_owned());
                mutations.push(toml::to_string(&value).unwrap());
            }
            let mut duplicate = original.clone();
            duplicate
                .fields
                .push(duplicate.fields.first().unwrap().clone());
            mutations.push(toml::to_string(&duplicate).unwrap());
            let mut missing = original.clone();
            missing.fields.pop().unwrap();
            mutations.push(toml::to_string(&missing).unwrap());
            let mut extra = original.clone();
            let mut invented = extra.fields.first().unwrap().clone();
            invented.field = "ifv".to_owned();
            extra.fields.push(invented);
            mutations.push(toml::to_string(&extra).unwrap());
            mutations.push(encoded.replace("[[fields]]", "[[fields]]\nclass = 'context'"));
            mutations.push("fields = []\nreviewer = 'invented'\n".to_owned());
            for mutated in mutations {
                fs::write(path.join("catalogue.toml"), &mutated).unwrap();
                check(path).unwrap_err();
                assert_eq!(
                    mutated,
                    fs::read_to_string(path.join("catalogue.toml")).unwrap()
                );
            }
        });
    }

    #[test]
    fn generate_rejects_output_aliasing_human_metadata_or_source() {
        with_inputs(|path| {
            for name in ["manifest.toml", "source.md"] {
                let target = path.join(name);
                let original = fs::read(&target).unwrap();
                generate(path, target.clone()).unwrap_err();
                assert_eq!(original, fs::read(&target).unwrap());
            }
        });
    }

    #[cfg(unix)]
    #[test]
    fn generate_rejects_symlink_and_hardlink_aliases_without_input_changes() {
        with_inputs(|path| {
            for name in ["manifest.toml", "source.md"] {
                let input = path.join(name);
                let original = fs::read(&input).unwrap();
                let symlink_path = path.join(format!("{name}.symlink"));
                symlink(&input, &symlink_path).unwrap();
                assert!(
                    generate(path, symlink_path)
                        .unwrap_err()
                        .to_string()
                        .contains("aliases input")
                );
                let hardlink = path.join(format!("{name}.hardlink"));
                fs::hard_link(&input, &hardlink).unwrap();
                assert!(
                    generate(path, hardlink)
                        .unwrap_err()
                        .to_string()
                        .contains("aliases input")
                );
                let indirect = path.join(".").join("source.md");
                generate(path, indirect).unwrap_err();
                assert_eq!(original, fs::read(&input).unwrap());
            }
        });
    }

    #[cfg(unix)]
    #[test]
    fn failed_output_replacement_cleans_temporary_files() {
        with_inputs(|path| {
            let directory = path.join("catalogue-directory");
            fs::create_dir_all(&directory).unwrap();
            let error = generate(path, directory.clone()).unwrap_err().to_string();
            assert!(error.contains("replace catalogue"), "{error}");
            assert!(directory.is_dir());
            assert!(!fs::read_dir(path).unwrap().any(|entry| {
                entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with(".auctioneer-openrtb-catalogue.")
            }));
        });
    }

    #[test]
    fn review_age_warns_only_after_180_days() {
        let review = date!(2026 - 10 - 09);
        for (days, expected) in [(179_i64, false), (180_i64, false), (181_i64, true)] {
            let today = review.checked_add(Duration::days(days)).unwrap();
            assert_eq!(review_is_stale_at(review, today), expected, "{days} days");
        }
    }

    #[test]
    fn future_review_date_is_not_stale() {
        assert!(!review_is_stale_at(
            date!(2026 - 10 - 09),
            date!(2026 - 10 - 08)
        ));
    }

    #[test]
    fn different_actual_dates_preserve_catalogue_and_review_metadata() {
        with_inputs(|path| {
            let manifest_path = path.join("manifest.toml");
            let source_path = path.join("source.md");
            let original_manifest = fs::read(&manifest_path).unwrap();
            let original_source = fs::read(&source_path).unwrap();
            let recent =
                load_inputs_at(&manifest_path, &source_path, date!(2026 - 10 - 10)).unwrap();
            let stale =
                load_inputs_at(&manifest_path, &source_path, date!(2100 - 01 - 01)).unwrap();
            assert_eq!(recent, stale);
            assert_eq!(
                toml::to_string(&recent).unwrap(),
                toml::to_string(&stale).unwrap()
            );
            assert_eq!(recent.fields.len(), 79);
            assert_eq!(original_manifest, fs::read(&manifest_path).unwrap());
            assert_eq!(original_source, fs::read(&source_path).unwrap());
        });
    }

    #[test]
    fn reports_actionable_missing_input_errors() {
        with_inputs(|path| {
            fs::remove_file(path.join("manifest.toml")).unwrap();
            let error = check(path).unwrap_err().to_string();
            assert!(
                error.contains("read manifest") && error.contains("manifest.toml"),
                "{error}"
            );
        });
    }
}
