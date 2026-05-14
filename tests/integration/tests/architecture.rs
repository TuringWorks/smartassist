//! Architecture boundary tests.
//!
//! Enforces crate dependency rules to prevent circular dependencies
//! and maintain the layered architecture.

use std::collections::HashMap;
use std::path::Path;

/// Parse a crate's Cargo.toml and return its local dependency names.
fn parse_local_deps(manifest_dir: &Path) -> Vec<String> {
    let cargo_toml = manifest_dir.join("Cargo.toml");
    let content = std::fs::read_to_string(&cargo_toml)
        .unwrap_or_else(|e| panic!("Failed to read {:?}: {}", cargo_toml, e));
    let manifest: toml::Value = content
        .parse()
        .unwrap_or_else(|e| panic!("Failed to parse {:?}: {}", cargo_toml, e));

    let mut deps = Vec::new();

    if let Some(table) = manifest.get("dependencies").and_then(|d| d.as_table()) {
        for (name, value) in table {
            if name.starts_with("smartassist-") {
                if let Some(dep_table) = value.as_table() {
                    if dep_table.contains_key("path") {
                        deps.push(name.clone());
                    }
                }
            }
        }
    }

    deps
}

/// Assert that `crate_name` does not directly depend on any crate in `forbidden`.
fn assert_no_dep(crate_name: &str, forbidden: &[&str]) {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let crate_dir = workspace_root.join("crates").join(crate_name);
    let deps = parse_local_deps(&crate_dir);

    for bad in forbidden {
        assert!(
            !deps.contains(&bad.to_string()),
            "{} must not depend on {} (found in Cargo.toml)",
            crate_name,
            bad
        );
    }
}

/// smartassist-core is the foundation: it must not depend on any other smartassist crate.
#[test]
fn test_core_has_no_internal_deps() {
    let forbidden = [
        "smartassist-agent",
        "smartassist-browser",
        "smartassist-canvas",
        "smartassist-channels",
        "smartassist-cli",
        "smartassist-cron",
        "smartassist-gateway",
        "smartassist-mcp",
        "smartassist-memory",
        "smartassist-plugin-sdk",
        "smartassist-providers",
        "smartassist-sandbox",
        "smartassist-security",
        "smartassist-talk",
        "smartassist-transcription",
    ];
    assert_no_dep("smartassist-core", &forbidden);
}

/// smartassist-plugin-sdk must not depend on higher-level crates.
#[test]
fn test_plugin_sdk_has_no_gateway_or_cli_deps() {
    let forbidden = [
        "smartassist-gateway",
        "smartassist-cli",
        "smartassist-agent",
        "smartassist-browser",
        "smartassist-canvas",
        "smartassist-cron",
        "smartassist-mcp",
        "smartassist-security",
        "smartassist-talk",
    ];
    assert_no_dep("smartassist-plugin-sdk", &forbidden);
}

/// smartassist-security must not depend on application-level crates.
#[test]
fn test_security_has_no_app_deps() {
    let forbidden = [
        "smartassist-gateway",
        "smartassist-cli",
        "smartassist-channels",
        "smartassist-browser",
        "smartassist-canvas",
        "smartassist-cron",
        "smartassist-mcp",
        "smartassist-talk",
    ];
    assert_no_dep("smartassist-security", &forbidden);
}

/// smartassist-sandbox must not depend on application-level crates.
#[test]
fn test_sandbox_has_no_app_deps() {
    let forbidden = [
        "smartassist-gateway",
        "smartassist-cli",
        "smartassist-channels",
        "smartassist-browser",
        "smartassist-canvas",
        "smartassist-cron",
        "smartassist-mcp",
        "smartassist-security",
        "smartassist-talk",
    ];
    assert_no_dep("smartassist-sandbox", &forbidden);
}

/// Verify the full dependency graph has no cycles among a sample of crates.
#[test]
fn test_no_cycles_in_sample() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let crates_dir = workspace_root.join("crates");

    let mut graph: HashMap<String, Vec<String>> = HashMap::new();

    for entry in std::fs::read_dir(&crates_dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().to_string();
        let deps = parse_local_deps(&entry.path());
        graph.insert(name, deps);
    }

    // DFS cycle detection for each crate
    fn has_cycle(
        node: &str,
        graph: &HashMap<String, Vec<String>>,
        visited: &mut HashMap<String, bool>,
        stack: &mut HashMap<String, bool>,
    ) -> bool {
        if let Some(&true) = stack.get(node) {
            return true;
        }
        if visited.get(node) == Some(&true) {
            return false;
        }

        visited.insert(node.to_string(), true);
        stack.insert(node.to_string(), true);

        if let Some(deps) = graph.get(node) {
            for dep in deps {
                // Only follow smartassist crates
                if dep.starts_with("smartassist-") && has_cycle(dep, graph, visited, stack) {
                    return true;
                }
            }
        }

        stack.insert(node.to_string(), false);
        false
    }

    let mut visited = HashMap::new();
    let mut stack = HashMap::new();

    for crate_name in graph.keys() {
        assert!(
            !has_cycle(crate_name, &graph, &mut visited, &mut stack),
            "Cycle detected starting from {}",
            crate_name
        );
    }
}
