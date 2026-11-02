//! Native requested-heap observations, not work, RSS, or WASM qualification.

use serde_json::{Value, json};
use std::alloc::Layout;
use std::collections::{HashMap, hash_map::RandomState};
use std::hint::black_box;
use std::mem::{align_of, size_of};
use std::sync::Arc;

pub(crate) use crate::schema_meter::{Metrics, measure};

pub(crate) fn controls() -> Value {
    // Fixed diagnostics stay outside this thread's allocation windows.
    eprintln!("allocation phase: entered");
    let (_, positive) = measure(|| {
        // SAFETY: each nonzero layout has alignment 1; every live allocation
        // is released with its current layout. OOM aborts before dereference.
        unsafe {
            let layout = |size| Layout::from_size_align(size, 1).unwrap();
            let first = black_box(std::alloc::alloc(black_box(layout(37))));
            if first.is_null() {
                std::alloc::handle_alloc_error(layout(37));
            }
            let zeroed = black_box(std::alloc::alloc_zeroed(black_box(layout(19))));
            if zeroed.is_null() {
                std::alloc::handle_alloc_error(layout(19));
            }
            let grown = black_box(std::alloc::realloc(first, layout(37), black_box(101)));
            if grown.is_null() {
                std::alloc::handle_alloc_error(layout(101));
            }
            let shrunk = black_box(std::alloc::realloc(grown, layout(101), black_box(23)));
            if shrunk.is_null() {
                std::alloc::handle_alloc_error(layout(23));
            }
            std::alloc::dealloc(black_box(shrunk), layout(23));
            std::alloc::dealloc(black_box(zeroed), layout(19));
        }
    });
    eprintln!("allocation phase: positive-complete");
    let (retained, retained_metrics) =
        measure(|| black_box(vec![black_box(7_u8); black_box(113)].into_boxed_slice()));
    eprintln!("allocation phase: retained-complete");
    let (_, release) = measure(|| drop(black_box(retained)));
    eprintln!("allocation phase: release-complete");
    // Construction, parsing and serialization happen before the zero control.
    let encoded = black_box(serde_json::to_vec(&json!({"value":[1,2,3]})).unwrap());
    let value: Value = serde_json::from_slice(&encoded).unwrap();
    let (_, outside) = measure(|| {
        black_box((&value, &encoded));
    });
    eprintln!("allocation phase: outside-complete");
    use std::sync::atomic::{AtomicU8, Ordering};
    let stage = AtomicU8::new(0);
    let (_, other_thread) = std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            while stage.load(Ordering::Acquire) != 1 {
                std::hint::spin_loop();
            }
            black_box(vec![black_box(5_u8); black_box(1 << 20)]);
            stage.store(2, Ordering::Release);
        });
        // This records spawn returning, not the worker being scheduled.
        eprintln!("allocation phase: thread-spawned");
        let result = measure(|| {
            stage.store(1, Ordering::Release);
            while stage.load(Ordering::Acquire) != 2 {
                std::hint::spin_loop();
            }
        });
        worker.join().unwrap();
        result
    });
    eprintln!("allocation phase: thread-complete");
    let collection_layouts = collection_layouts();
    let fresh_core = fresh_core_controls();
    let lazy_fallbacks = fallback_controls();
    let report = json!({"positive": positive.json(), "retained": retained_metrics.json(),
        "release": release.json(), "outside": outside.json(), "other_thread": other_thread.json(),
        "collection_layouts": collection_layouts, "fresh_core": fresh_core,
        "lazy_fallbacks": lazy_fallbacks});
    eprintln!("allocation phase: complete");
    report
}

