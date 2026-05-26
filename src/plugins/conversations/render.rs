//! Conversations plugin rendering
//!
//! This module provides the UI rendering for the Conversations plugin,
//! implementing a two-pane layout with sessions list and conversation view.

use crate::core::models::Theme;
use crate::core::models::conversation::{ContentBlock, Message, Role};
use crate::plugins::conversations::state::SessionInfo;
use crate::plugins::conversations::state::{ConversationView, PluginState};
use crate::theme::UiElement;
use crate::theme::style_for_ui_element;
use chrono::{DateTime, Local};
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    StatefulWidget, Widget, Wrap,
};
use unicode_width::UnicodeWidthStr;

/// Renderer for the Conversations plugin UI
#[derive(Clone, Debug, Default)]
pub struct ConversationsRenderer {
    /// Whether to use compact mode (smaller padding)
    compact: bool,
    /// Show timestamps in messages
    show_timestamps: bool,
    /// Maximum width for message wrapping
    wrap_width: Option<u16>,
}

impl ConversationsRenderer {
    /// Create a new renderer with default settings
    pub fn new() -> Self {
        Self {
            compact: false,
            show_timestamps: true,
            wrap_width: None,
        }
    }

    /// Create a compact renderer
    pub fn compact() -> Self {
        Self {
            compact: true,
            show_timestamps: false,
            wrap_width: None,
        }
    }

    /// Set compact mode
    pub fn with_compact(mut self, compact: bool) -> Self {
        self.compact = compact;
        self
    }

    /// Set timestamp display
    pub fn with_timestamps(mut self, show: bool) -> Self {
        self.show_timestamps = show;
        self
    }

    /// Set wrap width
    pub fn with_wrap_width(mut self, width: u16) -> Self {
        self.wrap_width = Some(width);
        self
    }

