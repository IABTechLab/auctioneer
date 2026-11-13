//! Native matcher comparison only; no serving policy or resource certificate.

use regex_automata::{Input, MatchKind, meta, nfa::thompson::pikevm, util::syntax};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, OpenOptions};
use std::hint::black_box;
use std::io::Write as _;
use std::mem::size_of;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::schema_cache_bounds::{emitted_patterns, forward_nfa};
use crate::schema_meter::measure;
use crate::schema_qualification::Draft;
use crate::{schema_profile, schema_reference};

struct Engines {
    meta: meta::Regex,
    pike: pikevm::PikeVM,
}

impl Engines {
    fn new(pattern: &str) -> Self {
        Self {
            meta: build_meta(pattern),
            pike: build_pike(pattern),
        }
    }

    fn matches(&self, input: &str) -> (bool, bool) {
        (
            search_meta(&self.meta, &mut self.meta.create_cache(), input),
            search_pike(&self.pike, &mut self.pike.create_cache(), input),
        )
    }
}

fn build_meta(pattern: &str) -> meta::Regex {
    meta::Regex::builder()
        .configure(
            meta::Regex::config()
                .match_kind(MatchKind::LeftmostFirst)
                .utf8_empty(true)
                .nfa_size_limit(Some(0x0001_0000))
                .hybrid_cache_capacity(0x0001_0000)
                .pool_capacity(1),
        )
        .syntax(syntax::Config::default().utf8(true))
        .build(pattern)
        .unwrap()
}

fn build_pike(pattern: &str) -> pikevm::PikeVM {
    pikevm::PikeVM::builder()
        .configure(pikevm::PikeVM::config().match_kind(MatchKind::LeftmostFirst))
        .build_from_nfa(forward_nfa(pattern).unwrap())
        .unwrap()
}

fn search_meta(program: &meta::Regex, cache: &mut meta::Cache, input: &str) -> bool {
    program
        .search_half_with(cache, &Input::new(input).earliest(true))
        .is_some()
}

fn search_pike(program: &pikevm::PikeVM, cache: &mut pikevm::Cache, input: &str) -> bool {
    program.is_match(cache, Input::new(input))
}

fn iterations(bytes: usize) -> usize {
    0x0001_0000_usize
        .checked_div(bytes.max(64))
        .unwrap()
        .clamp(64, 1024)
}

struct Pattern {
    key: String,
    text: String,
    hostname: bool,
}

impl Pattern {
    fn allowed(&self, input: &str) -> bool {
        !self.hostname || input.len() < 256
    }
}

struct Catalog {
    patterns: Vec<Pattern>,
    candidates: BTreeMap<String, BTreeSet<String>>,
    metadata: Value,
}

struct Subject {
    label: String,
    text: String,
    origins: Vec<String>,
    expected: bool,
}

fn selected_subjects(
    pattern: &Pattern,
    candidates: &BTreeMap<String, BTreeSet<String>>,
) -> (Vec<Subject>, Value) {
    let engines = Engines::new(&pattern.text);
    let mut meta = engines.meta.create_cache();
    let mut pike = engines.pike.create_cache();
    let mut positive = 0_usize;
    let mut negative = 0_usize;
    let mut skipped = 0_usize;
    let mut result = Vec::new();
    for (text, origins) in candidates {
        if !pattern.allowed(text) {
            skipped = skipped.checked_add(1).unwrap();
            continue;
        }
        let expected = search_meta(&engines.meta, &mut meta, text);
        assert_eq!(
            search_pike(&engines.pike, &mut pike, text),
            expected,
            "{}: {text:?}",
            pattern.key
        );
        let count = if expected {
            &mut positive
        } else {
            &mut negative
        };
        if *count < 3 {
            result.push(Subject {
                label: format!(
                    "capture-{}-{}",
                    if expected { "positive" } else { "negative" },
                    *count
                ),
                text: text.clone(),
                origins: origins.iter().cloned().collect(),
                expected,
            });
        }
        *count = count.checked_add(1).unwrap();
    }
    for mut subject in panel(pattern.hostname) {
        let admitted = pattern.allowed(&subject.text);
        let meta_result = admitted && search_meta(&engines.meta, &mut meta, &subject.text);
        let pike_result = admitted && search_pike(&engines.pike, &mut pike, &subject.text);
        assert_eq!(
            meta_result, pike_result,
            "{}: {}",
            pattern.key, subject.label
        );
        subject.expected = meta_result;
        result.push(subject);
    }
    assert_eq!(
        positive
            .checked_add(negative)
            .and_then(|n| n.checked_add(skipped))
            .unwrap(),
        candidates.len()
    );
    let counts = json!({"candidate_subjects":candidates.len(),"positive":positive,"negative":negative,
        "guard_skips":skipped,"positive_witness_missing":positive==0,
        "timing_selection":"first three admitted positives and negatives in lexical order, plus fixed panel"});
    (result, counts)
}

