//! Interactive read-eval-print loop.
//!
//! Provides `smartassist agent chat` -- an interactive session with
//! rustyline line editing, streaming responses, and markdown rendering.

use crate::render;
use smartassist_agent::providers::StreamEvent;
use smartassist_agent::runtime::AgentRuntime;
use smartassist_core::types::SessionKey;
use rustyline::error::ReadlineError;
use rustyline::hint::HistoryHinter;
use rustyline::{CompletionType, Config, EditMode, Editor};
use rustyline::highlight::MatchingBracketHighlighter;
use rustyline_derive::{Helper, Highlighter, Hinter, Validator};
use std::path::PathBuf;
use std::sync::Arc;
use futures::StreamExt;

/// REPL configuration.
pub struct ReplConfig {
    /// Path to history file.
    pub history_file: PathBuf,
    /// Maximum history entries.
    pub max_history: usize,
    /// Show markdown rendering.
    pub markdown_output: bool,
    /// Show token usage after each turn.
    pub show_token_usage: bool,
    /// Show tool call indicators.
    pub show_tool_calls: bool,
}

impl Default for ReplConfig {
    fn default() -> Self {
        let history_file = smartassist_core::paths::base_dir()
            .map(|d| d.join("history"))
            .unwrap_or_else(|_| PathBuf::from(".smartassist_history"));

        Self {
            history_file,
            max_history: 1000,
            markdown_output: true,
            show_token_usage: true,
            show_tool_calls: true,
        }
    }
}

/// Tab-completion helper for slash commands.
#[derive(Helper, Highlighter, Hinter, Validator)]
struct ReplHelper {
    #[rustyline(Hinter)]
    hinter: HistoryHinter,
    #[rustyline(Highlighter)]
    highlighter: MatchingBracketHighlighter,
    #[rustyline(Validator)]
    validator: rustyline::validate::MatchingBracketValidator,
}

impl rustyline::completion::Completer for ReplHelper {
    type Candidate = String;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<String>)> {
        if line.starts_with('/') {
            let commands = vec![
                "/help", "/quit", "/exit", "/clear", "/new",
                "/status", "/model", "/compact",
            ];
            let prefix = &line[..pos];
            let matches: Vec<String> = commands
                .into_iter()
                .filter(|c| c.starts_with(prefix))
                .map(|c| c.to_string())
                .collect();
            Ok((0, matches))
        } else {
            Ok((pos, Vec::new()))
        }
    }
}

/// The interactive REPL.
pub struct Repl {
    runtime: Arc<AgentRuntime>,
    session_key: SessionKey,
    config: ReplConfig,
}

impl Repl {
    /// Create a new REPL instance.
    pub fn new(
        runtime: Arc<AgentRuntime>,
        session_key: SessionKey,
        config: ReplConfig,
    ) -> Self {
        Self {
            runtime,
            session_key,
            config,
        }
    }

    /// Run the REPL loop.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        render::render_welcome(self.runtime.agent_id().as_str());

        let rl_config = Config::builder()
            .history_ignore_space(true)
            .completion_type(CompletionType::List)
            .edit_mode(EditMode::Emacs)
            .build();

        let helper = ReplHelper {
            hinter: HistoryHinter::new(),
            highlighter: MatchingBracketHighlighter::new(),
            validator: rustyline::validate::MatchingBracketValidator::new(),
        };

        let mut rl: Editor<ReplHelper, rustyline::history::FileHistory> =
            Editor::with_config(rl_config)?;
        rl.set_helper(Some(helper));

        // Load history
        let _ = rl.load_history(&self.config.history_file);