    /// Render the plugin UI
    pub fn render(
        &self,
        state: &PluginState,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        focused: bool,
    ) {
        // Clear the area with background color
        let bg_style = style_for_ui_element(theme, UiElement::Background);
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(bg_style);
                }
            }
        }

        // Split area into sidebar (sessions) and main (conversation)
        let sidebar_width = if self.compact { 30 } else { 40 };
        let constraints = [Constraint::Min(sidebar_width), Constraint::Percentage(70)];
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        let sidebar_area = chunks[0];
        let main_area = chunks[1];

        // Render sidebar (sessions list)
        self.render_sidebar(state, sidebar_area, buf, theme, focused);

        // Render main content based on view
        match state.view {
            ConversationView::SessionsList | ConversationView::Search => {
                self.render_welcome(state, main_area, buf, theme);
            }
            ConversationView::Conversation => {
                self.render_conversation(state, main_area, buf, theme, focused);
            }
        }

        // Render search overlay if in search mode
        if state.view == ConversationView::Search {
            self.render_search_overlay(state, area, buf, theme);
        }
    }

    /// Render the sidebar with sessions list
    fn render_sidebar(
        &self,
        state: &PluginState,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        focused: bool,
    ) {
        let border_style = if focused {
            style_for_ui_element(theme, UiElement::Border)
        } else {
            style_for_ui_element(theme, UiElement::MutedText)
        };

        let title_style =
            style_for_ui_element(theme, UiElement::Primary).add_modifier(Modifier::BOLD);
        let text_style = style_for_ui_element(theme, UiElement::Text);
        let muted_style = style_for_ui_element(theme, UiElement::MutedText);

        // Create block
        let title = if state.is_loading {
            " Sessions (Loading...) ".to_string()
        } else if state.search_query.is_some() {
            format!(" Sessions ({} filtered) ", state.filtered_sessions().len())
        } else {
            format!(" Sessions ({}) ", state.sessions.len())
        };

        let block = Block::default()
            .title(Line::from(vec![Span::styled(title, title_style)]))
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner_area = block.inner(area);
        block.render(area, buf);

        // Get filtered sessions
        let sessions: Vec<&SessionInfo> = state.filtered_sessions();

        if sessions.is_empty() && !state.is_loading {
            let empty_text = empty_sessions_message(state);

            let paragraph = Paragraph::new(empty_text)
                .style(muted_style)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });

            // Center vertically
            let empty_area = Rect {
                x: inner_area.x,
                y: inner_area.y + inner_area.height / 3,
                width: inner_area.width,
                height: inner_area.height / 3,
            };
            paragraph.render(empty_area, buf);
            return;
        }

        // Calculate visible range based on actual render area
        let lines_per_item: usize = if self.compact { 1 } else { 3 };
        let items_per_page = inner_area.height as usize / lines_per_item.max(1);
        let start_idx = state.list_nav.scroll_offset;
        let end_idx = (start_idx + items_per_page).min(sessions.len());
        let visible_sessions = &sessions[start_idx..end_idx];

        let mut lines: Vec<Line> = Vec::new();

        for (relative_idx, session_info) in visible_sessions.iter().enumerate() {
            let absolute_idx = start_idx + relative_idx;
            let is_selected = state.list_nav.is_selected(absolute_idx);

            // Build session line
            let mut spans = Vec::new();

            // Selection indicator and adapter icon
            let selection_char = if is_selected { "▶ " } else { "  " };
            spans.push(Span::styled(
                format!("{}{} ", selection_char, session_info.adapter_icon),
                if is_selected {
                    style_for_ui_element(theme, UiElement::Highlight)
                } else {
                    text_style
                },
            ));

            // Session name (truncated if needed)
            let name_width = inner_area.width.saturating_sub(6) as usize;
            let name = if session_info.title().width() > name_width {
                format!(
                    "{}...",
                    &session_info.title()[..name_width.saturating_sub(3)]
                )
            } else {
                session_info.title().to_string()
            };

            let name_style = if is_selected {
                style_for_ui_element(theme, UiElement::ActiveItem).add_modifier(Modifier::BOLD)
            } else {
                text_style
            };
            spans.push(Span::styled(name, name_style));

            lines.push(Line::from(spans));

            // Metadata line (date and message count)
            if !self.compact {
                let mut meta_spans = Vec::new();
                meta_spans.push(Span::raw("    ")); // Indent

                // Date
                let date_str = format_date(&session_info.session.updated_at);
                meta_spans.push(Span::styled(date_str, muted_style));

                // Message count
                let msg_count = session_info.message_count();
                let count_str = format!(" • {}", compact_message_count(msg_count));
                meta_spans.push(Span::styled(count_str, muted_style));

                // Token count if available
                if let Some(tokens) = session_info.total_tokens() {
                    let token_str = format!(" • {}", token_count_label(tokens));
                    meta_spans.push(Span::styled(token_str, muted_style));
                }

                lines.push(Line::from(meta_spans));

                // Empty line for spacing
                lines.push(Line::from(""));
            }
        }

        // Render the list
        let list = Paragraph::new(Text::from(lines))
            .style(text_style)
            .wrap(Wrap { trim: false });
        list.render(inner_area, buf);

        // Render scrollbar if needed
        if sessions.len() > items_per_page {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let mut scrollbar_state = ScrollbarState::default()
                .content_length(sessions.len())
                .position(state.list_nav.selected)
                .viewport_content_length(state.list_nav.viewport_height);

            scrollbar.render(inner_area, buf, &mut scrollbar_state);
        }
    }

    /// Render the welcome/info panel when no conversation is selected
    fn render_welcome(&self, state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let border_style = style_for_ui_element(theme, UiElement::Border);
        let title_style =
            style_for_ui_element(theme, UiElement::Primary).add_modifier(Modifier::BOLD);
        let text_style = style_for_ui_element(theme, UiElement::Text);
        let muted_style = style_for_ui_element(theme, UiElement::MutedText);
        let primary_style = style_for_ui_element(theme, UiElement::Primary);

        let block = Block::default()
            .title(Line::from(vec![Span::styled(
                " Conversation ",
                title_style,
            )]))
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner_area = block.inner(area);
        block.render(area, buf);

        // Build welcome content
        let mut text_lines = vec![
            Line::from(vec![Span::styled("Welcome to Conversations", title_style)]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "View and search your AI coding sessions from multiple adapters.",
                text_style,
            )]),
            Line::from(""),
        ];

        // Stats
        if !state.sessions.is_empty() {
            text_lines.push(Line::from(vec![Span::styled(
                "📊 Statistics",
                primary_style.add_modifier(Modifier::BOLD),
            )]));

            let total_sessions = state.sessions.len();
            let total_messages = state.total_message_count();
            text_lines.push(Line::from(vec![Span::styled(
                format!("  Total Sessions: {}", total_sessions),
                text_style,
            )]));
            text_lines.push(Line::from(vec![Span::styled(
                format!("  Total Messages: {}", total_messages),
                text_style,
            )]));

            // Adapter breakdown
            let adapter_counts = state.adapter_counts();
            for (adapter_type, count) in adapter_counts {
                text_lines.push(Line::from(vec![Span::styled(
                    format!(
                        "  {} {}: {}",
                        adapter_type.icon(),
                        adapter_type.display_name(),
                        count
                    ),
                    text_style,
                )]));
            }

            if let Some(tokens) = state.total_tokens {
                text_lines.push(Line::from(vec![Span::styled(
                    format!("  Total Tokens: {}", token_count_label(tokens.total_tokens)),
                    text_style,
                )]));
            }

            text_lines.push(Line::from(""));
        }

        // Key bindings help
        text_lines.push(Line::from(vec![Span::styled(
            "⌨️  Key Bindings",
            primary_style.add_modifier(Modifier::BOLD),
        )]));
        text_lines.push(Line::from(vec![Span::styled(
            "  ↑/↓ or j/k  Navigate sessions",
            muted_style,
        )]));
        text_lines.push(Line::from(vec![Span::styled(
            "  Enter or l    Open session",
            muted_style,
        )]));
        text_lines.push(Line::from(vec![Span::styled(
            "  f             Filter sessions",
            muted_style,
        )]));
        text_lines.push(Line::from(vec![Span::styled(
            "  r             Refresh sessions",
            muted_style,
        )]));
        text_lines.push(Line::from(vec![Span::styled(
            "  g/G           First/Last session",
            muted_style,
        )]));

        let paragraph = Paragraph::new(Text::from(text_lines))
            .style(text_style)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        paragraph.render(inner_area, buf);
    }

    /// Render the conversation view
    fn render_conversation(
        &self,
        state: &PluginState,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
        focused: bool,
    ) {
        let border_style = if focused {
            style_for_ui_element(theme, UiElement::Border)
        } else {
            style_for_ui_element(theme, UiElement::MutedText)
        };

        let title_style =
            style_for_ui_element(theme, UiElement::Primary).add_modifier(Modifier::BOLD);

        // Get session info for title
        let title = if let Some(session) = state.selected_session_info() {
            format!(
                " {} {} - {} ",
                session.adapter_icon,
                truncate_string(session.title(), 30),
                compact_message_count(state.messages.len())
            )
        } else {
            " Conversation ".to_string()
        };

        let block = Block::default()
            .title(Line::from(vec![Span::styled(title, title_style)]))
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner_area = block.inner(area);
        block.render(area, buf);

        if state.messages.is_empty() {
            if state.is_loading {
                let loading_style = style_for_ui_element(theme, UiElement::Info);
                let loading = Paragraph::new(loading_messages_message())
                    .style(loading_style)
                    .alignment(Alignment::Center);
                loading.render(inner_area, buf);
            } else {
                let muted_style = style_for_ui_element(theme, UiElement::MutedText);
                let empty = Paragraph::new(empty_messages_message())
                    .style(muted_style)
                    .alignment(Alignment::Center);
                empty.render(inner_area, buf);
            }
            return;
        }

        // Render messages
        self.render_messages(state, inner_area, buf, theme);
    }

    /// Render the list of messages
    fn render_messages(&self, state: &PluginState, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let text_style = style_for_ui_element(theme, UiElement::Text);
        let user_style =
            style_for_ui_element(theme, UiElement::Primary).add_modifier(Modifier::BOLD);
        let assistant_style =
            style_for_ui_element(theme, UiElement::Secondary).add_modifier(Modifier::BOLD);
        let system_style = style_for_ui_element(theme, UiElement::Warning);
        let tool_style = style_for_ui_element(theme, UiElement::Info);
        let muted_style = style_for_ui_element(theme, UiElement::MutedText);
        let mut lines: Vec<Line> = Vec::new();
        let content_width = area.width.saturating_sub(4) as usize;

        for message in state.messages.iter() {
            let is_expanded = state.is_message_expanded(&message.id);

            // Message header with role
            let role_style = match message.role {
                Role::User => user_style,
                Role::Assistant => assistant_style,
                Role::System => system_style,
                Role::Tool => tool_style,
            };

            let role_icon = match message.role {
                Role::User => "👤",
                Role::Assistant => "🤖",
                Role::System => "⚙️ ",
                Role::Tool => "🔧",
            };

            let mut header_spans = vec![
                Span::styled(format!("{} ", role_icon), role_style),
                Span::styled(format!("{:?}", message.role), role_style),
            ];

            // Timestamp
            if self.show_timestamps {
                let time_str = format_time(&message.timestamp);
                header_spans.push(Span::styled(format!("  {}", time_str), muted_style));
            }

            // Model info for assistant messages
            if let Some(ref model) = message.model {
                header_spans.push(Span::styled(format!("  ({})", model), muted_style));
            }

            // Token count
            if let Some(tokens) = message.tokens {
                header_spans.push(Span::styled(
                    format!("  {}↑ {}↓", tokens.prompt_tokens, tokens.completion_tokens),
                    muted_style,
                ));
            }

            // Expansion indicator
            if message.has_content_blocks() || message.has_tool_uses() {
                let expand_icon = if is_expanded { "▼" } else { "▶" };
                header_spans.push(Span::styled(format!("  {}", expand_icon), muted_style));
            }

            lines.push(Line::from(header_spans));

            // Message content
            let content = if message.content.is_empty() && !message.content_blocks.is_empty() {
                // Build content from blocks
                message
                    .content_blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { content } => Some(content.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                message.content.clone()
            };

            // Wrap content lines
            let content_lines = wrap_text(&content, content_width);
            for line in &content_lines {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(line.to_string(), text_style),
                ]));
            }

            // Show content blocks if expanded
            if is_expanded {
                for block in &message.content_blocks {
                    match block {
                        ContentBlock::Code { language, code, .. } => {
                            lines.push(Line::from(""));
                            let lang_str = language.as_deref().unwrap_or("text");
                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(format!("```{}", lang_str), muted_style),
                            ]));

                            let code_lines =
                                wrap_text(code.as_str(), content_width.saturating_sub(4));
                            for line in &code_lines {
                                lines.push(Line::from(vec![
                                    Span::raw("    "),
                                    Span::styled(line.to_string(), text_style),
                                ]));
                            }

                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled("```", muted_style),
                            ]));
                        }
                        ContentBlock::ToolUse { id, name, input } => {
                            lines.push(Line::from(""));
                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled("🔧 Tool Use: ", tool_style),
                                Span::styled(name.to_string(), text_style),
                            ]));
                            lines.push(Line::from(vec![
                                Span::raw("    ID: "),
                                Span::styled(id.to_string(), muted_style),
                            ]));

                            let input_lines =
                                wrap_text(input.as_str(), content_width.saturating_sub(4));
                            for line in &input_lines {
                                lines.push(Line::from(vec![
                                    Span::raw("    "),
                                    Span::styled(line.to_string(), muted_style),
                                ]));
                            }
                        }
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => {
                            lines.push(Line::from(""));
                            let result_style = if *is_error {
                                style_for_ui_element(theme, UiElement::Error)
                            } else {
                                tool_style
                            };
                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled("🔧 Tool Result: ", result_style),
                                Span::styled(tool_use_id.to_string(), muted_style),
                            ]));

                            let result_lines =
                                wrap_text(content.as_str(), content_width.saturating_sub(4));
                            for line in &result_lines {
                                lines.push(Line::from(vec![
                                    Span::raw("    "),
                                    Span::styled(line.to_string(), text_style),
                                ]));
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Show tool uses summary if present
            if !message.tool_uses.is_empty() && !is_expanded {
                let tool_count = message.tool_uses.len();
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!(
                            "🔧 {} tool use{}",
                            tool_count,
                            if tool_count > 1 { "s" } else { "" }
                        ),
                        tool_style,
                    ),
                ]));
            }

            // Separator between messages
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "─".repeat(area.width as usize),
                muted_style,
            )]));
            lines.push(Line::from(""));
        }

        // Calculate visible lines based on scroll
        let scroll_y = state.message_scroll.scroll_y;
        let visible_lines: Vec<Line> = lines
            .into_iter()
            .skip(scroll_y)
            .take(area.height as usize)
            .collect();

        // Render visible messages
        let paragraph = Paragraph::new(Text::from(visible_lines))
            .style(text_style)
            .wrap(Wrap { trim: false });

        paragraph.render(area, buf);

        // Render scrollbar
        if state.message_scroll.content_height > state.message_scroll.viewport_height {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            let mut scrollbar_state = ScrollbarState::default()
                .content_length(state.message_scroll.content_height)
                .position(state.message_scroll.scroll_y)
                .viewport_content_length(state.message_scroll.viewport_height);

            scrollbar.render(area, buf, &mut scrollbar_state);
        }
    }

    /// Render session filter overlay
    fn render_search_overlay(
        &self,
        state: &PluginState,
        area: Rect,
        buf: &mut Buffer,
        theme: &Theme,
    ) {
        let popup_width = 50;
        let popup_height = 3;

        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect {
            x: area.x + popup_x,
            y: area.y + popup_y,
            width: popup_width,
            height: popup_height,
        };

        // Clear background
        Clear.render(popup_area, buf);

        // Render border
        let border_style = style_for_ui_element(theme, UiElement::Border);
        let title_style =
            style_for_ui_element(theme, UiElement::Primary).add_modifier(Modifier::BOLD);

        let block = Block::default()
            .title(Line::from(vec![Span::styled(
                " Filter Sessions ",
                title_style,
            )]))
            .borders(Borders::ALL)
            .border_style(border_style);

        let inner_area = block.inner(popup_area);
        block.render(popup_area, buf);

        // Render search input
        let input_style = style_for_ui_element(theme, UiElement::Input);
        let query = state.search_query.as_deref().unwrap_or("");

        let input_text = if query.is_empty() {
            "Type to filter...".to_string()
        } else {
            query.to_string()
        };

        let input_style = if query.is_empty() {
            style_for_ui_element(theme, UiElement::InputPlaceholder)
        } else {
            input_style
        };

        let paragraph = Paragraph::new(input_text).style(input_style);
        paragraph.render(inner_area, buf);

        // Render cursor
        if !query.is_empty() {
            let cursor_x = inner_area.x + query.len() as u16;
            let cursor_y = inner_area.y;
            if let Some(cell) = buf.cell_mut((cursor_x, cursor_y)) {
                let cursor_style = style_for_ui_element(theme, UiElement::Primary);
                cell.set_style(cursor_style);
            }
        }
    }

    /// Get the total height of rendered content (for scroll calculation)
    pub fn calculate_content_height(&self, messages: &[Message], width: u16) -> usize {
        let content_width = width.saturating_sub(4) as usize;
        let mut total_lines = 0;

        for message in messages {
            // Header line
            total_lines += 1;

            // Content lines
            let content_lines = wrap_text(&message.content, content_width);
            total_lines += content_lines.len();

            // Separator
            total_lines += 3;
        }

        total_lines
    }
}

