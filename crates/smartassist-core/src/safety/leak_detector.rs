//! Secret pattern scanner for detecting leaked credentials and keys.
//!
//! Uses compiled regex patterns with an Aho-Corasick prefix pre-filter
//! for efficient scanning of text content.

use aho_corasick::AhoCorasick;
use regex::Regex;

use super::Severity;

/// Action to take when a secret leak is detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum LeakAction {
    /// Block the content entirely.
    Block,
    /// Redact the matched secret from output.
    Redact,
    /// Log a warning but allow the content.
    Warn,
}

/// A compiled leak detection pattern.
struct LeakPattern {
    /// Human-readable pattern name.
    name: &'static str,
    /// Compiled regex for full matching.
    regex: Regex,
    /// Severity of the leak.
    severity: Severity,
    /// Action to take on match.
    action: LeakAction,
}

/// A detected secret leak match.
#[derive(Debug, Clone)]
pub struct LeakMatch {
    /// Name of the pattern that matched.
    pub pattern_name: String,
    /// The matched text (may be truncated for display).
    pub matched_text: String,
    /// Severity of the leak.
    pub severity: Severity,
    /// Recommended action.
    pub action: LeakAction,
}

/// Secret pattern scanner that detects leaked credentials and keys.
///
/// Uses an Aho-Corasick automaton built from literal prefixes to quickly
/// skip text that cannot match any pattern, then runs full regex only
/// on segments where a prefix was found.
pub struct LeakDetector {
    /// Compiled leak patterns.
    patterns: Vec<LeakPattern>,
    /// Aho-Corasick automaton for prefix pre-filtering.
    prefix_matcher: AhoCorasick,
    /// Maps each prefix index to the indices of patterns that share it.
    prefix_to_patterns: Vec<Vec<usize>>,
    /// The literal prefixes used in the automaton (kept for diagnostics).
    #[allow(dead_code)]
    prefixes: Vec<String>,
}

/// Default leak detection patterns with their literal prefixes.
const DEFAULT_PATTERNS: &[(&str, &str, &str, Severity, LeakAction)] = &[
    // (name, regex, prefix, severity, action)
    (
        "openai_api_key",
        r"sk-(?:proj-)?[a-zA-Z0-9]{20,}",
        "sk-",
        Severity::Critical,
        LeakAction::Block,
    ),
    (
        "anthropic_api_key",
        r"sk-ant-api[a-zA-Z0-9_-]{90,}",
        "sk-ant-api",
        Severity::Critical,
        LeakAction::Block,
    ),
    (
        "aws_access_key",
        r"AKIA[0-9A-Z]{16}",
        "AKIA",
        Severity::Critical,
        LeakAction::Block,
    ),
    (
        "github_pat",
        r"ghp_[A-Za-z0-9_]{36,}",
        "ghp_",
        Severity::Critical,
        LeakAction::Block,
    ),
    (
        "github_fine_grained_pat",
        r"github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}",
        "github_pat_",
        Severity::Critical,
        LeakAction::Block,
    ),
    (
        "stripe_secret_key",
        r"sk_(?:live|test)_[a-zA-Z0-9]{24,}",
        "sk_",
        Severity::Critical,
        LeakAction::Block,
    ),
    (
        "google_api_key",
        r"AIza[0-9A-Za-z_-]{35}",
        "AIza",
        Severity::High,
        LeakAction::Block,
    ),
    (
        "slack_token",
        r"xox[baprs]-[0-9a-zA-Z-]{10,}",
        "xox",
        Severity::High,
        LeakAction::Block,
    ),
    (
        "pem_private_key",
        r"-----BEGIN\s+(?:RSA\s+)?PRIVATE\s+KEY-----",
        "-----BEGIN",
        Severity::Critical,
        LeakAction::Block,
    ),
    (
        "ssh_private_key",
        r"-----BEGIN\s+(?:OPENSSH|EC|DSA)\s+PRIVATE\s+KEY-----",
        "-----BEGIN",
        Severity::Critical,
        LeakAction::Block,
    ),
    (
        "bearer_token",
        r"Bearer\s+[a-zA-Z0-9_-]{20,}",
        "Bearer",
        Severity::High,
        LeakAction::Redact,
    ),
    (
        "auth_header",
        r"(?i)authorization:\s*[a-zA-Z]+\s+[a-zA-Z0-9_-]{20,}",
        "uthorization:",
        Severity::High,
        LeakAction::Redact,
    ),
    (
        "high_entropy_hex",
        r"\b[a-fA-F0-9]{64}\b",
        "",
        Severity::Medium,
        LeakAction::Warn,
    ),
];

