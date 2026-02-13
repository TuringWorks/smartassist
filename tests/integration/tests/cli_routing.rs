//! CLI binary integration tests.
//!
//! These tests exercise the compiled `smartassist` binary to verify that
//! top-level command routing, help text, and error handling work as expected.

use std::path::PathBuf;
use std::process::Command;

/// Locate the compiled `smartassist` binary in the workspace target directory.
///
/// Cargo sets `CARGO_MANIFEST_DIR` to the manifest directory of the package
/// being tested. We navigate up to the workspace root and look inside
/// `target/debug/`.
fn smartassist_bin() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // tests/integration -> workspace root
    let workspace_root = manifest_dir
        .parent()
        .expect("tests/ parent")
        .parent()
        .expect("workspace root");
    let bin = workspace_root.join("target").join("debug").join("smartassist");
    assert!(
        bin.exists(),
        "smartassist binary not found at {}; run `cargo build -p smartassist-cli` first",
        bin.display()
    );
    bin
}

fn smartassist_cmd() -> Command {
    Command::new(smartassist_bin())
}

#[test]
fn test_cli_version() {
    let output = smartassist_cmd()
        .arg("version")
        .output()
        .expect("failed to run smartassist");
    assert!(output.status.success(), "version command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("smartassist"),
        "version output should contain 'smartassist', got: {}",
        stdout
    );
}

#[test]
fn test_cli_help() {
    let output = smartassist_cmd()
        .arg("--help")
        .output()
        .expect("failed to run smartassist");
    assert!(output.status.success(), "--help should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("agent"),
        "help output should mention 'agent', got: {}",
        stdout
    );
    assert!(
        stdout.contains("gateway"),
        "help output should mention 'gateway', got: {}",
        stdout
    );
}

#[test]
fn test_cli_unknown_command() {
    let output = smartassist_cmd()
        .arg("nonexistent-command")
        .output()
        .expect("failed to run smartassist");
    assert!(
        !output.status.success(),
        "unknown command should return non-zero exit code"
    );
}

#[test]
fn test_cli_config_help() {
    let output = smartassist_cmd()
        .args(["config", "--help"])
        .output()
        .expect("failed to run smartassist config --help");
    assert!(
        output.status.success(),
        "config --help should succeed"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("config") || stdout.contains("Config"),
        "config help should mention 'config', got: {}",
        stdout
    );
}

#[test]
fn test_cli_doctor_help() {
    let output = smartassist_cmd()
        .args(["doctor", "--help"])
        .output()
        .expect("failed to run smartassist doctor --help");
    assert!(
        output.status.success(),
        "doctor --help should succeed"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("doctor") || stdout.contains("Doctor") || stdout.contains("diagnostic"),
        "doctor help should mention diagnostics, got: {}",
        stdout
    );
}