fn panel(hostname: bool) -> Vec<Subject> {
    let mut values: Vec<(String, String)> = [
        ("empty", ""),
        ("short-ascii", "abc123"),
        ("digits", "123"),
        ("unicode", "\u{0661}\u{00e9}"),
        ("multiline", "a\n123"),
        ("punctuation", "_-.:"),
        ("nul", "a\0b"),
    ]
    .into_iter()
    .map(|(name, text)| (name.to_owned(), text.to_owned()))
    .collect();
    for bytes in if hostname {
        [64_usize, 255, 256]
    } else {
        [64_usize, 1024, 0x0001_0000]
    } {
        values.extend([
            (format!("repeat-a-{bytes}"), "a".repeat(bytes)),
            (format!("repeat-digit-{bytes}"), "9".repeat(bytes)),
            (
                format!("alternating-{bytes}"),
                "a1".repeat(bytes.checked_div(2).unwrap()),
            ),
            (
                format!("late-invalid-{bytes}"),
                format!("{}!", "9".repeat(bytes.checked_sub(1).unwrap())),
            ),
        ]);
    }
    if hostname {
        values.extend([
            ("short-host".to_owned(), "a.example".to_owned()),
            (
                "max-host".to_owned(),
                [63_usize; 4].map(|n| "a".repeat(n)).join("."),
            ),
            (
                "guarded-host".to_owned(),
                [63_usize, 63, 63, 62, 1].map(|n| "a".repeat(n)).join("."),
            ),
            (
                "overlong-label".to_owned(),
                format!("{}.co", "a".repeat(64)),
            ),
        ]);
    }
    values
        .into_iter()
        .map(|(label, text)| Subject {
            label,
            text,
            origins: vec![],
            expected: false,
        })
        .collect()
}

fn engine_order(round: usize) -> [usize; 2] {
    if round.is_multiple_of(2) {
        [0, 1]
    } else {
        [1, 0]
    }
}

fn measured_pattern(
    pattern: &Pattern,
    subjects: &[Subject],
    deadline: Instant,
) -> Result<Value, &'static str> {
    check_deadline(deadline)?;
    let engines = Engines::new(&pattern.text);
    let memory = json!({
        "meta": memory_report(pattern, subjects, || build_meta(&pattern.text), meta::Regex::create_cache, search_meta),
        "pikevm": memory_report(pattern, subjects, || build_pike(&pattern.text), pikevm::PikeVM::create_cache, search_pike)
    });
    let mut program_ns = [[0_u64; 2]; 7];
    for (round, samples) in program_ns.iter_mut().enumerate() {
        check_deadline(deadline)?;
        for engine in engine_order(round) {
            samples[engine] = if engine == 0 {
                time_construction(|| build_meta(&pattern.text))
            } else {
                time_construction(|| build_pike(&pattern.text))
            };
        }
    }
    let mut searches = Vec::with_capacity(subjects.len());
    for subject in subjects {
        check_deadline(deadline)?;
        searches.push(json!({
            "id":subject.label,"input_bytes":subject.text.len(),"input_sha256":digest(subject.text.as_bytes()),
            "origins":subject.origins,"expected":subject.expected,"guard_skipped":!pattern.allowed(&subject.text),
            "iterations":iterations(subject.text.len()),
            "timing":paired_timings(pattern,&engines,subject,deadline)?
        }));
    }
    check_deadline(deadline)?;
    Ok(json!({
        "key":pattern.key,"pattern":pattern.text,"pattern_sha256":digest(pattern.text.as_bytes()),
        "hostname":pattern.hostname,"nfa_states":engines.pike.get_nfa().states().len(),
        "capture_slots":engines.pike.get_nfa().group_info().slot_len(),
        "memory":memory,"program_construction_ns":{"meta":program_ns.map(|samples| samples[0]),"pikevm":program_ns.map(|samples| samples[1])},
        "searches":searches
    }))
}

fn time_construction<R>(make: impl FnOnce() -> R) -> u64 {
    let start = Instant::now();
    let value = black_box(make());
    let nanos = u64::try_from(start.elapsed().as_nanos()).unwrap();
    drop(value);
    nanos
}

fn fresh<R, C>(
    pattern: &Pattern,
    program: &R,
    input: &str,
    cache: impl Fn(&R) -> C,
    search: impl Fn(&R, &mut C, &str) -> bool,
) -> bool {
    if !pattern.allowed(input) {
        return false;
    }
    search(program, &mut cache(program), input)
}