impl LeakDetector {
    /// Create a new leak detector with default patterns.
    pub fn new() -> Self {
        let mut patterns = Vec::new();
        let mut prefix_map: Vec<(String, usize)> = Vec::new();

        for (i, (name, regex_str, prefix, severity, action)) in
            DEFAULT_PATTERNS.iter().enumerate()
        {
            let regex = Regex::new(regex_str)
                .unwrap_or_else(|e| panic!("Invalid regex for pattern '{}': {}", name, e));

            patterns.push(LeakPattern {
                name,
                regex,
                severity: *severity,
                action: *action,
            });

            if !prefix.is_empty() {
                prefix_map.push((prefix.to_string(), i));
            }
        }

        // Build Aho-Corasick from unique prefixes
        let mut unique_prefixes: Vec<String> = Vec::new();
        let mut prefix_to_patterns: Vec<Vec<usize>> = Vec::new();

        for (prefix, pattern_idx) in &prefix_map {
            if let Some(pos) = unique_prefixes.iter().position(|p| p == prefix) {
                prefix_to_patterns[pos].push(*pattern_idx);
            } else {
                unique_prefixes.push(prefix.clone());
                prefix_to_patterns.push(vec![*pattern_idx]);
            }
        }

        // Use MatchKind::Standard with overlapping iteration so shorter
        // prefixes don't shadow longer ones that start at the same position.
        let prefix_matcher = AhoCorasick::builder()
            .ascii_case_insensitive(true)
            .build(&unique_prefixes)
            .expect("Failed to build Aho-Corasick prefix matcher");

        Self {
            patterns,
            prefix_matcher,
            prefix_to_patterns,
            prefixes: unique_prefixes,
        }
    }

    /// Scan text for secret leaks. Returns all matches with details.
    pub fn scan(&self, text: &str) -> Vec<LeakMatch> {
        let mut matches = Vec::new();
        let mut checked_patterns = vec![false; self.patterns.len()];

        // Phase 1: Use prefix matcher to identify candidate patterns.
        // Use overlapping iteration so shorter prefixes don't shadow longer
        // ones that start at the same position (e.g., "sk-" vs "sk-ant-api").
        let mut state = aho_corasick::automaton::OverlappingState::start();
        loop {
            self.prefix_matcher
                .find_overlapping(text, &mut state);
            let mat = match state.get_match() {
                Some(m) => m,
                None => break,
            };
            let prefix_idx = mat.pattern().as_usize();
            for &pattern_idx in &self.prefix_to_patterns[prefix_idx] {
                if !checked_patterns[pattern_idx] {
                    checked_patterns[pattern_idx] = true;
                    let pattern = &self.patterns[pattern_idx];
                    for regex_match in pattern.regex.find_iter(text) {
                        matches.push(LeakMatch {
                            pattern_name: pattern.name.to_string(),
                            matched_text: regex_match.as_str().to_string(),
                            severity: pattern.severity,
                            action: pattern.action,
                        });
                    }
                }
            }
        }

        // Phase 2: Check patterns without prefixes (e.g., high entropy hex)
        for (i, pattern) in self.patterns.iter().enumerate() {
            if checked_patterns[i] {
                continue;
            }
            // Patterns with no prefix in DEFAULT_PATTERNS have empty prefix
            let has_prefix = DEFAULT_PATTERNS
                .get(i)
                .map(|(_, _, p, _, _)| !p.is_empty())
                .unwrap_or(false);
            if has_prefix {
                continue;
            }
            for regex_match in pattern.regex.find_iter(text) {
                matches.push(LeakMatch {
                    pattern_name: pattern.name.to_string(),
                    matched_text: regex_match.as_str().to_string(),
                    severity: pattern.severity,
                    action: pattern.action,
                });
            }
        }

        matches
    }

