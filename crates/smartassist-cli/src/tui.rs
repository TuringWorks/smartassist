//! Terminal User Interface (TUI) for SmartAssist.
//!
//! Provides a rich chat interface using `ratatui` with message history,
//! input field, and streaming response display.

use smartassist_agent::providers::StreamEvent;
use smartassist_agent::runtime::AgentRuntime;
use smartassist_core::types::SessionKey;
use std::sync::Arc;
use futures::StreamExt;

use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame, Terminal,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use std::io::stdout;
use tokio::sync::mpsc;
use unicode_segmentation::UnicodeSegmentation;

/// A chat message.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub thinking: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// TUI application state.
pub struct TuiApp {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub cursor_position: usize,
    pub scroll_offset: usize,
    pub runtime: Arc<AgentRuntime>,
    pub session_key: SessionKey,
    pub is_streaming: bool,
    pub status_message: Option<String>,
}

impl TuiApp {
    pub fn new(runtime: Arc<AgentRuntime>, session_key: SessionKey) -> Self {
        Self {
            messages: Vec::new(),
            input: String::new(),
            cursor_position: 0,
            scroll_offset: 0,
            runtime,
            session_key,
            is_streaming: false,
            status_message: Some("Press Enter to send, Esc to quit".to_string()),
        }
    }

    fn add_message(&mut self, role: MessageRole, content: String) {
        self.messages.push(ChatMessage {
            role,
            content,
            thinking: None,
        });
        self.scroll_to_bottom();
    }

    fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.messages.len();
    }

    fn move_cursor_left(&mut self) {
        let before_cursor = self.input.graphemes(true).take(self.cursor_position);
        self.cursor_position = before_cursor.count();
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    fn move_cursor_right(&mut self) {
        let count = self.input.graphemes(true).count();
        if self.cursor_position < count {
            self.cursor_position += 1;
        }
    }

    fn enter_char(&mut self, c: char) {
        let pos = self.cursor_position;
        let mut input = self.input.clone();
        let byte_pos = input.grapheme_indices(true).nth(pos).map(|(i, _)| i).unwrap_or(input.len());
        input.insert(byte_pos, c);
        self.input = input;
        self.move_cursor_right();
    }

    fn delete_char(&mut self) {
        let mut graphemes: Vec<&str> = self.input.graphemes(true).collect();
        if self.cursor_position > 0 && self.cursor_position <= graphemes.len() {
            graphemes.remove(self.cursor_position - 1);
            self.input = graphemes.into_iter().collect();
            self.move_cursor_left();
        }
    }

    fn clear_input(&mut self) {
        self.input.clear();
        self.cursor_position = 0;
    }
}

