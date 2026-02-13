//! Secret management commands.
//!
//! Provides `smartassist secrets set|get|list|delete` subcommands for
//! managing encrypted secrets via the `smartassist-secrets` crate.

use clap::Args;
use smartassist_secrets::{FileSecretStore, SecretStore};

/// Secrets command arguments.
#[derive(Args)]
pub struct SecretsArgs {
    #[command(subcommand)]
    pub command: SecretsCommand,
}

#[derive(clap::Subcommand)]
pub enum SecretsCommand {
    /// Store a secret (prompts for value)
    Set {
        /// Secret name (alphanumeric, underscore, hyphen)
        name: String,

        /// Secret value (if omitted, prompts for hidden input)
        #[arg(long)]
        value: Option<String>,
    },

    /// Retrieve and print a decrypted secret
    Get {
        /// Secret name
        name: String,
    },

    /// List all stored secrets (names only)
    List,

    /// Delete a secret
    Delete {
        /// Secret name
        name: String,
    },
}

/// Run the secrets command.
pub async fn run(args: SecretsArgs) -> anyhow::Result<()> {
    let store = FileSecretStore::from_default_dir()
        .map_err(|e| anyhow::anyhow!("Failed to initialize secret store: {}", e))?;

    match args.command {
        SecretsCommand::Set { name, value } => {
            let secret_value = match value {
                Some(v) => v,
                None => {
                    let prompt = format!("Enter value for '{name}': ");
                    rpassword::prompt_password(prompt)
                        .map_err(|e| anyhow::anyhow!("Failed to read secret: {}", e))?
                }
            };

            if secret_value.is_empty() {
                anyhow::bail!("Secret value must not be empty");
            }

            store
                .set(&name, &secret_value)
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!("Secret '{}' stored successfully.", name);
        }

        SecretsCommand::Get { name } => {
            let secret = store
                .get(&name)
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!("{}", secret.expose());
        }

        SecretsCommand::List => {
            let refs = store
                .list()
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            if refs.is_empty() {
                println!("No secrets stored.");
            } else {
                println!("{:<32} {}", "NAME", "CREATED");
                println!("{}", "-".repeat(56));
                for r in &refs {
                    println!(
                        "{:<32} {}",
                        r.name,
                        r.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                    );
                }
                println!("\n{} secret(s) total.", refs.len());
            }
        }

        SecretsCommand::Delete { name } => {
            store
                .delete(&name)
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!("Secret '{}' deleted.", name);
        }
    }

    Ok(())
}
