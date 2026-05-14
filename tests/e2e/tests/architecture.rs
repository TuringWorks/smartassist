//! Architecture boundary tests.
//!
//! These tests enforce crate dependency rules using cargo-metadata.

use serde_json::Value;
use std::process::Command;

/// Run `cargo metadata` for a crate and return the parsed JSON.
fn metadata_for_crate(manifest_path: &str) -> Value {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version",
            "1",
            "--manifest-path",
            manifest_path,
        ])
        .output()
        .expect("cargo metadata should run");

    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).expect("cargo metadata should be valid JSON")
}

/// Collect dependency names (by package ID) for a given root package name.
fn dependency_names(metadata: &Value, package_name: &str) -> Vec<String> {
    let packages = metadata["packages"].as_array().expect("packages array");
    let resolve = metadata["resolve"].as_object().expect("resolve object");
    let root = resolve["root"].as_str().expect("resolve root");

    // Map package ID to name
    let mut id_to_name: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for pkg in packages {
        let id = pkg["id"].as_str().unwrap_or("").to_string();
        let name = pkg["name"].as_str().unwrap_or("").to_string();
        id_to_name.insert(id, name);
    }

    // Find the node for the root package and collect its dependencies
    let nodes = resolve["nodes"].as_array().expect("nodes array");
    let mut dep_names = Vec::new();
    for node in nodes {
        if node["id"].as_str() == Some(root) {
            if let Some(deps) = node["dependencies"].as_array() {
                for dep in deps {
                    if let Some(dep_id) = dep.as_str() {
                        if let Some(name) = id_to_name.get(dep_id) {
                            dep_names.push(name.clone());
                        }
                    }
                }
            }
            break;
        }
    }
    dep_names
}

/// Verify that plugin-sdk does not directly depend on gateway.
#[test]
fn test_plugin_sdk_does_not_depend_on_gateway() {
    let metadata = metadata_for_crate("../../crates/smartassist-plugin-sdk/Cargo.toml");
    let deps = dependency_names(&metadata, "smartassist-plugin-sdk");
    assert!(
        !deps.contains(&"smartassist-gateway".to_string()),
        "plugin-sdk must not depend on gateway, but depends on: {:?}",
        deps
    );
}

/// Verify that core does not directly depend on any other smartassist crate.
#[test]
fn test_core_has_no_internal_dependencies() {
    let metadata = metadata_for_crate("../../crates/smartassist-core/Cargo.toml");
    let deps = dependency_names(&metadata, "smartassist-core");
    let internal = [
        "smartassist-agent",
        "smartassist-gateway",
        "smartassist-channels",
        "smartassist-memory",
        "smartassist-sandbox",
        "smartassist-secrets",
        "smartassist-providers",
        "smartassist-plugin-sdk",
        "smartassist-cli",
    ];
    for forbidden in internal {
        assert!(
            !deps.contains(&forbidden.to_string()),
            "smartassist-core must not depend on {}, but depends on: {:?}",
            forbidden,
            deps
        );
    }
}