    /// Scan text and return cleaned output plus a list of matches.
    ///
    /// - Blocked matches: replaced with `[BLOCKED: <pattern>]`
    /// - Redacted matches: replaced with `[REDACTED]`
    /// - Warned matches: left in place (only logged)
    pub fn scan_and_clean(&self, text: &str) -> (String, Vec<LeakMatch>) {
        let matches = self.scan(text);
        if matches.is_empty() {
            return (text.to_string(), matches);
        }

        let mut result = text.to_string();

        // Process blocked matches first (highest priority), then redacted
        for leak in &matches {
            match leak.action {
                LeakAction::Block => {
                    result = result.replace(
                        &leak.matched_text,
                        &format!("[BLOCKED: {}]", leak.pattern_name),
                    );
                }
                LeakAction::Redact => {
                    result = result.replace(&leak.matched_text, "[REDACTED]");
                }
                LeakAction::Warn => {
                    // Warned matches are left in place; callers should log them
                }
            }
        }

        (result, matches)
    }

    /// Mask a secret for safe display: show first 4 and last 4 characters,
    /// mask the middle with asterisks.
    ///
    /// Returns the full string if it is 8 characters or shorter.
    pub fn mask_secret(secret: &str) -> String {
        if secret.len() <= 8 {
            return "*".repeat(secret.len());
        }
        let prefix = &secret[..4];
        let suffix = &secret[secret.len() - 4..];
        format!("{}{}{}",  prefix, "*".repeat(secret.len() - 8), suffix)
    }
}