/// Format a datetime for display
fn format_date(dt: &DateTime<chrono::Utc>) -> String {
    let local = dt.with_timezone(&Local);
    let now = Local::now();
    let diff = now.signed_duration_since(local);

    if diff.num_days() == 0 {
        local.format("%H:%M").to_string()
    } else if diff.num_days() < 7 {
        local.format("%a %H:%M").to_string()
    } else {
        local.format("%Y-%m-%d").to_string()
    }
}

/// Format a time for display
fn format_time(dt: &DateTime<chrono::Utc>) -> String {
    let local = dt.with_timezone(&Local);
    local.format("%H:%M:%S").to_string()
}

/// Truncate a string to a maximum length
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.width() > max_len {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    } else {
        s.to_string()
    }
}

/// Wrap text to a maximum width
fn wrap_text(text: &str, width: usize) -> Vec<&str> {
    if width == 0 {
        return vec![text];
    }

    let mut lines = Vec::new();

    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push("");
            continue;
        }

        let mut remaining = paragraph;
        while !remaining.is_empty() {
            if remaining.width() <= width {
                lines.push(remaining);
                break;
            }

            // Find break point
            let mut break_idx = width;
            while break_idx > 0 && !remaining.is_char_boundary(break_idx) {
                break_idx -= 1;
            }

            // Try to break at word boundary
            if let Some(space_idx) = remaining[..break_idx].rfind(' ') {
                break_idx = space_idx;
            }

            if break_idx == 0 {
                break_idx = width;
                while break_idx > 0 && !remaining.is_char_boundary(break_idx) {
                    break_idx -= 1;
                }
                if break_idx == 0 {
                    break_idx = 1;
                }
            }

            lines.push(&remaining[..break_idx]);
            remaining = remaining[break_idx..].trim_start();
        }
    }

    lines
}

