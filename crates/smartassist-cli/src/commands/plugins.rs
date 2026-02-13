//! Plugin management commands.

use clap::Args;
use smartassist_plugin_sdk::PluginLoader;

/// Plugin command arguments.
#[derive(Args)]
pub struct PluginsArgs {
    #[command(subcommand)]
    pub command: PluginsCommand,
}

/// Available plugin subcommands.
#[derive(clap::Subcommand)]
pub enum PluginsCommand {
    /// List installed plugins
    List,

    /// Show plugin details
    Info {
        /// Plugin name
        name: String,
    },

    /// Install a plugin from a file
    Install {
        /// Path to the plugin file (.so/.dylib/.dll)
        path: String,
    },
}

/// Run the plugins command.
pub async fn run(args: PluginsArgs) -> anyhow::Result<()> {
    let plugins_dir = smartassist_core::paths::plugins_dir()
        .map_err(|e| anyhow::anyhow!("Failed to get plugins dir: {}", e))?;

    match args.command {
        PluginsCommand::List => {
            let mut loader = PluginLoader::new();

            if plugins_dir.exists() {
                let loaded = unsafe { loader.load_from_dir(&plugins_dir) }
                    .map_err(|e| anyhow::anyhow!("Failed to scan plugins: {}", e))?;

                if loaded.is_empty() {
                    println!("No plugins installed.");
                    println!("\nPlugin directory: {}", plugins_dir.display());
                } else {
                    println!("{:<24} {:<12} {}", "NAME", "VERSION", "DESCRIPTION");
                    println!("{}", "-".repeat(60));
                    for meta in &loaded {
                        println!("{:<24} {:<12} {}", meta.name, meta.version, meta.description);
                    }
                    println!("\n{} plugin(s) installed.", loaded.len());
                }
            } else {
                println!("No plugins installed.");
                println!("\nPlugin directory: {}", plugins_dir.display());
            }
        }

        PluginsCommand::Info { name } => {
            let mut loader = PluginLoader::new();

            if plugins_dir.exists() {
                let _ = unsafe { loader.load_from_dir(&plugins_dir) };
            }

            match loader.get(&name) {
                Some(plugin) => {
                    let meta = plugin.metadata();
                    println!("Plugin: {}", meta.name);
                    println!("Version: {}", meta.version);
                    println!("Description: {}", meta.description);
                    if let Some(author) = &meta.author {
                        println!("Author: {}", author);
                    }
                    if let Some(license) = &meta.license {
                        println!("License: {}", license);
                    }
                    println!("Capabilities: {:?}", meta.capabilities);
                }
                None => {
                    println!("Plugin '{}' not found.", name);
                }
            }
        }

        PluginsCommand::Install { path } => {
            let source = std::path::Path::new(&path);

            if !source.exists() {
                anyhow::bail!("Plugin file not found: {}", path);
            }

            let filename = source
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid plugin path"))?;

            // Ensure plugins directory exists
            std::fs::create_dir_all(&plugins_dir)?;

            let dest = plugins_dir.join(filename);
            std::fs::copy(source, &dest)?;

            // Verify the library loads correctly
            let mut loader = PluginLoader::new();
            match unsafe { loader.load_plugin(&dest) } {
                Ok(meta) => {
                    println!("Installed plugin: {} v{}", meta.name, meta.version);
                }
                Err(e) => {
                    // Remove the invalid file
                    let _ = std::fs::remove_file(&dest);
                    anyhow::bail!("Invalid plugin file: {}", e);
                }
            }
        }
    }

    Ok(())
}