fn history<R, C>(
    pattern: &Pattern,
    program: &R,
    subjects: &[Subject],
    cache: impl Fn(&R) -> C,
    search: impl Fn(&R, &mut C, &str) -> bool,
) -> usize {
    let mut owner = cache(program);
    let mut matches = 0_usize;
    for subject in subjects {
        for _ in 0_usize..2 {
            matches = matches
                .checked_add(usize::from(
                    pattern.allowed(&subject.text)
                        && search(program, &mut owner, black_box(&subject.text)),
                ))
                .unwrap();
        }
    }
    matches
}

fn memory_report<R, C>(
    pattern: &Pattern,
    subjects: &[Subject],
    make: impl Fn() -> R,
    cache: impl Fn(&R) -> C + Copy,
    search: impl Fn(&R, &mut C, &str) -> bool + Copy,
) -> Value {
    let expected = subjects
        .iter()
        .filter(|row| row.expected)
        .count()
        .checked_mul(2)
        .unwrap();
    let ((), program_lifetime) = measure(|| drop(black_box(make())));
    let (full_matches, program_and_cache) = measure(|| {
        let program = make();
        history(pattern, &program, subjects, cache, search)
    });
    assert_eq!(full_matches, expected);
    let program = make();
    let (cache_matches, cache_lifetime) =
        measure(|| history(pattern, &program, subjects, cache, search));
    assert_eq!(cache_matches, expected);
    for metric in [program_lifetime, program_and_cache, cache_lifetime] {
        assert_eq!(
            metric.json()["retained_bytes"],
            0_i64,
            "complete owner lifetime leaked requested heap"
        );
    }
    let mut owner = cache(&program);
    let mut phases = Vec::with_capacity(subjects.len());
    for (index, subject) in subjects.iter().enumerate() {
        let invoke = |state: &mut C| {
            pattern.allowed(&subject.text) && search(&program, state, black_box(&subject.text))
        };
        let (first, first_metrics) = measure(|| invoke(&mut owner));
        let (repeat, repeat_metrics) = measure(|| invoke(&mut owner));
        assert_eq!((first, repeat), (subject.expected, subject.expected));
        let fresh_metrics = if pattern.hostname {
            let (result, metrics) =
                measure(|| fresh(pattern, &program, black_box(&subject.text), cache, search));
            assert_eq!(result, subject.expected);
            assert_eq!(metrics.json()["retained_bytes"], 0_i64);
            if !pattern.allowed(&subject.text) {
                assert_eq!(metrics.json()["allocation_calls"], 0_u64);
            }
            metrics.json()
        } else {
            Value::Null
        };
        phases.push(
            json!({"id":subject.label,"phase":if index==0 {"first"} else {"changed"},
            "incremental_search":first_metrics.json(),"incremental_repeat":repeat_metrics.json(),
            "fresh_hostname_call":fresh_metrics}),
        );
    }
    json!({"program_lifetime":program_lifetime.json(),"cache_history_lifetime":cache_lifetime.json(),
        "program_and_cache_lifetime":program_and_cache.json(),"incremental_phases":phases,
        "program_inline_bytes":size_of::<R>(),"cache_inline_bytes":size_of::<C>()})
}

#[derive(Default)]
struct Timing {
    cache: [u64; 7],
    first: [u64; 7],
    warm: [u64; 7],
    fresh: [u64; 7],
}

impl Timing {
    fn json(&self, admitted: bool, hostname: bool) -> Value {
        json!({"cache_construction_ns":if admitted {json!(self.cache)} else {Value::Null},
            "first_search_ns":if admitted {json!(self.first)} else {Value::Null},
            "warm_batch_ns":if admitted {json!(self.warm)} else {Value::Null},
            "fresh_hostname_batch_ns":if hostname {json!(self.fresh)} else {Value::Null}})
    }
}

fn batch(count: usize, expected: bool, mut search: impl FnMut() -> bool) -> u64 {
    let start = Instant::now();
    let matched = (0..count).filter(|_| black_box(search())).count();
    let nanos = u64::try_from(start.elapsed().as_nanos()).unwrap();
    assert_eq!(matched, if expected { count } else { 0 });
    nanos
}