impl Default for LeakDetector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_key_detection() {
        let detector = LeakDetector::new();
        let text = "My API key is sk-abcdefghijklmnopqrstuvwx";
        let matches = detector.scan(text);
        assert!(
            matches.iter().any(|m| m.pattern_name == "openai_api_key"),
            "Should detect OpenAI API key"
        );
        assert!(matches.iter().any(|m| m.severity == Severity::Critical));
    }

    #[test]
    fn test_openai_proj_key_detection() {
        let detector = LeakDetector::new();
        let text = "key: sk-proj-abcdefghijklmnopqrstuvwx";
        let matches = detector.scan(text);
        assert!(
            matches.iter().any(|m| m.pattern_name == "openai_api_key"),
            "Should detect OpenAI project key"
        );
    }

    #[test]
    fn test_anthropic_key_detection() {
        let detector = LeakDetector::new();
        // Anthropic key has 90+ chars after prefix
        let key_body = "a".repeat(95);
        let text = format!("sk-ant-api{}", key_body);
        let matches = detector.scan(&text);
        assert!(
            matches
                .iter()
                .any(|m| m.pattern_name == "anthropic_api_key"),
            "Should detect Anthropic API key"
        );
    }

    #[test]
    fn test_aws_key_detection() {
        let detector = LeakDetector::new();
        let text = "AWS key: AKIAIOSFODNN7EXAMPLE";
        let matches = detector.scan(text);
        assert!(
            matches
                .iter()
                .any(|m| m.pattern_name == "aws_access_key"),
            "Should detect AWS access key"
        );
    }

    #[test]
    fn test_github_pat_detection() {
        let detector = LeakDetector::new();
        let token = format!("ghp_{}", "a".repeat(40));
        let text = format!("token: {}", token);
        let matches = detector.scan(&text);
        assert!(
            matches.iter().any(|m| m.pattern_name == "github_pat"),
            "Should detect GitHub PAT"
        );
    }

    #[test]
    fn test_github_fine_grained_pat_detection() {
        let detector = LeakDetector::new();
        let token = format!("github_pat_{}_{}", "a".repeat(22), "b".repeat(59));
        let text = format!("token: {}", token);
        let matches = detector.scan(&text);
        assert!(
            matches
                .iter()
                .any(|m| m.pattern_name == "github_fine_grained_pat"),
            "Should detect GitHub fine-grained PAT"
        );
    }

    #[test]
    fn test_stripe_key_detection() {
        let detector = LeakDetector::new();
        let key = format!("sk_live_{}", "a".repeat(30));
        let text = format!("stripe: {}", key);
        let matches = detector.scan(&text);
        assert!(
            matches
                .iter()
                .any(|m| m.pattern_name == "stripe_secret_key"),
            "Should detect Stripe secret key"
        );
    }

    #[test]
    fn test_google_api_key_detection() {
        let detector = LeakDetector::new();
        let key = format!("AIza{}", "a".repeat(35));
        let text = format!("google key: {}", key);
        let matches = detector.scan(&text);
        assert!(
            matches
                .iter()
                .any(|m| m.pattern_name == "google_api_key"),
            "Should detect Google API key"
        );
    }

    #[test]
    fn test_slack_token_detection() {
        let detector = LeakDetector::new();
        let text = "slack: xoxb-12345678901-abcdefghij";
        let matches = detector.scan(text);
        assert!(
            matches.iter().any(|m| m.pattern_name == "slack_token"),
            "Should detect Slack token"
        );
    }

    #[test]
    fn test_pem_private_key_detection() {
        let detector = LeakDetector::new();
        let text = "-----BEGIN PRIVATE KEY-----\nMIIEvgIBADANBg...";
        let matches = detector.scan(text);
        assert!(
            matches
                .iter()
                .any(|m| m.pattern_name == "pem_private_key"),
            "Should detect PEM private key"
        );
    }

    #[test]
    fn test_pem_rsa_private_key_detection() {
        let detector = LeakDetector::new();
        let text = "-----BEGIN RSA PRIVATE KEY-----";
        let matches = detector.scan(text);
        assert!(
            matches
                .iter()
                .any(|m| m.pattern_name == "pem_private_key"),
            "Should detect RSA PEM private key"
        );
    }

    #[test]
    fn test_ssh_private_key_detection() {
        let detector = LeakDetector::new();
        let text = "-----BEGIN OPENSSH PRIVATE KEY-----";
        let matches = detector.scan(text);
        assert!(
            matches
                .iter()
                .any(|m| m.pattern_name == "ssh_private_key"),
            "Should detect SSH private key"
        );
    }

    #[test]
    fn test_bearer_token_detection() {
        let detector = LeakDetector::new();
        let token = format!("Bearer {}", "a".repeat(30));
        let matches = detector.scan(&token);
        assert!(
            matches.iter().any(|m| m.pattern_name == "bearer_token"),
            "Should detect Bearer token"
        );
        assert!(matches.iter().any(|m| m.action == LeakAction::Redact));
    }

    #[test]
    fn test_auth_header_detection() {
        let detector = LeakDetector::new();
        let text = format!("Authorization: Bearer {}", "a".repeat(25));
        let matches = detector.scan(&text);
        assert!(
            matches.iter().any(|m| m.pattern_name == "auth_header"),
            "Should detect Authorization header"
        );
    }

    #[test]
    fn test_high_entropy_hex_detection() {
        let detector = LeakDetector::new();
        let hex_str = "a".repeat(64);
        let text = format!("hash: {}", hex_str);
        let matches = detector.scan(&text);
        assert!(
            matches
                .iter()
                .any(|m| m.pattern_name == "high_entropy_hex"),
            "Should detect high entropy hex string"
        );
        assert!(matches.iter().any(|m| m.action == LeakAction::Warn));
    }

    #[test]
    fn test_scan_and_clean_block() {
        let detector = LeakDetector::new();
        let text = "My key is sk-abcdefghijklmnopqrstuvwx";
        let (cleaned, matches) = detector.scan_and_clean(text);
        assert!(!matches.is_empty());
        assert!(
            cleaned.contains("[BLOCKED:"),
            "Blocked secrets should be replaced"
        );
        assert!(
            !cleaned.contains("sk-abcdefghijklmnopqrstuvwx"),
            "Original secret should not appear"
        );
    }

    #[test]
    fn test_scan_and_clean_redact() {
        let detector = LeakDetector::new();
        let token = format!("Bearer {}", "x".repeat(30));
        let text = format!("auth: {}", token);
        let (cleaned, matches) = detector.scan_and_clean(&text);
        assert!(!matches.is_empty());
        assert!(
            cleaned.contains("[REDACTED]"),
            "Redacted secrets should show [REDACTED]"
        );
    }

    #[test]
    fn test_scan_and_clean_no_leaks() {
        let detector = LeakDetector::new();
        let text = "Hello, this is a normal message.";
        let (cleaned, matches) = detector.scan_and_clean(text);
        assert!(matches.is_empty());
        assert_eq!(cleaned, text);
    }

    #[test]
    fn test_mask_secret() {
        assert_eq!(
            LeakDetector::mask_secret("sk-test1234abcd"),
            "sk-t*******abcd"
        );
    }

    #[test]
    fn test_mask_short_secret() {
        assert_eq!(LeakDetector::mask_secret("short"), "*****");
    }

    #[test]
    fn test_mask_boundary_secret() {
        // Exactly 8 chars
        assert_eq!(LeakDetector::mask_secret("12345678"), "********");
        // 9 chars: show first 4 + 1 masked + last 4
        assert_eq!(LeakDetector::mask_secret("123456789"), "1234*6789");
    }
}