fn fresh_core_controls() -> Value {
    use regex_automata::meta::Regex;
    use regex_automata::nfa::thompson::{backtrack::BoundedBacktracker, pikevm::PikeVM};

    let sixteen = format!("^{}a+{}$", "(".repeat(16), ")".repeat(16));
    let seventeen = format!("^{}a+{}$", "(".repeat(17), ")".repeat(17));
    let cases = [
        ("no-captures", vec!["^a+$"], true),
        ("one-capture", vec!["^(a+)$"], true),
        ("two-captures", vec!["^(a+)(b+)$"], true),
        ("onepass-disabled", vec!["^(a+)(b+)$"], false),
        ("sixteen-captures", vec![sixteen.as_str()], true),
        ("seventeen-captures", vec![seventeen.as_str()], true),
        ("two-patterns", vec!["^(a+)$", "^(b+)(c+)$"], true),
    ];
    let rows: Vec<_> = cases
        .into_iter()
        .map(|(id, patterns, enabled)| {
            // Only the control isolates these owners. Emitted validators keep
            // their unchanged hybrid/strategy settings and require composition.
            let engine = Regex::builder()
                .configure(
                    Regex::config()
                        .auto_prefilter(false)
                        .hybrid(false)
                        .dfa(false)
                        .onepass(enabled)
                        .nfa_size_limit(Some(65_536)),
                )
                .build_many(&patterns)
                .unwrap();
            let (cache, construction) = measure(|| black_box(engine.create_cache()));
            let reported_bytes = cache.memory_usage();
            let (_, release) = measure(|| drop(black_box(cache)));
            json!({"id": id, "slots": engine.group_info().slot_len(),
                "patterns": engine.pattern_len(), "onepass_enabled": enabled,
                "reported_bytes": reported_bytes, "construction": construction.json(),
                "release": release.json()})
        })
        .collect();
    let pike = PikeVM::new("^(a+)(b+)$").unwrap();
    let backtrack = BoundedBacktracker::new("^(a+)(b+)$").unwrap();
    let (cache, construction) = measure(|| black_box(pike.create_cache()));
    let reported_bytes = cache.memory_usage();
    let (_, release) = measure(|| drop(black_box(cache)));
    let standalone_pike = json!({"reported_bytes": reported_bytes,
        "construction": construction.json(), "release": release.json()});
    let (cache, construction) = measure(|| black_box(backtrack.create_cache()));
    let (_, release) = measure(|| drop(black_box(cache)));
    json!({"word_bytes": size_of::<usize>(), "rows": rows,
        "standalone_pike": standalone_pike,
        "standalone_backtrack": {"construction": construction.json(), "release": release.json()}})
}

fn fallback_history<C>(
    create: impl FnOnce() -> C,
    mut search: impl FnMut(&mut C, &str) -> bool,
    reported: impl Fn(&C) -> usize,
    inputs: &[(&str, String)],
) -> Value {
    let (mut cache, construction) = measure(|| black_box(create()));
    let mut total = construction;
    let mut searches = Vec::with_capacity(inputs.len());
    for (phase, input) in inputs {
        let (valid, metrics) =
            measure(|| black_box(search(black_box(&mut cache), black_box(input))));
        total.append(metrics);
        searches.push(
            json!({"phase": phase, "input_bytes": input.len(), "valid": valid,
            "reported_bytes": reported(&cache), "metrics": metrics.json()}),
        );
    }
    let (_, release) = measure(|| drop(black_box(cache)));
    total.append(release);
    json!({"construction": construction.json(), "searches": searches,
        "release": release.json(), "total": total.json()})
}