fn timed_sample<R, C>(
    pattern: &Pattern,
    program: &R,
    subject: &Subject,
    cache: impl Fn(&R) -> C + Copy,
    search: impl Fn(&R, &mut C, &str) -> bool + Copy,
    timing: &mut Timing,
    round: usize,
) {
    let count = iterations(subject.text.len());
    if pattern.allowed(&subject.text) {
        let start = Instant::now();
        let mut owner = black_box(cache(program));
        timing.cache[round] = u64::try_from(start.elapsed().as_nanos()).unwrap();
        timing.first[round] = batch(1, subject.expected, || {
            search(program, &mut owner, black_box(&subject.text))
        });
        timing.warm[round] = batch(count, subject.expected, || {
            search(program, &mut owner, black_box(&subject.text))
        });
    }
    if pattern.hostname {
        timing.fresh[round] = batch(count, subject.expected, || {
            fresh(pattern, program, black_box(&subject.text), cache, search)
        });
    }
}

fn paired_timings(
    pattern: &Pattern,
    engines: &Engines,
    subject: &Subject,
    deadline: Instant,
) -> Result<Value, &'static str> {
    let mut meta = Timing::default();
    let mut pike = Timing::default();
    for round in 0..7 {
        check_deadline(deadline)?;
        for engine in engine_order(round) {
            if engine == 0 {
                timed_sample(
                    pattern,
                    &engines.meta,
                    subject,
                    meta::Regex::create_cache,
                    search_meta,
                    &mut meta,
                    round,
                );
            } else {
                timed_sample(
                    pattern,
                    &engines.pike,
                    subject,
                    pikevm::PikeVM::create_cache,
                    search_pike,
                    &mut pike,
                    round,
                );
            }
        }
    }
    let admitted = pattern.allowed(&subject.text);
    Ok(
        json!({"meta":meta.json(admitted,pattern.hostname),"pikevm":pike.json(admitted,pattern.hostname)}),
    )
}

fn collect_catalog() -> Catalog {
    let snapshot = schema_profile::inputs().snapshot;
    assert_eq!(snapshot.schemas.len(), 261);
    let mut patterns = Vec::new();
    for schema in &snapshot.schemas {
        let draft = match schema.audit.draft {
            Draft::Draft4 => quote::quote!(Draft4),
            Draft::Draft202012 => quote::quote!(Draft202012),
            Draft::Unspecified | Draft::Other(_) => panic!("unsupported source draft"),
        };
        let source_path = snapshot.root.join(&schema.witness.path);
        let path = source_path.to_str().unwrap();
        let options = quote::quote! {
            path = #path, draft = #draft, validate_formats = true,
            ignore_unknown_formats = false,
            formats = { "uuid" => crate::reference_uuid_format, "hostname" => crate::reference_hostname_format },
            pattern_options = { engine = regex, cache = explicit, size_limit = 65_536, dfa_size_limit = 65_536 },
            methods = { is_valid = true, validate = false, iter_errors = false }
        };
        let generated = syn::parse2(jsonschema_macros_core::expand(
            options,
            quote::quote!(
                pub struct MatcherComparison;
            ),
        ))
        .unwrap();
        for (helper, text) in emitted_patterns(&generated).unwrap() {
            patterns.push(Pattern {
                key: format!("{}:{helper}", schema.witness.name),
                text,
                hostname: false,
            });
        }
    }
    let keys: BTreeSet<_> = patterns.iter().map(|pattern| pattern.key.clone()).collect();
    assert_eq!(keys.len(), 23);
    assert_eq!(keys.len(), patterns.len());
    assert_eq!(keys, schema_profile::helpers().keys().cloned().collect());
    patterns.push(Pattern {
        key: "hostname-format".to_owned(),
        text: hostname_source(),
        hostname: true,
    });

    let capture = schema_reference::captured().unwrap();
    assert_eq!(capture.cases.len(), 300);
    let mut candidates = BTreeMap::new();
    let mut parsed = 0_usize;
    let mut rejected_json = 0_usize;
    let mut occurrences = 0_usize;
    for case in &capture.cases {
        if let Ok(value) = serde_json::from_str::<Value>(&case.input) {
            let origin = format!("{}:{:?}:{}", case.schema, case.group, case.index);
            occurrences = occurrences
                .checked_add(gather(&value, &origin, &mut candidates))
                .unwrap();
            parsed = parsed.checked_add(1).unwrap();
        } else {
            assert!(!case.valid);
            rejected_json = rejected_json.checked_add(1).unwrap();
        }
    }
    assert_eq!(parsed.checked_add(rejected_json).unwrap(), 300);
    assert!(!candidates.is_empty());
    assert!(candidates.keys().all(|text| text.len() <= 0x0001_0000));
    let metadata = json!({
        "source_revision": snapshot.revision, "schema_count": snapshot.schemas.len(),
        "schema_bytes": snapshot.schema_bytes, "capture_cases": capture.cases.len(),
        "parseable_capture_cases": parsed, "rejected_json_cases": rejected_json,
        "string_and_key_occurrences": occurrences, "distinct_candidate_subjects": candidates.len(),
        "inventory_sha256": digest(include_bytes!("../conformance/fixtures/reference/catalog/source-inventory.toml")),
        "capture_sha256": digest(include_bytes!("../conformance/fixtures/reference/schema-reference/parameters.json")),
        "candidates_sha256": digest(&serde_json::to_vec(&candidates).unwrap()),
        "candidate_scope": "deduplicated strings and keys from canonical parsing; not observed helper invocations"
    });
    Catalog {
        patterns,
        candidates,
        metadata,
    }
}

