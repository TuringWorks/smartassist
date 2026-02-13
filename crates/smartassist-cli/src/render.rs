//! Terminal rendering utilities.
//!
//! Provides markdown rendering, tool status display, and token usage formatting.

use console::style;
use smartassist_core::types::TokenUsage;

/// Render markdown text to the terminal.
pub fn render_markdown(text: &str) {
    // Use termimad for markdown rendering
    let skin = termimad::MadSkin::default();
    skin.print_text(text);
}

/// Tool execution status.
pub enum ToolStatus {
    /// Tool is running.
    Running,
    /// Tool completed successfully.
    Done,
    /// Tool failed.
    Failed,
}

/// Render a tool status indicator.
pub fn render_tool_status(name: &str, status: ToolStatus) {
    let indicator = match status {
        ToolStatus::Running => style("~").yellow().to_string(),
        ToolStatus::Done => style("*").green().to_string(),
        ToolStatus::Failed => style("x").red().to_string(),
    };
    eprintln!("  {} {}", indicator, style(name).dim());
}

/// Render token usage in a compact format.
pub fn render_token_usage(usage: &TokenUsage) {
    eprintln!(
        "  {} input: {} | output: {} | total: {}",
        style("tokens").dim(),
        style(usage.input).cyan(),
        style(usage.output).cyan(),
        style(usage.total()).cyan(),
    );
}

/// Render the approval prompt for a tool call.
/// Returns true if approved, false if denied.
pub fn render_approval_prompt(tool: &str, args: &serde_json::Value) -> bool {
    eprintln!();
    eprintln!(
        "{} Tool {} wants to execute:",
        style("?").yellow().bold(),
        style(tool).bold(),
    );

    // Pretty-print args (truncated)
    let args_str = serde_json::to_string_pretty(args).unwrap_or_else(|_| args.to_string());
    let truncated = if args_str.len() > 500 {
        format!("{}...", &args_str[..500])
    } else {
        args_str
    };
    eprintln!("{}", style(&truncated).dim());
    eprintln!();

    // Prompt
    eprint!("{} ", style("Allow? [y/N/a(lways)]").bold());

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
        let trimmed = input.trim().to_lowercase();
        matches!(trimmed.as_str(), "y" | "yes" | "a" | "always")
    } else {
        false
    }
}

/// Print the welcome banner for the REPL.
pub fn render_welcome(model: &str) {
    eprintln!(
        "{} {} {}",
        style("smartassist").bold().cyan(),
        style("interactive").dim(),
        style(format!("({})", model)).dim(),
    );
    eprintln!(
        "{}",
        style("Type /help for commands, /quit to exit.").dim()
    );
    eprintln!();
}

/// Print the help message.
pub fn render_help() {
    eprintln!("{}", style("Available commands:").bold());
    eprintln!("  {}  - Show this help", style("/help").cyan());
    eprintln!("  {}  - Exit the REPL", style("/quit").cyan());
    eprintln!("  {}  - Exit the REPL", style("/exit").cyan());
    eprintln!("  {} - Clear conversation history", style("/clear").cyan());
    eprintln!("  {}   - Start a new session", style("/new").cyan());
    eprintln!("  {}  - Show session status", style("/status").cyan());
    eprintln!("  {} - Show or switch model", style("/model").cyan());
    eprintln!("  {} - Trigger context compaction", style("/compact").cyan());
    eprintln!();
}