fn empty_sessions_message(state: &PluginState) -> String {
    match state
        .search_query
        .as_deref()
        .filter(|query| !query.is_empty())
    {
        Some(query) => format!(
            "No sessions match \"{}\"\n\nType another query\nr  Refresh sessions\nf  Reset filter\nEsc  clear search\n?  Help",
            query
        ),
        None => "No sessions found\n\nr  Refresh detected adapters\nf  Filter sessions\n/  Search sessions\n?  Help\n\nSessions appear after supported adapters are detected."
            .to_string(),
    }
}

fn empty_messages_message() -> &'static str {
    "No messages in this session\n\nEsc/h  Back to sessions\nThen r  Refresh or f  Filter"
}

fn loading_messages_message() -> &'static str {
    "Loading messages\n\nEsc/h  Back to sessions\nr  Refresh sessions"
}

fn compact_message_count(count: usize) -> String {
    if count == 1 {
        "1 msg".to_string()
    } else {
        format!("{} msgs", count)
    }
}

fn token_count_label(count: usize) -> String {
    if count < 1000 {
        if count == 1 {
            "1 token".to_string()
        } else {
            format!("{} tokens", count)
        }
    } else {
        format!("{}K tokens", count / 1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_creation() {
        let renderer = ConversationsRenderer::new();
        assert!(!renderer.compact);
        assert!(renderer.show_timestamps);

        let compact = ConversationsRenderer::compact();
        assert!(compact.compact);
        assert!(!compact.show_timestamps);
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("Hello", 10), "Hello");
        assert_eq!(truncate_string("Hello World", 8), "Hello...");
        assert_eq!(truncate_string("Test", 3), "...");
    }

    #[test]
    fn test_wrap_text() {
        let text = "Hello World";
        let lines = wrap_text(text, 5);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "Hello");
        assert_eq!(lines[1], "World");

        let long_text = "This is a very long line that needs wrapping";
        let lines = wrap_text(long_text, 10);
        assert!(!lines.is_empty());
        for line in &lines {
            assert!(line.width() <= 10);
        }
    }

    #[test]
    fn test_wrap_text_with_newlines() {
        let text = "Line 1\nLine 2\n\nLine 4";
        let lines = wrap_text(text, 20);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "Line 1");
        assert_eq!(lines[1], "Line 2");
        assert_eq!(lines[2], "");
        assert_eq!(lines[3], "Line 4");
    }

    #[test]
    fn test_format_date() {
        use chrono::Utc;

        let now = Utc::now();
        let date_str = format_date(&now);
        // Should show time format for today
        assert!(date_str.contains(':') || date_str.contains("today"));

        let old = now - chrono::Duration::days(10);
        let old_str = format_date(&old);
        // Should show date format for old dates
        assert!(old_str.contains('-'));
    }

    #[test]
    fn test_empty_sessions_message_points_to_next_actions() {
        let state = PluginState::new();
        let message = empty_sessions_message(&state);

        assert!(message.contains("No sessions found"));
        assert!(message.contains("r  Refresh detected adapters"));
        assert!(message.contains("f  Filter sessions"));
        assert!(message.contains("/  Search sessions"));
        assert!(message.contains("?  Help"));
        assert!(message.contains("Sessions appear after supported adapters are detected"));
    }

    #[test]
    fn test_empty_sessions_message_mentions_query() {
        let mut state = PluginState::new();
        state.start_search("render".to_string());
        let message = empty_sessions_message(&state);

        assert!(message.contains("No sessions match \"render\""));
        assert!(message.contains("Type another query"));
        assert!(message.contains("r  Refresh sessions"));
        assert!(message.contains("f  Reset filter"));
        assert!(message.contains("Esc  clear search"));
        assert!(message.contains("?  Help"));
    }

    #[test]
    fn test_empty_messages_message_points_to_next_actions() {
        let message = empty_messages_message();

        assert!(message.contains("No messages in this session"));
        assert!(message.contains("Esc/h  Back to sessions"));
        assert!(message.contains("Then r  Refresh or f  Filter"));
    }

    #[test]
    fn test_loading_messages_message_points_to_next_actions() {
        let message = loading_messages_message();

        assert!(message.contains("Loading messages"));
        assert!(message.contains("Esc/h  Back to sessions"));
        assert!(message.contains("r  Refresh sessions"));
    }

    #[test]
    fn test_render_sessions_list_uses_plural_message_count() {
        let renderer = ConversationsRenderer::new();
        let mut state = PluginState::new();
        let mut session = crate::core::models::conversation::Session::new(
            "session-1",
            "Render polish",
            "test-adapter",
        );
        session.message_count = 2;
        state.set_sessions(vec![SessionInfo {
            session,
            adapter_type: crate::adapters::types::AdapterType::Codex,
            adapter_icon: 'T',
            adapter_name: "Test Adapter".to_string(),
        }]);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        renderer.render(&state, area, &mut buf, &theme, true);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("2 msgs"));
        assert!(!content.contains("2 msg "));
    }

    #[test]
    fn test_render_sessions_list_shows_small_token_counts() {
        let renderer = ConversationsRenderer::new();
        let mut state = PluginState::new();
        let mut session = crate::core::models::conversation::Session::new(
            "session-1",
            "Token polish",
            "test-adapter",
        );
        session.message_count = 1;
        session.total_tokens = Some(999);
        state.set_sessions(vec![SessionInfo {
            session,
            adapter_type: crate::adapters::types::AdapterType::Codex,
            adapter_icon: 'T',
            adapter_name: "Test Adapter".to_string(),
        }]);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        renderer.render(&state, area, &mut buf, &theme, true);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("999 tokens"));
        assert!(!content.contains("0K"));
    }

    #[test]
    fn test_render_conversation_title_uses_plural_message_count() {
        let renderer = ConversationsRenderer::new();
        let mut state = PluginState::new();
        let session = crate::core::models::conversation::Session::new(
            "session-1",
            "Render polish",
            "test-adapter",
        );
        state.set_sessions(vec![SessionInfo {
            session,
            adapter_type: crate::adapters::types::AdapterType::Codex,
            adapter_icon: 'T',
            adapter_name: "Test Adapter".to_string(),
        }]);
        state.selected_session = Some(0);
        state.view = ConversationView::Conversation;
        state.messages = vec![
            crate::core::models::conversation::Message::user("msg-1", "First"),
            crate::core::models::conversation::Message::assistant("msg-2", "Second"),
        ];
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        renderer.render(&state, area, &mut buf, &theme, true);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("2 msgs"));
        assert!(!content.contains("2 msg "));
    }

    #[test]
    fn test_token_count_label_keeps_large_counts_compact() {
        assert_eq!(token_count_label(1), "1 token");
        assert_eq!(token_count_label(999), "999 tokens");
        assert_eq!(token_count_label(1_500), "1K tokens");
    }

    #[test]
    fn test_render_loading_messages_includes_next_actions() {
        let renderer = ConversationsRenderer::new();
        let mut state = PluginState::new();
        state.view = ConversationView::Conversation;
        state.set_loading(true);
        let theme = Theme::default();
        let area = Rect::new(0, 0, 120, 30);
        let mut buf = Buffer::empty(area);

        renderer.render(&state, area, &mut buf, &theme, true);

        let content: String = buf
            .content()
            .iter()
            .map(|cell| cell.symbol().to_string())
            .collect();
        assert!(content.contains("Loading messages"));
        assert!(content.contains("Esc/h  Back to sessions"));
        assert!(content.contains("r  Refresh sessions"));
    }
}
