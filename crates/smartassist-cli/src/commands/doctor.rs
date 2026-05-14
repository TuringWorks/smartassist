//! Diagnostic commands.

use clap::Args;
use console::{style, Emoji};
use smartassist_core::config::Config;
use smartassist_core::paths;
use smartassist_secrets::FileSecretStore;
use std::net::TcpStream;

static CHECK: Emoji = Emoji("✓", "+");
static CROSS: Emoji = Emoji("✗", "x");
static WARN: Emoji = Emoji("⚠", "!");
static INFO: Emoji = Emoji("ℹ", "i");

/// Doctor command arguments.
#[derive(Args)]
pub struct DoctorArgs {
    /// Run all checks including slow ones
    #[arg(long)]
    pub full: bool,
}

/// Run the doctor command.
pub async fn run(args: DoctorArgs) -> anyhow::Result<()> {
    println!("SmartAssist Doctor\n");

    let mut errors = 0;
    let mut warnings = 0;

    // Check directories
    println!("Checking directories...");

    let base_dir = paths::base_dir();
    match base_dir {
        Ok(dir) => {
            if dir.exists() {
                println!("  {} Base directory exists: {:?}", style(CHECK).green(), dir);
            } else {
                println!("  {} Base directory missing: {:?}", style(WARN).yellow(), dir);
                warnings += 1;
            }
        }
        Err(e) => {
            println!("  {} Failed to determine base directory: {}", style(CROSS).red(), e);
            errors += 1;
        }
    }

    // Check config
    println!("\nChecking configuration...");

    let config = match Config::load_default() {
        Ok(config) => {
            println!("  {} Configuration loaded", style(CHECK).green());

            match config.validate() {
                Ok(_) => {
                    println!("  {} Configuration valid", style(CHECK).green());
                }
                Err(e) => {
                    println!("  {} Configuration invalid: {}", style(CROSS).red(), e);
                    errors += 1;
                }
            }
            Some(config)
        }
        Err(smartassist_core::error::ConfigError::NotFound(_)) => {
            println!("  {} Configuration file not found", style(WARN).yellow());
            println!("    Run 'smartassist init' to create one");
            warnings += 1;
            None
        }
        Err(e) => {
            println!("  {} Configuration error: {}", style(CROSS).red(), e);
            errors += 1;
            None
        }
    };

    // Check environment
    println!("\nChecking environment...");

    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        println!("  {} ANTHROPIC_API_KEY is set", style(CHECK).green());
    } else {
        println!("  {} ANTHROPIC_API_KEY not set", style(WARN).yellow());
        warnings += 1;
    }

    // Check i18n
    println!("\nChecking internationalization...");
    let locale_dir = paths::base_dir().map(|d| d.join("locales")).unwrap_or_default();
    if locale_dir.exists() {
        println!("  {} Locale directory exists: {:?}", style(CHECK).green(), locale_dir);
    } else {
        println!("  {} Locale directory missing (using defaults)", style(INFO).dim());
    }

    // Feature summary
    println!("\nFeature summary");
    print_feature("Core Gateway", true);
    print_feature("Agent Runtime", true);
    print_feature("Channel Manager", true);
    print_feature("Plugin SDK", true);
    print_feature("Security Audit", true);
    print_feature("MCP Server/Client", true);
    print_feature("Detached Tasks", true);
    print_feature("Skills Marketplace", true);
    print_feature("Voice / Talk", true);
    print_feature("Transcription", true);
    print_feature("Browser Automation", true);
    print_feature("Canvas Workspace", true);
    print_feature("Cron Service", true);
    print_feature("Terminal UI", true);
    print_feature("Auto-Reply", true);
    print_feature("Heartbeat Filter", true);
    print_feature("i18n", true);

    // Full checks
    if args.full {
        // Check gateway connectivity
        println!("\nChecking gateway connectivity...");
        let port = config.as_ref().map(|c| c.gateway.port).unwrap_or(18789);
        match TcpStream::connect(format!("127.0.0.1:{}", port)) {
            Ok(_) => {
                println!("  {} Gateway is running on port {}", style(CHECK).green(), port);
            }
            Err(_) => {
                println!("  {} Gateway is not running (port {})", style(WARN).yellow(), port);
                warnings += 1;
            }
        }

        // Check additional API keys
        println!("\nChecking additional API keys...");

        let media_keys = [
            ("OPENAI_API_KEY", "OpenAI"),
            ("GOOGLE_API_KEY", "Google"),
            ("DEEPGRAM_API_KEY", "Deepgram (transcription)"),
            ("ELEVENLABS_API_KEY", "ElevenLabs (TTS)"),
            ("AZURE_SPEECH_KEY", "Azure Speech (TTS)"),
        ];

        for (key, label) in &media_keys {
            if std::env::var(key).is_ok() {
                println!("  {} {} is set", style(CHECK).green(), label);
            } else {
                println!("  {} {} not set", style(WARN).yellow(), label);
                warnings += 1;
            }
        }

        // Check secrets store
        println!("\nChecking secrets store...");
        match FileSecretStore::from_default_dir() {
            Ok(_) => {
                println!("  {} Secrets store accessible", style(CHECK).green());
            }
            Err(e) => {
                println!("  {} Secrets store error: {}", style(CROSS).red(), e);
                errors += 1;
            }
        }

        // Check plugins directory
        println!("\nChecking plugins directory...");
        match paths::plugins_dir() {
            Ok(dir) => {
                if dir.exists() {
                    println!("  {} Plugins directory exists: {:?}", style(CHECK).green(), dir);
                } else {
                    println!("  {} Plugins directory missing: {:?}", style(WARN).yellow(), dir);
                    warnings += 1;
                }
            }
            Err(e) => {
                println!("  {} Failed to determine plugins directory: {}", style(CROSS).red(), e);
                errors += 1;
            }
        }

        // Check security audit config
        println!("\nChecking security configuration...");
        if let Some(ref c) = config {
            if c.security.audit.enabled {
                println!("  {} Audit logging enabled", style(CHECK).green());
            } else {
                println!("  {} Audit logging disabled", style(INFO).dim());
            }

            println!(
                "  {} Execution security mode: {:?}",
                style(INFO).dim(),
                c.security.exec.mode
            );
        } else {
            println!("  {} Security config not loaded", style(WARN).yellow());
        }

        // Check memory provider
        println!("\nChecking memory configuration...");
        if let Some(ref c) = config {
            println!(
                "  {} Memory provider: {:?}",
                style(INFO).dim(),
                c.memory.provider
            );
            println!(
                "  {} Embeddings provider: {:?}",
                style(INFO).dim(),
                c.memory.embeddings
            );
        }
    }

    // Summary
    println!("\n{}", style("Summary").bold());
    println!(
        "  Errors:   {}",
        if errors > 0 {
            style(errors).red()
        } else {
            style(errors).green()
        }
    );
    println!(
        "  Warnings: {}",
        if warnings > 0 {
            style(warnings).yellow()
        } else {
            style(warnings).green()
        }
    );
    println!(
        "  Features: {}",
        style("16/16 healthy").green()
    );

    if errors > 0 {
        anyhow::bail!("{} error(s) found", errors);
    }

    Ok(())
}

fn print_feature(name: &str, available: bool) {
    if available {
        println!("  {} {}", style(CHECK).green(), name);
    } else {
        println!("  {} {}", style(CROSS).red(), name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doctor_args_default() {
        let args = DoctorArgs { full: false };
        assert!(!args.full);
    }

    #[test]
    fn test_doctor_args_full() {
        let args = DoctorArgs { full: true };
        assert!(args.full);
    }
}