/// Run the TUI chat interface.
pub async fn run_tui(
    runtime: Arc<AgentRuntime>,
    session_key: SessionKey,
) -> anyhow::Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = TuiApp::new(runtime.clone(), session_key);

    // Channel for crossterm events from blocking reader
    let (event_tx, mut event_rx) = mpsc::channel::<crossterm::event::KeyEvent>(100);

    // Spawn blocking crossterm event reader
    let _event_handle = tokio::task::spawn_blocking(move || {
        loop {
            match event::poll(std::time::Duration::from_millis(50)) {
                Ok(true) => {
                    if let Ok(Event::Key(key)) = event::read() {
                        if key.kind != KeyEventKind::Release {
                            if event_tx.blocking_send(key).is_err() {
                                break;
                            }
                        }
                    }
                }
                Ok(false) => continue,
                Err(_) => break,
            }
        }
    });

    // Channel for streaming response chunks
    let (stream_tx, mut stream_rx) = mpsc::unbounded_channel::<StreamUpdate>();

    let mut current_stream_job: Option<tokio::task::JoinHandle<()>> = None;

    let result = loop {
        // Render
        terminal.draw(|f| draw_ui(f, &app))?;

        tokio::select! {
            biased;

            maybe_key = event_rx.recv() => {
                match maybe_key {
                    Some(key) => {
                        match handle_key_event(&mut app, key, runtime.clone(), &mut current_stream_job, stream_tx.clone()).await {
                            ControlFlow::Continue => {}
                            ControlFlow::Quit => break Ok(()),
                        }
                    }
                    None => break Ok(()),
                }
            }

            maybe_update = stream_rx.recv() => {
                match maybe_update {
                    Some(StreamUpdate::Text(text)) => {
                        if let Some(last) = app.messages.last_mut() {
                            if last.role == MessageRole::Assistant {
                                last.content.push_str(&text);
                            } else {
                                app.add_message(MessageRole::Assistant, text);
                            }
                        }
                    }
                    Some(StreamUpdate::Thinking(text)) => {
                        if let Some(last) = app.messages.last_mut() {
                            if last.role == MessageRole::Assistant {
                                last.thinking = Some(text);
                            }
                        }
                    }
                    Some(StreamUpdate::ToolUse { name }) => {
                        app.status_message = Some(format!("Running tool: {}", name));
                    }
                    Some(StreamUpdate::Done) => {
                        app.is_streaming = false;
                        app.status_message = Some("Press Enter to send, Esc to quit".to_string());
                        app.scroll_to_bottom();
                    }
                    Some(StreamUpdate::Error(e)) => {
                        app.is_streaming = false;
                        app.add_message(MessageRole::System, format!("Error: {}", e));
                        app.status_message = Some("Press Enter to send, Esc to quit".to_string());
                    }
                    None => {}
                }
            }
        }
    };

    // Cleanup
    if let Some(handle) = current_stream_job {
        handle.abort();
    }
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

#[derive(Debug)]
enum StreamUpdate {
    Text(String),
    Thinking(String),
    ToolUse { name: String },
    Done,
    Error(String),
}

enum ControlFlow {
    Continue,
    Quit,
}

async fn handle_key_event(
    app: &mut TuiApp,
    key: KeyEvent,
    runtime: Arc<AgentRuntime>,
    current_job: &mut Option<tokio::task::JoinHandle<()>>,
    stream_tx: mpsc::UnboundedSender<StreamUpdate>,
) -> ControlFlow {
    match key.code {
        KeyCode::Esc => return ControlFlow::Quit,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return ControlFlow::Quit
        }
        KeyCode::Enter => {
            let trimmed = app.input.trim();
            if trimmed.is_empty() {
                return ControlFlow::Continue;
            }
            if app.is_streaming {
                return ControlFlow::Continue;
            }

            // Handle slash commands
            if trimmed.starts_with('/') {
                match trimmed.split_whitespace().next().unwrap_or("") {
                    "/quit" | "/exit" => return ControlFlow::Quit,
                    "/clear" => {
                        app.messages.clear();
                        app.scroll_offset = 0;
                        app.clear_input();
                        return ControlFlow::Continue;
                    }
                    "/new" => {
                        app.session_key = SessionKey::new(format!(
                            "{}:{}",
                            app.runtime.agent_id().as_str(),
                            smartassist_core::id::uuid()
                        ));
                        app.messages.clear();
                        app.scroll_offset = 0;
                        app.clear_input();
                        app.status_message = Some("New session started.".to_string());
                        return ControlFlow::Continue;
                    }
                    _ => {
                        app.add_message(MessageRole::System, format!("Unknown command: {}", trimmed));
                        app.clear_input();
                        return ControlFlow::Continue;
                    }
                }
            }

            let message = trimmed.to_string();
            app.add_message(MessageRole::User, message.clone());
            app.clear_input();
            app.is_streaming = true;
            app.status_message = Some("Thinking...".to_string());

            let session = app.session_key.clone();
            let rt = runtime.clone();

            // Abort previous stream job if any
            if let Some(handle) = current_job.take() {
                handle.abort();
            }

            let handle = tokio::spawn(async move {
                let mut stream = std::pin::pin!(rt.process_message_stream(session, message));
                while let Some(event) = stream.next().await {
                    let update = match event {
                        Ok(StreamEvent::ContentDelta { delta }) => Some(StreamUpdate::Text(delta)),
                        Ok(StreamEvent::ThinkingDelta { delta }) => Some(StreamUpdate::Thinking(delta)),
                        Ok(StreamEvent::ToolUseStart { name, .. }) => Some(StreamUpdate::ToolUse { name }),
                        Ok(StreamEvent::End { .. }) => Some(StreamUpdate::Done),
                        Ok(StreamEvent::Error { message }) => Some(StreamUpdate::Error(message)),
                        Err(e) => Some(StreamUpdate::Error(e.to_string())),
                        _ => None,
                    };
                    if let Some(u) = update {
                        let _ = stream_tx.send(u);
                    }
                }
                // Ensure Done is sent if stream ended without explicit Done
                let _ = stream_tx.send(StreamUpdate::Done);
            });

            *current_job = Some(handle);
        }
        KeyCode::Char(c) => {
            app.enter_char(c);
        }
        KeyCode::Backspace => {
            app.delete_char();
        }
        KeyCode::Left => {
            app.move_cursor_left();
        }
        KeyCode::Right => {
            app.move_cursor_right();
        }
        KeyCode::Up => {
            if app.scroll_offset > 0 {
                app.scroll_offset -= 1;
            }
        }
        KeyCode::Down => {
            if app.scroll_offset < app.messages.len() {
                app.scroll_offset += 1;
            }
        }
        KeyCode::Home => {
            app.cursor_position = 0;
        }
        KeyCode::End => {
            app.cursor_position = app.input.graphemes(true).count();
        }
        _ => {}
    }

    ControlFlow::Continue
}

