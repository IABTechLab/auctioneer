use serde_json::Value;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use serde_json::json;

// This isolated Rust retention entry is not a deployed FFI API or guest execution.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[allow(private_interfaces)]
#[unsafe(no_mangle)]
pub fn auctioneer_schema_validate(index: usize, input: &Value, owner: &mut CacheOwner) -> bool {
    VALIDATORS
        .get(index)
        .is_some_and(|(_, _, validate)| validate(input, owner))
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[unsafe(no_mangle)]
pub fn auctioneer_schema_initialize(index: usize) -> bool {
    INITIALIZERS
        .get(index)
        .is_some_and(|(_, initialize)| initialize())
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[unsafe(no_mangle)]
pub fn auctioneer_schema_membership() -> &'static str {
    MEMBERSHIP
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
fn main() {}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn raw_verdict(input: &str, validate: Validator, owner: &mut CacheOwner) -> Value {
    match serde_json::from_str::<Value>(input) {
        Ok(value) => json!({"kind": "schema", "valid": validate(&value, owner)}),
        Err(_) => json!({"kind": "invalid_json"}),
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let output = match args.as_slice() {
        [operation] if operation == "cache-layouts" => {
            let rows: Vec<_> = CACHE_LAYOUTS.iter().map(|(name, bytes, alignment)| {
                json!({"name": name, "inline_bytes": bytes, "alignment": alignment})
            }).collect();
            json!({
                "schema_version": 2,
                "word_bytes": std::mem::size_of::<usize>(),
                "cache_bytes": std::mem::size_of::<regex_automata::meta::Cache>(),
                "cache_alignment": std::mem::align_of::<regex_automata::meta::Cache>(),
                "rows": rows
            })
        }
        #[cfg(not(target_arch = "wasm32"))]
        [operation, workload] if operation == "allocations" => {
            schema_explicit_allocation::report(workload)?
        }
        [operation] if operation == "inventory" => {
            let empty = json!({});
            let names: Vec<_> = VALIDATORS.iter().map(|(name, _, _)| *name).collect();
            let results: Vec<_> = VALIDATORS
                .iter()
                .map(|(_, factory, validate)| {
                    factory().is_some_and(|mut owner| validate(&empty, &mut owner))
                })
                .collect();
            json!({"names": names, "results": results, "membership": MEMBERSHIP})
        }
        [operation] if operation == "initialize" => {
            let names: Vec<_> = INITIALIZERS.iter().map(|(name, _)| *name).collect();
            let first: Vec<_> = INITIALIZERS
                .iter()
                .map(|(_, initialize)| initialize())
                .collect();
            let repeated: Vec<_> = INITIALIZERS
                .iter()
                .map(|(_, initialize)| initialize())
                .collect();
            json!({"names": names, "first": first, "repeated": repeated})
        }
        [operation, name, input] if operation == "check" && input.len() <= 65_536 => {
            let (_, factory, validate) = VALIDATORS
                .iter()
                .find(|(candidate, _, _)| candidate == name)
                .ok_or("unknown schema")?;
            let value: Value = serde_json::from_str(input)?;
            let mut owner = factory().ok_or("schema cache construction failed")?;
            json!({"valid": validate(&value, &mut owner)})
        }
        #[cfg(not(target_arch = "wasm32"))]
        [operation, name, input]
            if matches!(
                operation.as_str(),
                "check-formats-disabled" | "check-draft4"
            ) && input.len() <= 65_536 =>
        {
            let value: Value = serde_json::from_str(input)?;
            json!({"valid": alternate_controls::check(operation, name, &value).ok_or("unknown alternate schema control")?})
        }
        [operation, name, input] if operation == "check-raw" && input.len() <= 65_536 => {
            let (_, factory, validate) = VALIDATORS
                .iter()
                .find(|(candidate, _, _)| candidate == name)
                .ok_or("unknown schema")?;
            match serde_json::from_str::<Value>(input) {
                Ok(value) => {
                    let mut owner = factory().ok_or("schema cache construction failed")?;
                    json!({"kind": "schema", "valid": validate(&value, &mut owner)})
                }
                Err(_) => json!({"kind": "invalid_json"}),
            }
        }
        [operation, name, input] if operation == "raw-sequence" && input.len() <= 65_536 => {
            let (_, factory, validate) = VALIDATORS
                .iter()
                .find(|(candidate, _, _)| candidate == name)
                .ok_or("unknown schema")?;
            let inputs: Vec<String> = serde_json::from_str(input)?;
            let mut owner = factory().ok_or("schema cache construction failed")?;
            let results: Vec<_> = inputs.iter().map(|raw| raw_verdict(raw, *validate, &mut owner)).collect();
            json!(results)
        }
        [operation, name, input] if operation == "sequence" && input.len() <= 65_536 => {
            let (_, factory, validate) = VALIDATORS
                .iter()
                .find(|(candidate, _, _)| candidate == name)
                .ok_or("unknown schema")?;
            let values: Vec<Value> = serde_json::from_str(input)?;
            let mut owner = factory().ok_or("schema cache construction failed")?;
            let results: Vec<_> = values
                .iter()
                .map(|value| validate(value, &mut owner))
                .collect();
            json!(results)
        }
        _ => return Err("invalid probe input".into()),
    };
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