fn fallback_controls() -> Value {
    use regex_automata::Input;
    use regex_automata::nfa::thompson::{self, NFA, backtrack::BoundedBacktracker, pikevm::PikeVM};
    let rows: Vec<_> = [
        ("alternates", "^(a|b)+z$", "abz", "a"),
        ("captures", "^(a+)(b*)z$", "abz", "a"),
        ("unicode", r"^\p{Greek}+z$", "\u{3b1}z", "\u{3b1}"),
    ]
    .into_iter()
    .map(|(id, pattern, matched, repeated)| {
        let nfa = NFA::compiler()
            .configure(
                thompson::Config::new()
                    .shrink(false)
                    .nfa_size_limit(Some(65_536)),
            )
            .build(pattern)
            .unwrap();
        let pike = PikeVM::new_from_nfa(nfa.clone()).unwrap();
        let backtrack = BoundedBacktracker::new_from_nfa(nfa.clone()).unwrap();
        let inputs = [
            ("empty", String::new()),
            ("short-match", matched.to_owned()),
            (
                "long-miss",
                repeated.repeat(128_usize.checked_div(repeated.len()).unwrap()),
            ),
            ("short-after-long", matched.to_owned()),
        ];
        // The slots are caller-owned, outside cache measurement, just as
        // meta's eager Captures owner is disjoint from its lazy fallbacks.
        let mut slots = vec![None; nfa.group_info().slot_len()];
        let pike = fallback_history(
            || pike.create_cache(),
            |cache, input| {
                pike.search_slots(cache, &Input::new(input).earliest(true), &mut slots)
                    .is_some()
            },
            |cache| cache.memory_usage(),
            &inputs,
        );
        let backtrack = fallback_history(
            || backtrack.create_cache(),
            |cache, input| {
                backtrack
                    .try_search_slots(cache, &Input::new(input).earliest(true), &mut slots)
                    .unwrap()
                    .is_some()
            },
            |cache| cache.memory_usage(),
            &inputs,
        );
        json!({"id": id, "pattern": pattern, "pike": pike, "backtrack": backtrack})
    })
    .collect();
    Value::Array(rows)
}

fn vector_layout<T: Clone>(kind: &str, value: &T, high_water: usize) -> Value {
    let (items, construction) = measure(|| {
        let mut items = Vec::new();
        for _ in 0..high_water {
            items.push(black_box(value.clone()));
        }
        black_box(items)
    });
    let capacity = items.capacity();
    let length = items.len();
    let (_, release) = measure(|| drop(black_box(items)));
    json!({"kind": kind, "high_water": high_water, "element_bytes": size_of::<T>(),
        "length": length, "capacity": capacity,
        "construction": construction.json(), "release": release.json()})
}

fn state_map_layout(high_water: usize) -> Value {
    let keys = (0..high_water)
        .map(|index| {
            let bytes = u64::try_from(index).unwrap().to_ne_bytes();
            Arc::<[u8]>::from(bytes.as_slice())
        })
        .collect::<Vec<_>>();
    let hasher = RandomState::new();
    let (mut table, construction) = measure(|| {
        let mut table = HashMap::with_hasher(hasher);
        for key in &keys {
            table.insert(Arc::clone(key), black_box(7_u32));
        }
        black_box(table)
    });
    let (_, replacement) = measure(|| {
        for key in &keys {
            table.insert(Arc::clone(key), black_box(9_u32));
        }
    });
    let length_after_replace = table.len();
    let capacity_after_replace = table.capacity();
    let (_, cleared) = measure(|| table.clear());
    let length_after_clear = table.len();
    let capacity_after_clear = table.capacity();
    let (_, repeated) = measure(|| {
        for key in &keys {
            table.insert(Arc::clone(key), black_box(11_u32));
        }
    });
    let length_after_repeat = table.len();
    let (_, release) = measure(|| drop(black_box(table)));
    json!({"high_water": high_water, "length_after_replace": length_after_replace,
        "capacity_after_replace": capacity_after_replace, "length_after_clear": length_after_clear,
        "capacity_after_clear": capacity_after_clear, "construction": construction.json(),
        "length_after_repeat": length_after_repeat,
        "replacement": replacement.json(), "clear": cleared.json(), "repeat": repeated.json(),
        "release": release.json()})
}

