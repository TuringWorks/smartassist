//! Internationalization (i18n) support.
//!
//! Provides a lightweight translation system backed by YAML locale files.
//! Translations are loaded from `../locales/<locale>.yml` at runtime and
//! cached in a `HashMap` behind a `RwLock`.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::RwLock;

/// Global in-memory translation store.
static TRANSLATIONS: Lazy<RwLock<HashMap<String, String>>> = Lazy::new(|| {
    RwLock::new(load_locale("en").unwrap_or_default())
});

/// Load a locale file from `../locales/<locale>.yml`.
fn load_locale(locale: &str) -> Option<HashMap<String, String>> {
    let manifest_dir = std::env!("CARGO_MANIFEST_DIR");
    let path = std::path::Path::new(manifest_dir).join("locales").join(format!("{}.yml", locale));
    let contents = std::fs::read_to_string(&path).ok()?;
    let map: HashMap<String, HashMap<String, String>> = serde_yaml::from_str(&contents).ok()?;
    map.into_iter().next().map(|(_, v)| v)
}

/// Set the active locale by loading its translation file.
/// Falls back silently if the file is missing or malformed.
pub fn set_locale(locale: &str) {
    if let Some(map) = load_locale(locale) {
        if let Ok(mut store) = TRANSLATIONS.write() {
            *store = map;
        }
    }
}

/// Get a translated string by key.
///
/// Returns the key itself as a fallback when no translation is found.
///
/// # Example
/// ```
/// use smartassist_core::i18n;
/// let msg = i18n::t("welcome");
/// assert!(msg.contains("SmartAssist"));
/// ```
pub fn t(key: &str) -> String {
    if let Ok(store) = TRANSLATIONS.read() {
        if let Some(value) = store.get(key) {
            return value.clone();
        }
    }
    key.to_string()
}

/// Get a translated string with a single interpolated variable.
///
/// Replaces `%{var}` in the translation with `value`.
///
/// # Example
/// ```
/// use smartassist_core::i18n;
/// let msg = i18n::t_with("error_agent_not_found", "agent", "mybot");
/// assert!(msg.contains("mybot"));
/// ```
pub fn t_with(key: &str, var: &str, value: &str) -> String {
    let mut text = t(key);
    text = text.replace(&format!("%{{{}}}", var), value);
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_welcome() {
        let msg = t("welcome");
        assert!(msg.contains("SmartAssist"), "got: {}", msg);
    }

    #[test]
    fn test_translate_error_agent_not_found() {
        let msg = t_with("error_agent_not_found", "agent", "test-agent");
        assert!(msg.contains("test-agent"), "got: {}", msg);
    }

    #[test]
    fn test_translate_config_not_found() {
        let msg = t("error_config_not_found");
        assert!(!msg.is_empty());
    }

    #[test]
    fn test_translate_missing_key_returns_key() {
        let msg = t("nonexistent_key_xyz");
        assert_eq!(msg, "nonexistent_key_xyz");
    }
}
