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

    match Config::load_default() {
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
        }
        Err(smartassist_core::error::ConfigError::NotFound(_)) => {
            println!("  {} Configuration file not found", style(WARN).yellow());
            println!("    Run 'smartassist config init' to create one");
            warnings += 1;
        }
        Err(e) => {
            println!("  {} Configuration error: {}", style(CROSS).red(), e);
            errors += 1;
        }
    }

    // Check environment
    println!("\nChecking environment...");

    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        println!("  {} ANTHROPIC_API_KEY is set", style(CHECK).green());
    } else {
        println!("  {} ANTHROPIC_API_KEY not set", style(WARN).yellow());
        warnings += 1;
    }

    // Full checks: gateway connectivity, additional API keys, secrets store, plugins dir
    if args.full {
        // Check gateway connectivity
        println!("\nChecking gateway connectivity...");
        let port = match Config::load_default() {
            Ok(c) => c.gateway.port,
            Err(_) => 18789, // default port
        };
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

        if std::env::var("OPENAI_API_KEY").is_ok() {
            println!("  {} OPENAI_API_KEY is set", style(CHECK).green());
        } else {
            println!("  {} OPENAI_API_KEY not set", style(WARN).yellow());
            warnings += 1;
        }

        if std::env::var("GOOGLE_API_KEY").is_ok() {
            println!("  {} GOOGLE_API_KEY is set", style(CHECK).green());
        } else {
            println!("  {} GOOGLE_API_KEY not set", style(WARN).yellow());
            warnings += 1;
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
    }

    // Summary
    println!("\n{}", style("Summary").bold());
    println!("  Errors: {}", if errors > 0 { style(errors).red() } else { style(errors).green() });
    println!("  Warnings: {}", if warnings > 0 { style(warnings).yellow() } else { style(warnings).green() });

    if errors > 0 {
        anyhow::bail!("{} error(s) found", errors);
    }

    Ok(())
}
