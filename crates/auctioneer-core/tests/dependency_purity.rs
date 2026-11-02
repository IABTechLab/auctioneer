//! Library crates must NEVER transitively depend on
//! `EdgeZero` or runtime crates. Only auctioneer-core and the four adapter
//! crates may.

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::process::Command;

    const LIB_CRATES: &[&str] = &[
        "auctioneer-openrtb",
        "auctioneer-config",
        "auctioneer-runtime",
        "auctioneer-privacy",
        "auctioneer-bidder",
        "auctioneer-exchange",
    ];
    const FORBIDDEN: &[&str] = &[
        "edgezero-core",
        "edgezero-adapter-axum",
        "edgezero-adapter-cloudflare",
        "edgezero-adapter-fastly",
        "edgezero-adapter-spin",
        "edgezero-cli",
        "edgezero-macros",
        "tokio",
        "reqwest",
        "fastly",
        "worker",
        "spin-sdk",
    ];

    #[test]
    fn library_crates_have_no_forbidden_deps() {
        let output = Command::new(env!("CARGO"))
            .args(["metadata", "--format-version", "1"])
            .output()
            .expect("cargo metadata");
        assert!(
            output.status.success(),
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr),
        );
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let packages = json["packages"].as_array().unwrap();
        let nodes = json["resolve"]["nodes"].as_array().unwrap();

        let mut violations: Vec<String> = Vec::new();
        for &lib in LIB_CRATES {
            let Some(lib_id) = packages
                .iter()
                .find(|p| p["name"] == lib)
                .map(|p| p["id"].as_str().unwrap().to_owned())
            else {
                continue;
            };
            let reachable = walk(&lib_id, nodes);
            for dep_id in &reachable {
                let name = packages
                    .iter()
                    .find(|p| p["id"] == serde_json::Value::String(dep_id.clone()))
                    .and_then(|p| p["name"].as_str())
                    .unwrap_or("");
                if FORBIDDEN.contains(&name) {
                    violations.push(format!(
                        "library crate `{lib}` transitively depends on `{name}`"
                    ));
                }
            }
        }
        assert!(violations.is_empty(), "{}", violations.join("\n"));
    }

    fn walk(start: &str, nodes: &[serde_json::Value]) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![start.to_owned()];
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(node) = nodes
                .iter()
                .find(|n| n["id"] == serde_json::Value::String(id.clone()))
                && let Some(deps) = node["dependencies"].as_array()
            {
                for dep in deps {
                    stack.push(dep.as_str().unwrap().to_owned());
                }
            }
        }
        seen
    }
}