fn hostname_source() -> String {
    let formats = syn::parse_file(include_str!("../schema_formats/mod.rs")).unwrap();
    let hostnames: Vec<_> = formats
        .items
        .iter()
        .filter_map(|item| {
            let syn::Item::Const(declaration) = item else {
                return None;
            };
            if declaration.ident != "HOSTNAME_SOURCE" {
                return None;
            }
            let syn::Expr::Lit(literal) = declaration.expr.as_ref() else {
                panic!("hostname must remain literal");
            };
            let syn::Lit::Str(pattern) = &literal.lit else {
                panic!("hostname must remain a string");
            };
            Some(pattern.value())
        })
        .collect();
    assert_eq!(hostnames.len(), 1);
    hostnames.into_iter().next().unwrap()
}

fn gather(value: &Value, origin: &str, output: &mut BTreeMap<String, BTreeSet<String>>) -> usize {
    match value {
        Value::String(text) => {
            output
                .entry(text.clone())
                .or_default()
                .insert(format!("{origin}:string"));
            1
        }
        Value::Array(values) => values
            .iter()
            .map(|child| gather(child, origin, output))
            .sum(),
        Value::Object(fields) => fields
            .iter()
            .map(|(key, child)| {
                output
                    .entry(key.clone())
                    .or_default()
                    .insert(format!("{origin}:key"));
                gather(child, origin, output).checked_add(1).unwrap()
            })
            .sum(),
        Value::Null | Value::Bool(_) | Value::Number(_) => 0,
    }
}

fn digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn check_deadline(deadline: Instant) -> Result<(), &'static str> {
    if Instant::now() >= deadline {
        Err("comparison deadline expired")
    } else {
        Ok(())
    }
}

fn metadata() -> Value {
    use crate::schema_build::{resolved_engine_features, schema_regex_vendor};

    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let rustc = Command::new(env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("-vV")
        .output()
        .unwrap();
    assert!(rustc.status.success());
    let revision = Command::new("git")
        .current_dir(&root)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(revision.status.success());
    let lock: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("Cargo.lock")).unwrap()).unwrap();
    let packages = lock["package"].as_array().unwrap();
    let engine = packages
        .iter()
        .find(|row| row["name"].as_str() == Some("regex-automata"))
        .unwrap();
    assert_eq!(engine["version"].as_str(), Some("0.4.18"));
    assert!(engine.get("source").is_none());
    assert!(engine.get("checksum").is_none());
    let source = schema_regex_vendor::verify(&root).unwrap();
    let features = resolved_engine_features(&source).unwrap();
    let mut engine_identity = serde_json::to_value(source).unwrap();
    engine_identity
        .as_object_mut()
        .unwrap()
        .insert("features".into(), json!(features));
    let mut sources = BTreeMap::new();
    for relative in [
        "Cargo.toml",
        "Cargo.lock",
        "crates/auctioneer-bidder/tests/schema_matchers/mod.rs",
        "crates/auctioneer-bidder/tests/schema_meter/mod.rs",
        "crates/auctioneer-bidder/tests/schema_formats/mod.rs",
        "crates/auctioneer-bidder/tests/schema_cache_model/mod.rs",
        "crates/auctioneer-bidder/tests/schema_reference/mod.rs",
        "crates/auctioneer-bidder/tests/schema_profile/mod.rs",
        "crates/auctioneer-bidder/tests/schema_qualification/mod.rs",
        "vendor/jsonschema-macros-core/Cargo.toml",
        "vendor/jsonschema-macros-core/src/codegen/emit_root.rs",
    ] {
        sources.insert(relative, digest(&fs::read(root.join(relative)).unwrap()));
    }
    let binary = fs::read(env::current_exe().unwrap()).unwrap();
    json!({"scope":"native matcher primitives; no production engine selection or resource qualification",
        "base_revision":String::from_utf8(revision.stdout).unwrap().trim(),"source_sha256":sources,
        "rustc":String::from_utf8(rustc.stdout).unwrap(),"profile":"release","debug_assertions":cfg!(debug_assertions),
        "engine_version":engine["version"].as_str(),"engine_checksum":engine.get("checksum").and_then(toml::Value::as_str),
        "engine_source_identity":engine_identity,
        "compiler_version":"0.61.2 with existing pinned patches","os":env::consts::OS,
        "arch":env::consts::ARCH,"word_bytes":size_of::<usize>(),
        "test_binary_sha256":digest(&binary),"test_binary_bytes":binary.len(),"host_exclusive":false,
        "method_revision":2_u32,"samples":7_u32,"iterations_rule":"clamp(65536 / max(input_bytes, 64), 64, 1024)",
        "allocator_scope":"thread-local requested heap; wrapper remains installed while timing, accounting disabled",
        "construction_scope":"fresh API construction after process/library initialization; not cold-process startup",
        "memory_scope":"per-owner requested heap; phase deltas are incremental; complete lifetimes include construction/drop",
        "timing_scope":"paired fresh-cache history then repeated same input; hostname fresh-call path additionally measured",
        "limits":{"nfa_bytes":0x0001_0000_u64,"meta_hybrid_cache_bytes":0x0001_0000_u64,"schema_input_bytes":0x0001_0000_u64,"hostname_exclusive_bytes":256_u64},
        "pike_policy":"same forward NFA configuration and captures; direct boolean API, no meta prefilter"})
}

