//! Config save/load roundtrip integration tests.
//!
//! These tests verify that configuration can be serialized, written to disk,
//! and loaded back with identical field values.

use smartassist_core::config::Config;
use std::path::Path;
use tempfile::TempDir;

#[test]
fn test_config_save_and_load() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.json");

    let config = Config::default();
    config.save(&path).unwrap();

    let loaded = Config::load(&path).unwrap();
    // Default port should survive the roundtrip
    assert_eq!(loaded.gateway.port, config.gateway.port);
    // Default bind mode should survive the roundtrip
    assert_eq!(loaded.gateway.bind, config.gateway.bind);
    // Memory search defaults should survive the roundtrip
    assert_eq!(loaded.memory.search.limit, config.memory.search.limit);
    assert_eq!(loaded.memory.search.top_k, config.memory.search.top_k);
}

#[test]
fn test_config_modify_and_reload() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.json");

    let mut config = Config::default();
    config.gateway.port = 9090;
    config.save(&path).unwrap();

    let loaded = Config::load(&path).unwrap();
    assert_eq!(loaded.gateway.port, 9090);
}

#[test]
fn test_config_load_nonexistent() {
    let result = Config::load(Path::new("/nonexistent/config.json"));
    assert!(result.is_err());
}

#[test]
fn test_config_parse_invalid() {
    let result = Config::parse("not valid json");
    assert!(result.is_err());
}