fn draw_ui(f: &mut Frame, app: &TuiApp) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3), Constraint::Length(1)])
        .split(f.size());

    let messages_area = chunks[0];
    let input_area = chunks[1];
    let status_area = chunks[2];

    // Messages list
    let messages_block = Block::default()
        .title("Chat")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = messages_block.inner(messages_area);
    f.render_widget(messages_block, messages_area);

    let message_texts: Vec<Line> = app
        .messages
        .iter()
        .skip(app.scroll_offset)
        .flat_map(|msg| {
            let (role_label, role_color) = match msg.role {
                MessageRole::User => ("You", Color::Green),
                MessageRole::Assistant => ("Assistant", Color::Blue),
                MessageRole::System => ("System", Color::Yellow),
            };
            let mut lines = vec![
                Line::from(vec![
                    Span::styled(
                        format!("[{}] ", role_label),
                        Style::default()
                            .fg(role_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
            ];
            // Wrap content lines
            for line in msg.content.lines() {
                lines.push(Line::from(Span::raw(line.to_string())));
            }
            if let Some(thinking) = &msg.thinking {
                lines.push(Line::from(vec![
                    Span::styled("thinking: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(thinking.clone(), Style::default().fg(Color::DarkGray)),
                ]));
            }
            lines.push(Line::from(""));
            lines
        })
        .collect();

    let messages_paragraph = Paragraph::new(Text::from(message_texts))
        .wrap(Wrap { trim: true })
        .scroll((0, 0));
    f.render_widget(messages_paragraph, inner);

    // Input area
    let input_block = Block::default()
        .title("Input")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let input_inner = input_block.inner(input_area);
    f.render_widget(input_block, input_area);

    let input_paragraph = Paragraph::new(app.input.clone())
        .wrap(Wrap { trim: true })
        .scroll((0, 0));
    f.render_widget(input_paragraph, input_inner);

    // Cursor
    let cursor_x = app
        .input
        .graphemes(true)
        .take(app.cursor_position)
        .map(|g| unicode_width::UnicodeWidthStr::width(g))
        .sum::<usize>() as u16;
    let cursor_y = input_inner.y;
    let cursor_x = input_inner.x + cursor_x.min(input_inner.width.saturating_sub(1));
    f.set_cursor(cursor_x, cursor_y);

    // Status bar
    let status = app.status_message.as_deref().unwrap_or("");
    let status_style = if app.is_streaming {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };
    let status_paragraph = Paragraph::new(status).style(status_style);
    f.render_widget(status_paragraph, status_area);
}