fn arc_layout(payload_bytes: usize) -> Value {
    let input = vec![7_u8; payload_bytes];
    let (owner, construction) = measure(|| Arc::<[u8]>::from(black_box(input.as_slice())));
    let (clones, cloned) = measure(|| (Arc::clone(&owner), Arc::clone(&owner)));
    let strong_after_clone = Arc::strong_count(&owner);
    let (_, drop_clones) = measure(|| drop(black_box(clones)));
    let strong_after_drop = Arc::strong_count(&owner);
    let (_, release) = measure(|| drop(black_box(owner)));
    json!({"payload_bytes": payload_bytes, "strong_after_clone": strong_after_clone,
        "strong_after_drop": strong_after_drop, "construction": construction.json(),
        "clone": cloned.json(), "drop_clones": drop_clones.json(), "release": release.json()})
}

fn collection_layouts() -> Value {
    let mut vectors = Vec::new();
    let shared: Arc<[u8]> = Arc::from([1_u8, 2, 3].as_slice());
    for kind in ["byte", "id", "handle"] {
        for high_water in [0, 1, 3, 4, 5, 8, 9, 16, 17, 65] {
            let row = match kind {
                "byte" => vector_layout(kind, &7_u8, high_water),
                "id" => vector_layout(kind, &7_u32, high_water),
                "handle" => vector_layout(kind, &shared, high_water),
                _ => unreachable!(),
            };
            vectors.push(row);
        }
    }
    let maps = [0, 1, 3, 7, 14, 15, 28, 29, 56, 57, 128]
        .into_iter()
        .map(state_map_layout)
        .collect::<Vec<_>>();
    let arcs = [0, 1, 4, 7, 8, 9, 15, 16, 17, 31, 32, 33, 255]
        .into_iter()
        .map(arc_layout)
        .collect::<Vec<_>>();
    json!({"word_bytes": size_of::<usize>(), "pair_bytes": size_of::<(Arc<[u8]>, u32)>(),
        "pair_align": align_of::<(Arc<[u8]>, u32)>(), "vectors": vectors,
        "state_maps": maps, "arc_slices": arcs})
}

pub(crate) struct Case {
    pub(crate) id: String,
    pub(crate) name: &'static str,
    pub(crate) input: Value,
    pub(crate) expected: Option<bool>,
}

pub(crate) fn shape(value: &Value) -> (usize, usize) {
    let children: Vec<_> = match value {
        Value::Array(values) => values.iter().collect(),
        Value::Object(values) => values.values().collect(),
        _ => return (1, 0),
    };
    // Match bounded_json's syntactic convention: object keys are nodes too.
    let mut nodes = 1 + value.as_object().map_or(0, serde_json::Map::len);
    let mut depth = 1;
    for child in children {
        let (child_nodes, child_depth) = shape(child);
        nodes += child_nodes;
        depth = depth.max(1 + child_depth);
    }
    (nodes, depth)
}

fn max_string(mut input: Value, pointer: &str, character: char, invalid: bool) -> Value {
    let bytes = serde_json::to_vec(&input).unwrap().len();
    let field = input.pointer_mut(pointer).unwrap();
    let length = field.as_str().unwrap().len() + 65_536 - bytes;
    let mut text = character.to_string().repeat(length);
    if invalid {
        text.pop();
        text.push('!');
    }
    *field = Value::String(text);
    assert_eq!(serde_json::to_vec(&input).unwrap().len(), 65_536);
    input
}

fn max_collection(mut input: Value, pointer: &str, invalid: bool) -> Value {
    let count = 4096 - shape(&input).0;
    let array = input.pointer_mut(pointer).unwrap().as_array_mut().unwrap();
    array.extend((0..count).map(|_| json!("x")));
    if invalid {
        *array.last_mut().unwrap() = Value::Bool(false);
    }
    assert_eq!(shape(&input).0, 4096);
    input
}

fn max_properties(mut input: Value, pointer: &str, invalid: bool) -> Value {
    let remaining = 4096 - shape(&input).0;
    let object = input.pointer_mut(pointer).unwrap().as_object_mut().unwrap();
    for index in 0..remaining / 2 {
        let value = if index == 0 && remaining % 2 == 1 {
            json!(["x"])
        } else if invalid && index == remaining / 2 - 1 {
            json!(false)
        } else {
            json!([])
        };
        object.insert(format!("k{index:04}"), value);
    }
    assert_eq!(shape(&input).0, 4096);
    input
}