fn comparison(catalog: &Catalog) -> Result<Value, &'static str> {
    let context = metadata();
    let deadline = Instant::now()
        .checked_add(Duration::from_secs(180))
        .unwrap();
    let mut rows = Vec::with_capacity(catalog.patterns.len());
    for pattern in &catalog.patterns {
        check_deadline(deadline)?;
        let (subjects, agreement) = selected_subjects(pattern, &catalog.candidates);
        let mut row = measured_pattern(pattern, &subjects, deadline)?;
        row.as_object_mut()
            .unwrap()
            .insert("agreement".to_owned(), agreement);
        rows.push(row);
    }
    if rows.len() != 24 {
        return Err("incomplete matcher comparison");
    }
    check_deadline(deadline)?;
    Ok(json!({"schema_version":1_u32,"metadata":context,"corpus":catalog.metadata,"patterns":rows}))
}

fn emit_report(
    destination: Option<&Path>,
    optimized: bool,
    produce: impl FnOnce() -> Result<Value, &'static str>,
) -> Result<(), &'static str> {
    let Some(path) = destination else {
        return Ok(());
    };
    if !optimized {
        return Err("comparison reporting requires release mode");
    }
    if path
        .try_exists()
        .map_err(|_error| "comparison destination inspection failed")?
    {
        return Err("comparison report destination already exists");
    }
    let complete = produce()?;
    let mut bytes =
        serde_json::to_vec_pretty(&complete).map_err(|_error| "comparison encoding failed")?;
    bytes.push(b'\n');
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_error| "comparison report creation failed")?;
    file.write_all(&bytes)
        .map_err(|_error| "comparison report write failed")?;
    file.sync_all()
        .map_err(|_error| "comparison report sync failed")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        Engines, Pattern, Subject, check_deadline, collect_catalog, emit_report, engine_order,
        iterations, measured_pattern, panel,
    };
    use serde_json::{Value, json};
    use std::cell::Cell;
    use std::collections::BTreeSet;
    use std::env;
    use std::fs;
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[test]
    fn both_matchers_obey_literal_boolean_controls_and_unicode_search_semantics() {
        for (pattern, subject, expected) in [
            ("[0-9]+", "abc123def", true),
            ("[0-9]+", "abc", false),
            ("^[0-9]+$", "abc123def", false),
            ("^[0-9]+$", "123", true),
            (r"\d+", "\u{0661}", true),
            ("[0-9]+", "\u{0661}", false),
            ("^a+$", "aaa\na", false),
            ("^a+$", "aaaa", true),
            ("a*", "\u{00e9}", true),
            ("[a-z]+", "", false),
        ] {
            assert_eq!(
                Engines::new(pattern).matches(subject),
                (expected, expected),
                "{pattern}: {subject:?}"
            );
        }
    }

    #[test]
    fn sample_iterations_vary_with_bytes_and_bound_both_ends() {
        for (bytes, expected) in [
            (0, 1024),
            (64, 1024),
            (128, 512),
            (4096, 64),
            (0x0001_0000, 64),
        ] {
            assert_eq!(iterations(bytes), expected);
        }
    }

    #[test]
    fn matcher_metadata_preserves_the_local_engine_checksum_absence() {
        let context = super::metadata();
        assert_eq!(context["engine_version"], "0.4.18");
        assert!(context.as_object().unwrap().contains_key("engine_checksum"));
        assert!(context["engine_checksum"].is_null());
        assert_eq!(
            context["engine_source_identity"]["archive_sha256"],
            "ad8553b9b26413251cbf30e620595c7a41b3887f03da04579c0e6b0d6a06b4b2"
        );
        for field in ["patch_sha256", "tree_sha256"] {
            assert_eq!(
                context["engine_source_identity"][field]
                    .as_str()
                    .unwrap()
                    .len(),
                64,
                "{field}"
            );
        }
        assert_eq!(
            context["engine_source_identity"]["features"]
                .as_array()
                .unwrap()
                .len(),
            25
        );
    }

    #[test]
    fn an_absent_report_never_invokes_the_timing_producer() {
        let calls = Cell::new(0_usize);
        assert_eq!(
            emit_report(None, false, || {
                calls.set(calls.get() + 1);
                Ok(json!({}))
            }),
            Ok(())
        );
        assert_eq!(calls.get(), 0);
    }

    #[test]
    fn debug_reporting_refuses_before_measurement_or_file_creation() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let calls = Cell::new(0_usize);
        assert_eq!(
            emit_report(Some(&path), false, || {
                calls.set(calls.get() + 1);
                Ok(json!({}))
            }),
            Err("comparison reporting requires release mode")
        );
        assert_eq!(calls.get(), 0);
        assert!(!path.exists());
    }

    #[test]
    fn existing_reports_are_neither_remeasured_nor_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        fs::write(&path, b"unchanged").unwrap();
        let calls = Cell::new(0_usize);
        assert_eq!(
            emit_report(Some(&path), true, || {
                calls.set(calls.get() + 1);
                Ok(json!({}))
            }),
            Err("comparison report destination already exists")
        );
        assert_eq!(calls.get(), 0);
        assert_eq!(fs::read(path).unwrap(), b"unchanged");
    }

    #[test]
    fn a_failed_or_expired_probe_cannot_publish_partial_output() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        for reason in ["comparison deadline expired", "matcher disagreement"] {
            assert_eq!(emit_report(Some(&path), true, || Err(reason)), Err(reason));
            assert!(!path.exists());
        }
    }

    #[test]
    fn complete_reports_are_written_once_as_json_with_a_newline() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("report.json");
        let calls = Cell::new(0_usize);
        let expected = json!({"complete":true});
        emit_report(Some(&path), true, || {
            calls.set(calls.get() + 1);
            Ok(expected.clone())
        })
        .unwrap();
        let bytes = fs::read(path).unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap(), expected);
        assert_eq!(bytes.last(), Some(&b'\n'));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn elapsed_deadlines_refuse_instead_of_becoming_successful_partial_runs() {
        assert_eq!(
            check_deadline(Instant::now().checked_sub(Duration::from_secs(1)).unwrap()),
            Err("comparison deadline expired")
        );
        assert_eq!(
            check_deadline(Instant::now().checked_add(Duration::from_secs(1)).unwrap()),
            Ok(())
        );
    }

    #[test]
    fn catalog_matchers_agree_before_optional_measurement() {
        let catalog = collect_catalog();
        assert_eq!(catalog.patterns.len(), 24);
        assert_eq!(
            catalog.patterns.iter().filter(|row| row.hostname).count(),
            1
        );
        assert_eq!(catalog.metadata["capture_cases"], 300_u64);
        assert!(!catalog.candidates.is_empty());
        for pattern in &catalog.patterns {
            let (selected, counts) = super::selected_subjects(pattern, &catalog.candidates);
            assert!(selected.len() >= 19);
            assert_eq!(counts["candidate_subjects"], catalog.candidates.len());
        }
        let destination = env::var_os("AUCTIONEER__SCHEMA__MATCHER_REPORT");
        emit_report(
            destination.as_deref().map(Path::new),
            !cfg!(debug_assertions),
            || super::comparison(&catalog),
        )
        .unwrap();
    }

    #[test]
    fn adversarial_panels_keep_exact_input_boundaries_and_a_valid_overlong_hostname() {
        let ordinary = panel(false);
        assert_eq!(ordinary.len(), 19);
        assert!(ordinary.iter().any(|row| row.text.len() == 0x0001_0000));
        assert!(ordinary.iter().all(|row| row.text.len() <= 0x0001_0000));
        let hostname = panel(true);
        assert_eq!(hostname.len(), 23);
        let exact = hostname.iter().find(|row| row.label == "max-host").unwrap();
        let over = hostname
            .iter()
            .find(|row| row.label == "guarded-host")
            .unwrap();
        assert_eq!(exact.text.len(), 255);
        assert_eq!(over.text.len(), 256);
        assert!(over.text.split('.').all(|label| label.len() <= 63));
    }

    #[test]
    fn paired_samples_alternate_both_engine_orders() {
        for (round, expected) in [(0, [0, 1]), (1, [1, 0]), (2, [0, 1]), (3, [1, 0])] {
            assert_eq!(engine_order(round), expected);
        }
    }

    #[test]
    fn measured_patterns_keep_complete_lifetimes_and_all_seven_raw_samples() {
        let pattern = Pattern {
            key: "fake_pattern".to_owned(),
            text: "^a+$".to_owned(),
            hostname: false,
        };
        let subjects = vec![
            Subject {
                label: "match".to_owned(),
                text: "aaa".to_owned(),
                origins: vec![],
                expected: true,
            },
            Subject {
                label: "miss".to_owned(),
                text: "!".to_owned(),
                origins: vec![],
                expected: false,
            },
        ];
        let report = measured_pattern(
            &pattern,
            &subjects,
            Instant::now()
                .checked_add(Duration::from_secs(180))
                .unwrap(),
        )
        .unwrap();
        assert_eq!(report["key"], "fake_pattern");
        for engine in ["meta", "pikevm"] {
            for phase in [
                "program_lifetime",
                "cache_history_lifetime",
                "program_and_cache_lifetime",
            ] {
                assert!(
                    report["memory"][engine][phase]["allocated_bytes"]
                        .as_u64()
                        .unwrap()
                        > 0
                );
                assert_eq!(report["memory"][engine][phase]["retained_bytes"], 0_i64);
            }
            assert_eq!(
                report["program_construction_ns"][engine]
                    .as_array()
                    .unwrap()
                    .len(),
                7
            );
            let rows = report["searches"].as_array().unwrap();
            assert_eq!(rows.len(), 2);
            for (index, row) in rows.iter().enumerate() {
                assert_eq!(row["iterations"], 1024_u64);
                assert_eq!(row["expected"], index == 0);
                for phase in ["cache_construction_ns", "first_search_ns", "warm_batch_ns"] {
                    assert_eq!(row["timing"][engine][phase].as_array().unwrap().len(), 7);
                }
            }
        }
    }

    #[test]
    fn timing_selection_keeps_both_classes_provenance_and_unsampled_agreement_counts() {
        let pattern = Pattern {
            key: "fake_pattern".to_owned(),
            text: "^a+$".to_owned(),
            hostname: false,
        };
        let candidates = ["a", "aa", "aaa", "aaaa", "!", "?"]
            .into_iter()
            .map(|text| {
                (
                    text.to_owned(),
                    BTreeSet::from(["fake_source:Valid:0:string".to_owned()]),
                )
            })
            .collect();
        let (selected, counts) = super::selected_subjects(&pattern, &candidates);
        assert_eq!(counts["candidate_subjects"], 6_u64);
        assert_eq!(counts["positive"], 4_u64);
        assert_eq!(counts["negative"], 2_u64);
        assert_eq!(counts["guard_skips"], 0_u64);
        let captured: Vec<_> = selected
            .iter()
            .filter(|row| !row.origins.is_empty())
            .collect();
        assert_eq!(captured.len(), 5);
        assert_eq!(captured.iter().filter(|row| row.expected).count(), 3);
        assert!(
            captured
                .iter()
                .all(|row| row.origins == ["fake_source:Valid:0:string"])
        );
        assert!(selected.iter().any(|row| row.text.len() == 0x0001_0000));
    }

    #[test]
    fn hostname_guard_refuses_before_either_fresh_cache_is_constructed() {
        let pattern = Pattern {
            key: "fake_pattern".to_owned(),
            text: "^a+$".to_owned(),
            hostname: true,
        };
        let engines = Engines::new(&pattern.text);
        let over = "a".repeat(256);
        assert_eq!(engines.matches(&over), (true, true));
        let (meta, meta_metrics) = super::measure(|| {
            super::fresh(
                &pattern,
                &engines.meta,
                &over,
                super::meta::Regex::create_cache,
                super::search_meta,
            )
        });
        let (pike, pike_metrics) = super::measure(|| {
            super::fresh(
                &pattern,
                &engines.pike,
                &over,
                super::pikevm::PikeVM::create_cache,
                super::search_pike,
            )
        });
        assert_eq!((meta, pike), (false, false));
        for metrics in [meta_metrics, pike_metrics] {
            assert_eq!(metrics.json()["allocation_calls"], 0_u64);
        }
        let (accepted, positive) = super::measure(|| {
            super::fresh(
                &pattern,
                &engines.pike,
                "a",
                super::pikevm::PikeVM::create_cache,
                super::search_pike,
            )
        });
        assert!(accepted);
        assert!(positive.json()["allocation_calls"].as_u64().unwrap() > 0);
        assert_eq!(positive.json()["retained_bytes"], 0_i64);
    }
}
