//! Input validation for safety checks.
//!
//! Validates text inputs against configurable limits including length,
//! null bytes, whitespace ratio, and character repetition.

use serde::{Deserialize, Serialize};

use crate::error::SecurityError;

/// Configuration for input validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorConfig {
    /// Maximum allowed input length in bytes.
    pub max_length: usize,
    /// Minimum allowed input length in bytes.
    pub min_length: usize,
    /// Whether to check for null bytes.
    pub check_null_bytes: bool,
    /// Whether to check whitespace ratio.
    pub check_whitespace_ratio: bool,
    /// Maximum allowed ratio of whitespace characters (0.0 - 1.0).
    pub max_whitespace_ratio: f64,
    /// Whether to check for excessive character repetition.
    pub check_repetition: bool,
    /// Maximum number of consecutive identical characters allowed.
    pub max_repetition: usize,
}

impl Default for ValidatorConfig {
    fn default() -> Self {
        Self {
            max_length: 100 * 1024, // 100KB
            min_length: 0,
            check_null_bytes: true,
            check_whitespace_ratio: true,
            max_whitespace_ratio: 0.9,
            check_repetition: true,
            max_repetition: 20,
        }
    }
}

/// Input validator that checks text content against configurable rules.
pub struct Validator {
    config: ValidatorConfig,
}

impl Validator {
    /// Create a new validator with the given configuration.
    pub fn new(config: ValidatorConfig) -> Self {
        Self { config }
    }

    /// Validate a text string against all configured checks.
    pub fn validate(&self, text: &str) -> Result<(), SecurityError> {
        // Check minimum length
        if text.len() < self.config.min_length {
            return Err(SecurityError::InputValidation {
                reason: format!(
                    "Input too short: {} bytes (minimum: {})",
                    text.len(),
                    self.config.min_length
                ),
            });
        }

        // Check maximum length
        if text.len() > self.config.max_length {
            return Err(SecurityError::InputValidation {
                reason: format!(
                    "Input too long: {} bytes (maximum: {})",
                    text.len(),
                    self.config.max_length
                ),
            });
        }

        // Check for null bytes
        if self.config.check_null_bytes && text.contains('\0') {
            return Err(SecurityError::InputValidation {
                reason: "Input contains null bytes".to_string(),
            });
        }

        // Check whitespace ratio
        if self.config.check_whitespace_ratio && !text.is_empty() {
            let whitespace_count = text.chars().filter(|c| c.is_whitespace()).count();
            let total_chars = text.chars().count();
            let ratio = whitespace_count as f64 / total_chars as f64;

            if ratio > self.config.max_whitespace_ratio {
                return Err(SecurityError::InputValidation {
                    reason: format!(
                        "Whitespace ratio too high: {:.1}% (maximum: {:.1}%)",
                        ratio * 100.0,
                        self.config.max_whitespace_ratio * 100.0
                    ),
                });
            }
        }

        // Check for excessive character repetition
        if self.config.check_repetition {
            if let Some(repeated_char) = find_excessive_repetition(text, self.config.max_repetition)
            {
                return Err(SecurityError::InputValidation {
                    reason: format!(
                        "Excessive character repetition: '{}' repeated more than {} times consecutively",
                        repeated_char, self.config.max_repetition
                    ),
                });
            }
        }

        Ok(())
    }

    /// Recursively validate all string values in a JSON value.
    pub fn validate_json(&self, value: &serde_json::Value) -> Result<(), SecurityError> {
        match value {
            serde_json::Value::String(s) => self.validate(s),
            serde_json::Value::Array(arr) => {
                for item in arr {
                    self.validate_json(item)?;
                }
                Ok(())
            }
            serde_json::Value::Object(map) => {
                for (_key, val) in map {
                    self.validate_json(val)?;
                }
                Ok(())
            }
            // Numbers, bools, and null don't need validation
            _ => Ok(()),
        }
    }
}

impl Default for Validator {
    fn default() -> Self {
        Self::new(ValidatorConfig::default())
    }
}