        loop {
            let prompt = console::style("> ").green().bold().to_string();
            match rl.readline(&prompt) {
                Ok(line) => {
                    let trimmed = line.trim();
                    if trimmed.is_empty() {
                        continue;
                    }

                    let _ = rl.add_history_entry(trimmed);

                    // Handle slash commands
                    if trimmed.starts_with('/') {
                        match self.handle_command(trimmed).await {
                            CommandResult::Continue => {}
                            CommandResult::Quit => break,
                        }
                        continue;
                    }

                    // Send message and stream response
                    self.send_message(trimmed).await;
                }
                Err(ReadlineError::Interrupted) => {
                    // Ctrl-C: cancel current input, not exit
                    eprintln!("{}", console::style("^C (type /quit to exit)").dim());
                    continue;
                }
                Err(ReadlineError::Eof) => {
                    // Ctrl-D: exit
                    break;
                }
                Err(err) => {
                    eprintln!("{}: {}", console::style("Error").red(), err);
                    break;
                }
            }
        }

        // Save history
        let _ = rl.save_history(&self.config.history_file);

        eprintln!("{}", console::style("Goodbye!").dim());
        Ok(())
    }

    /// Handle a slash command.
    async fn handle_command(&mut self, cmd: &str) -> CommandResult {
        match cmd.split_whitespace().next().unwrap_or("") {
            "/help" => {
                render::render_help();
                CommandResult::Continue
            }
            "/quit" | "/exit" => CommandResult::Quit,
            "/clear" => {
                eprintln!("{}", console::style("Conversation cleared.").dim());
                // Create a new session key to effectively clear history
                self.session_key = SessionKey::new(format!(
                    "{}:{}",
                    self.runtime.agent_id().as_str(),
                    smartassist_core::id::uuid()
                ));
                CommandResult::Continue
            }
            "/new" => {
                self.session_key = SessionKey::new(format!(
                    "{}:{}",
                    self.runtime.agent_id().as_str(),
                    smartassist_core::id::uuid()
                ));
                eprintln!("{}", console::style("New session started.").dim());
                CommandResult::Continue
            }
            "/status" => {
                eprintln!("  {} {}", console::style("session:").dim(), self.session_key.as_str());
                eprintln!("  {} {}", console::style("agent:").dim(), self.runtime.agent_id().as_str());
                eprintln!("  {} {}", console::style("model:").dim(), "default");
                CommandResult::Continue
            }
            "/model" => {
                eprintln!("  {} default", console::style("model:").dim());
                CommandResult::Continue
            }
            "/compact" => {
                eprintln!("{}", console::style("Context compaction not yet wired.").dim());
                CommandResult::Continue
            }
            _ => {
                eprintln!("{}: {}", console::style("Unknown command").red(), cmd);
                render::render_help();
                CommandResult::Continue
            }
        }
    }

    /// Send a message and display the streaming response.
    async fn send_message(&self, message: &str) {
        let stream = self.runtime.process_message_stream(
            self.session_key.clone(),
            message.to_string(),
        );

        let mut stream = std::pin::pin!(stream);
        let mut full_response = String::new();

        while let Some(event) = stream.next().await {
            match event {
                Ok(StreamEvent::Start) => {
                    // Response starting
                }
                Ok(StreamEvent::Text(text)) => {
                    full_response.push_str(&text);
                }
                Ok(StreamEvent::Thinking(text)) => {
                    if self.config.show_tool_calls {
                        eprintln!("{} {}", console::style("thinking:").dim(), console::style(&text).dim());
                    }
                }
                Ok(StreamEvent::ToolUse { name, .. }) => {
                    if self.config.show_tool_calls {
                        render::render_tool_status(&name, render::ToolStatus::Running);
                    }
                }
                Ok(StreamEvent::Usage(usage)) => {
                    if self.config.show_token_usage {
                        render::render_token_usage(&usage);
                    }
                }
                Ok(StreamEvent::Done) => {
                    // Stream complete
                }
                Ok(StreamEvent::Error(e)) => {
                    eprintln!("{}: {}", console::style("Error").red(), e);
                    return;
                }
                Err(e) => {
                    eprintln!("{}: {}", console::style("Error").red(), e);
                    return;
                }
            }
        }

        // Render the full response
        if !full_response.is_empty() {
            eprintln!();
            if self.config.markdown_output {
                render::render_markdown(&full_response);
            } else {
                println!("{}", full_response);
            }
            eprintln!();
        }
    }
}

/// Result of handling a slash command.
enum CommandResult {
    Continue,
    Quit,
}