pub(crate) fn workloads() -> impl Iterator<Item = &'static str> {
    crate::WORKLOAD_DATA.iter().map(|(name, _)| *name)
}

pub(crate) fn cases(name: &str) -> Vec<Case> {
    let (name, encoded) = crate::WORKLOAD_DATA
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .unwrap();
    let data: Value = serde_json::from_str(encoded).unwrap();
    data["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let mut input = if let Some(encoded) = row["input_json"].as_str() {
                serde_json::from_str(encoded).unwrap()
            } else {
                row["input"].clone()
            };
            for operation in row["transforms"].as_array().unwrap() {
                let pointer = operation["pointer"].as_str().unwrap();
                let invalid = operation["invalid"].as_bool().unwrap_or(false);
                input = match operation["op"].as_str().unwrap() {
                    "max_string" => max_string(
                        input,
                        pointer,
                        operation["character"]
                            .as_str()
                            .unwrap()
                            .chars()
                            .next()
                            .unwrap(),
                        invalid,
                    ),
                    "max_collection" => max_collection(input, pointer, invalid),
                    "max_properties" => max_properties(input, pointer, invalid),
                    "set" => {
                        *input.pointer_mut(pointer).unwrap() = operation["value"].clone();
                        input
                    }
                    "insert" => {
                        input
                            .pointer_mut(pointer)
                            .unwrap()
                            .as_object_mut()
                            .unwrap()
                            .insert(
                                operation["key"].as_str().unwrap().to_owned(),
                                operation["value"].clone(),
                            );
                        input
                    }
                    "pop" => {
                        for _ in 0..operation["count"].as_u64().unwrap() {
                            input
                                .pointer_mut(pointer)
                                .unwrap()
                                .as_array_mut()
                                .unwrap()
                                .pop()
                                .unwrap();
                        }
                        input
                    }
                    "nested_arrays" => {
                        let mut value = json!([]);
                        for _ in 0..operation["depth"].as_u64().unwrap() {
                            value = json!([value]);
                        }
                        *input.pointer_mut(pointer).unwrap() = value;
                        input
                    }
                    other => panic!("unknown workload operation: {other}"),
                };
            }
            Case {
                id: row["id"].as_str().unwrap().to_owned(),
                name,
                input,
                expected: Some(row["valid"].as_bool().unwrap()),
            }
        })
        .collect()
}

pub(crate) fn summary(rows: &[(bool, Metrics)]) -> Metrics {
    let mut total = Metrics::default();
    for (_, metrics) in rows {
        total.append(*metrics);
    }
    total
}

pub(crate) fn constructors() -> Value {
    let run = || {
        let mut results = Vec::new();
        // Keep the format observation first so summaries follow execution order.
        results.push(measure(|| {
            black_box(crate::schema_formats::initialize_reference_formats())
        }));
        for (_, initialize) in crate::INITIALIZERS {
            results.push(measure(|| black_box(initialize())));
        }
        results
    };
    let first = run();
    let repeat = run();
    let schemas: Vec<_> = crate::INITIALIZERS
        .iter()
        .zip(&first[1..])
        .zip(&repeat[1..])
        .map(|(((name, _), initial), next)| {
            json!({"name":name,"first":{"initialized":initial.0,"metrics":initial.1.json()},
                "repeat":{"initialized":next.0,"metrics":next.1.json()}})
        })
        .collect();
    json!({"formats":{
        "first":{"initialized":first[0].0,"metrics":first[0].1.json()},
        "repeat":{"initialized":repeat[0].0,"metrics":repeat[0].1.json()}},
        "schemas":schemas,"first_pass":summary(&first).json(),"repeat":summary(&repeat).json()})
}