/// Find the first character that repeats more than `max` consecutive times.
/// Returns `None` if no excessive repetition is found.
fn find_excessive_repetition(text: &str, max: usize) -> Option<char> {
    let mut chars = text.chars();
    let mut prev = match chars.next() {
        Some(c) => c,
        None => return None,
    };
    let mut count = 1usize;

    for c in chars {
        if c == prev {
            count += 1;
            if count > max {
                return Some(c);
            }
        } else {
            prev = c;
            count = 1;
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_input() {
        let validator = Validator::default();
        assert!(validator.validate("Hello, world!").is_ok());
    }

    #[test]
    fn test_empty_input_default() {
        let validator = Validator::default();
        // Default min_length is 0, so empty is allowed
        assert!(validator.validate("").is_ok());
    }

    #[test]
    fn test_input_too_short() {
        let config = ValidatorConfig {
            min_length: 5,
            ..Default::default()
        };
        let validator = Validator::new(config);
        let result = validator.validate("hi");
        assert!(result.is_err());
        match result.unwrap_err() {
            SecurityError::InputValidation { reason } => {
                assert!(reason.contains("too short"));
            }
            e => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_input_too_long() {
        let config = ValidatorConfig {
            max_length: 10,
            ..Default::default()
        };
        let validator = Validator::new(config);
        let result = validator.validate("This is way too long for the limit");
        assert!(result.is_err());
        match result.unwrap_err() {
            SecurityError::InputValidation { reason } => {
                assert!(reason.contains("too long"));
            }
            e => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_null_bytes() {
        let validator = Validator::default();
        let result = validator.validate("hello\x00world");
        assert!(result.is_err());
        match result.unwrap_err() {
            SecurityError::InputValidation { reason } => {
                assert!(reason.contains("null bytes"));
            }
            e => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_null_bytes_disabled() {
        let config = ValidatorConfig {
            check_null_bytes: false,
            ..Default::default()
        };
        let validator = Validator::new(config);
        assert!(validator.validate("hello\x00world").is_ok());
    }

    #[test]
    fn test_high_whitespace_ratio() {
        let validator = Validator::default();
        // 95% whitespace
        let text = format!("a{}", " ".repeat(19));
        let result = validator.validate(&text);
        assert!(result.is_err());
        match result.unwrap_err() {
            SecurityError::InputValidation { reason } => {
                assert!(reason.contains("Whitespace ratio"));
            }
            e => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_acceptable_whitespace_ratio() {
        let validator = Validator::default();
        // Normal text has moderate whitespace
        assert!(validator.validate("Hello world, this is a test.").is_ok());
    }

    #[test]
    fn test_excessive_repetition() {
        let validator = Validator::default();
        // 21 consecutive 'a' characters (exceeds default max of 20)
        let text = "a".repeat(21);
        let result = validator.validate(&text);
        assert!(result.is_err());
        match result.unwrap_err() {
            SecurityError::InputValidation { reason } => {
                assert!(reason.contains("repetition"));
            }
            e => panic!("Unexpected error: {:?}", e),
        }
    }

    #[test]
    fn test_acceptable_repetition() {
        let validator = Validator::default();
        // Exactly 20 is allowed
        let text = "a".repeat(20);
        assert!(validator.validate(&text).is_ok());
    }

    #[test]
    fn test_validate_json_strings() {
        let validator = Validator::default();
        let json = serde_json::json!({
            "name": "test",
            "value": "hello world"
        });
        assert!(validator.validate_json(&json).is_ok());
    }

    #[test]
    fn test_validate_json_nested() {
        let validator = Validator::default();
        let json = serde_json::json!({
            "outer": {
                "inner": "valid text",
                "list": ["also", "valid"]
            }
        });
        assert!(validator.validate_json(&json).is_ok());
    }

    #[test]
    fn test_validate_json_with_invalid_string() {
        let config = ValidatorConfig {
            max_length: 10,
            ..Default::default()
        };
        let validator = Validator::new(config);
        let json = serde_json::json!({
            "name": "short",
            "value": "this string is way too long for the limit"
        });
        assert!(validator.validate_json(&json).is_err());
    }

    #[test]
    fn test_validate_json_null_bytes_in_nested() {
        let validator = Validator::default();
        let json = serde_json::json!({
            "array": ["good", "has\x00null"]
        });
        assert!(validator.validate_json(&json).is_err());
    }

    #[test]
    fn test_validate_json_non_string_types() {
        let validator = Validator::default();
        let json = serde_json::json!({
            "number": 42,
            "bool": true,
            "null": null
        });
        assert!(validator.validate_json(&json).is_ok());
    }

    #[test]
    fn test_find_excessive_repetition_none() {
        assert!(find_excessive_repetition("abcdef", 20).is_none());
    }

    #[test]
    fn test_find_excessive_repetition_boundary() {
        // Exactly at max should be fine
        let text = "a".repeat(20);
        assert!(find_excessive_repetition(&text, 20).is_none());

        // One over max triggers
        let text = "a".repeat(21);
        assert_eq!(find_excessive_repetition(&text, 20), Some('a'));
    }

    #[test]
    fn test_find_excessive_repetition_empty() {
        assert!(find_excessive_repetition("", 20).is_none());
    }
}
