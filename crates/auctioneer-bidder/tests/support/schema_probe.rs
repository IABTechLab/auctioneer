use serde_json::Value;
#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
use serde_json::json;

// Declarations, format functions and the static dispatch table are prepended.
// The isolated probe owns this export name; this is not a deployed FFI API.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[unsafe(no_mangle)]
pub fn auctioneer_schema_validate(index: usize, input: &Value) -> bool {
    VALIDATORS
        .get(index)
        .is_some_and(|(_, validate)| validate(input))
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
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let output = match args.as_slice() {
        #[cfg(not(target_arch = "wasm32"))]
        [operation, workload] if operation == "allocations" => {
            schema_default_allocation::report(workload)?
        }
        [operation] if operation == "inventory" => {
            let empty = json!({});
            let names: Vec<_> = VALIDATORS.iter().map(|(name, _)| *name).collect();
            let results: Vec<_> = VALIDATORS
                .iter()
                .map(|(_, validate)| validate(&empty))
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
            let (_, validate) = VALIDATORS
                .iter()
                .find(|(candidate, _)| candidate == name)
                .ok_or("unknown schema")?;
            let value: Value = serde_json::from_str(input)?;
            json!({"valid": validate(&value)})
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
            let (_, validate) = VALIDATORS
                .iter()
                .find(|(candidate, _)| candidate == name)
                .ok_or("unknown schema")?;
            match serde_json::from_str::<Value>(input) {
                Ok(value) => json!({"kind": "schema", "valid": validate(&value)}),
                Err(_) => json!({"kind": "invalid_json"}),
            }
        }
        _ => return Err("invalid probe input".into()),
    };
    println!("{}", serde_json::to_string(&output)?);
    Ok(())
}
