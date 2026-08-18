use keith_protocol::{MessageProjection, MessageRole, PresenceState, TurnTerminalStatus};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Clear, List, ListItem, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{ColorMode, TuiApp, TuiOverlay};

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    canvas: Color,
    layer: Color,
    selected: Color,
    text: Color,
    muted: Color,
    accent: Color,
    danger: Color,
}

pub fn render(frame: &mut Frame<'_>, app: &TuiApp) {
    let area = frame.area();
    let palette = palette(app.accessibility.color_mode);
    frame.render_widget(
        Paragraph::new("").style(Style::new().bg(palette.canvas)),
        area,
    );

    let activity_height = u16::from(authoritative_activity(app).is_some());
    let header_height = u16::from(area.height >= 8);
    let composer_height = if area.height < 7 { 2 } else { 4 };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(1),
            Constraint::Length(activity_height),
            Constraint::Length(composer_height),
            Constraint::Length(1),
        ])
        .split(area);

    if header_height > 0 {
        render_header(frame, app, rows[0], palette);
    }
    render_conversation(frame, app, rows[1], palette);
    if let Some(activity) = authoritative_activity(app) {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  Keith  ", Style::new().fg(palette.accent)),
                Span::styled(activity, Style::new().fg(palette.muted)),
            ]))
            .style(Style::new().bg(palette.canvas)),
            rows[2],
        );
    }
    render_composer(frame, app, rows[3], palette);
    render_status(frame, app, rows[4], palette);
    if let Some(overlay) = app.overlay {
        render_overlay(frame, app, overlay, area, palette);
    }
}

fn render_header(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, palette: Palette) {
    let conversation = app
        .reducer
        .as_ref()
        .and_then(|reducer| reducer.snapshot().session.title.as_deref())
        .unwrap_or("Conversation");
    let title = if area.width >= 52 {
        format!(" Keith   {}", terminal_safe(conversation))
    } else {
        " Keith".into()
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![Span::styled(
            title,
            Style::new().fg(palette.text).add_modifier(Modifier::BOLD),
        )]))
        .style(Style::new().bg(palette.canvas)),
        area,
    );
}

pub(crate) fn render_conversation(
    frame: &mut Frame<'_>,
    app: &TuiApp,
    area: Rect,
    palette: Palette,
) {
    let width = usize::from(area.width.saturating_sub(2)).max(1);
    let lines = app.reducer.as_ref().map_or_else(
        || {
            vec![Line::from(Span::styled(
                "  Choose a conversation to begin.",
                Style::new().fg(palette.muted),
            ))]
        },
        |reducer| {
            let snapshot = reducer.snapshot();
            let mut messages = snapshot
                .messages
                .iter()
                .filter(|message| !app.is_message_settled(message))
                .flat_map(|message| transcript_lines(message, width, palette))
                .collect::<Vec<_>>();
            if messages.is_empty() {
                messages.push(Line::from(Span::styled(
                    "  Tell Keith what you want taken care of.",
                    Style::new().fg(palette.muted),
                )));
            }
            messages
        },
    );
    let visible = usize::from(area.height);
    let end = lines
        .len()
        .saturating_sub(app.scroll_from_end)
        .min(lines.len());
    let start = end.saturating_sub(visible);
    frame.render_widget(
        Paragraph::new(Text::from(lines[start..end].to_vec()))
            .style(Style::new().bg(palette.canvas).fg(palette.text))
            .wrap(Wrap { trim: false }),
        area,
    );
}

pub fn settled_transcript_lines(
    message: &MessageProjection,
    width: u16,
    color_mode: ColorMode,
) -> Vec<Line<'static>> {
    transcript_lines(message, usize::from(width).max(1), palette(color_mode))
}

fn transcript_lines(
    message: &MessageProjection,
    width: usize,
    palette: Palette,
) -> Vec<Line<'static>> {
    let (label, label_style, body_style) = match message.role {
        MessageRole::User => (
            "You",
            Style::new().fg(palette.text).add_modifier(Modifier::BOLD),
            Style::new().fg(palette.text),
        ),
        MessageRole::Assistant => (
            "Keith",
            Style::new().fg(palette.accent).add_modifier(Modifier::BOLD),
            Style::new().fg(palette.text),
        ),
        MessageRole::Tool => (
            "Activity",
            Style::new().fg(palette.muted),
            Style::new().fg(palette.muted),
        ),
        MessageRole::System => (
            "Notice",
            Style::new().fg(palette.danger).add_modifier(Modifier::BOLD),
            Style::new().fg(palette.text),
        ),
    };
    let body_width = width.saturating_sub(2).max(1);
    let mut lines = vec![Line::from(Span::styled(format!("  {label}"), label_style))];
    lines.extend(
        wrap_preserving_indentation(&message.text, body_width)
            .into_iter()
            .map(|line| Line::from(Span::styled(format!("  {line}"), body_style))),
    );
    lines.push(Line::default());
    lines
}

fn wrap_preserving_indentation(value: &str, width: usize) -> Vec<String> {
    let safe = terminal_safe(value);
    let mut output = Vec::new();
    for logical in safe.split('\n') {
        if logical.is_empty() {
            output.push(String::new());
            continue;
        }
        let indentation = logical
            .chars()
            .take_while(|character| matches!(character, ' ' | '\t'))
            .collect::<String>();
        let continuation = if UnicodeWidthStr::width(indentation.as_str()) < width {
            indentation
        } else {
            String::new()
        };
        let mut current = String::new();
        let mut current_width: usize = 0;
        for character in logical.chars() {
            let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
            if current_width > 0 && current_width.saturating_add(character_width) > width {
                output.push(std::mem::take(&mut current));
                current.push_str(&continuation);
                current_width = UnicodeWidthStr::width(current.as_str());
            }
            current.push(character);
            current_width = current_width.saturating_add(character_width);
        }
        output.push(current);
    }
    output
}

fn render_composer(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, palette: Palette) {
    let enabled = app.attached_session.is_some();
    let prompt = if enabled {
        terminal_safe(&app.composer)
    } else {
        "Choose a conversation before sending a message".into()
    };
    let hint = if area.width >= 64 {
        "Enter send   Alt-Enter newline   Ctrl-P commands   Ctrl-X stop"
    } else if area.width >= 36 {
        "Enter send   Ctrl-P commands"
    } else {
        "Enter send"
    };
    let text = Text::from(vec![
        Line::from(vec![
            Span::styled(" › ", Style::new().fg(palette.accent)),
            Span::styled(
                prompt,
                Style::new().fg(if enabled { palette.text } else { palette.muted }),
            ),
        ]),
        Line::from(Span::styled(
            format!("   {hint}"),
            Style::new().fg(palette.muted),
        )),
    ]);
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::new().bg(palette.layer).fg(palette.text))
            .wrap(Wrap { trim: false }),
        area,
    );
    if enabled && area.width > 3 && area.height > 0 {
        let before = &app.composer[..app.cursor_byte];
        let line = before.lines().count().saturating_sub(1);
        let column = before.lines().next_back().map_or(0, UnicodeWidthStr::width);
        let x = area
            .x
            .saturating_add(3)
            .saturating_add(u16::try_from(column).unwrap_or(u16::MAX))
            .min(area.right().saturating_sub(1));
        let y = area
            .y
            .saturating_add(u16::try_from(line).unwrap_or(u16::MAX))
            .min(area.bottom().saturating_sub(1));
        frame.set_cursor_position(Position::new(x, y));
    }
}

fn render_status(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, palette: Palette) {
    let (connection, color) = if app.reconnecting {
        ("Reconnecting", palette.muted)
    } else if app.connected {
        ("Connected", palette.accent)
    } else {
        ("Offline", palette.danger)
    };
    let terminal = app.reducer.as_ref().and_then(|reducer| {
        reducer
            .snapshot()
            .terminal
            .as_ref()
            .map(|terminal| match terminal.status {
                TurnTerminalStatus::Completed => "Completed",
                TurnTerminalStatus::Failed => "Could not finish",
                TurnTerminalStatus::Cancelled => "Stopped",
                TurnTerminalStatus::Exhausted => "Reached its limit",
            })
    });
    let mut status = format!(" {connection}");
    if let Some(terminal) = terminal {
        status.push_str("   ");
        status.push_str(terminal);
    }
    if area.width >= 44 {
        status.push_str("   Ctrl-S conversations   Ctrl-Q quit");
    }
    frame.render_widget(
        Paragraph::new(status).style(Style::new().bg(palette.canvas).fg(color)),
        area,
    );
}

fn authoritative_activity(app: &TuiApp) -> Option<&'static str> {
    let state = app.reducer.as_ref()?.snapshot().presence.state;
    match state {
        PresenceState::Available => None,
        PresenceState::Thinking => Some("Thinking"),
        PresenceState::UsingTools => Some("Taking care of it"),
        PresenceState::WaitingChild => Some("Waiting for delegated work"),
        PresenceState::WaitingExternal => Some("Waiting for a response"),
        PresenceState::PausedForUser => Some("Needs you"),
        PresenceState::Scheduled => Some("Scheduled"),
        PresenceState::Completed => Some("Done"),
        PresenceState::Failed => Some("Could not finish"),
    }
}

fn render_overlay(
    frame: &mut Frame<'_>,
    app: &TuiApp,
    overlay: TuiOverlay,
    area: Rect,
    palette: Palette,
) {
    let overlay_area = centered(area, 76, 22);
    frame.render_widget(Clear, overlay_area);
    frame.render_widget(
        Paragraph::new("").style(Style::new().bg(palette.layer)),
        overlay_area,
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(overlay_area);
    frame.render_widget(
        Paragraph::new(overlay.label()).style(
            Style::new()
                .bg(palette.layer)
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        ),
        rows[0],
    );
    let filter = if app.overlay_query.is_empty() {
        "Type to filter".into()
    } else {
        format!("Filter: {}", terminal_safe(&app.overlay_query))
    };
    frame.render_widget(
        Paragraph::new(filter).style(Style::new().bg(palette.layer).fg(palette.muted)),
        rows[1],
    );
    let items = app.overlay_rows();
    let list = if items.is_empty() {
        List::new([ListItem::new(overlay_empty(overlay)).style(Style::new().fg(palette.muted))])
    } else {
        List::new(items.into_iter().enumerate().map(|(index, value)| {
            let style = if index == app.overlay_selection {
                Style::new()
                    .bg(palette.selected)
                    .fg(palette.text)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::new().bg(palette.layer).fg(palette.text)
            };
            ListItem::new(format!(" {}", terminal_safe(&value))).style(style)
        }))
    };
    frame.render_widget(list, rows[2]);
    let help = if overlay == TuiOverlay::Approvals {
        "Alt-A allow once   Alt-D deny   Esc close"
    } else {
        "Enter choose   Tab next   Esc close"
    };
    frame.render_widget(
        Paragraph::new(help).style(Style::new().bg(palette.layer).fg(palette.muted)),
        rows[3],
    );
}

const fn overlay_empty(overlay: TuiOverlay) -> &'static str {
    match overlay {
        TuiOverlay::Sessions => "No conversations match. Clear the filter to see everything.",
        TuiOverlay::Commands => "No commands match.",
        TuiOverlay::Models => "No models match.",
        TuiOverlay::Approvals => "Keith does not need a decision right now.",
        TuiOverlay::Work => "Nothing is in progress. Ask Keith to take care of something.",
        TuiOverlay::Memory => "No saved context is available for this conversation.",
        TuiOverlay::Diagnostics => "Attach a conversation to inspect diagnostics.",
    }
}

fn centered(area: Rect, max_width: u16, max_height: u16) -> Rect {
    let width = area.width.saturating_sub(2).min(max_width).max(1);
    let height = area.height.saturating_sub(2).min(max_height).max(1);
    Rect::new(
        area.x.saturating_add(area.width.saturating_sub(width) / 2),
        area.y
            .saturating_add(area.height.saturating_sub(height) / 2),
        width,
        height,
    )
}

pub(crate) fn terminal_safe(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control() && !matches!(character, '\n' | '\t') {
                '\u{fffd}'
            } else {
                character
            }
        })
        .collect()
}

const fn palette(mode: ColorMode) -> Palette {
    match mode {
        ColorMode::TrueColor => Palette {
            canvas: Color::Reset,
            layer: Color::Rgb(31, 34, 38),
            selected: Color::Rgb(48, 53, 59),
            text: Color::Rgb(235, 237, 240),
            muted: Color::Rgb(151, 158, 166),
            accent: Color::Rgb(97, 205, 153),
            danger: Color::Rgb(232, 112, 112),
        },
        ColorMode::Ansi256 => Palette {
            canvas: Color::Reset,
            layer: Color::Indexed(236),
            selected: Color::Indexed(239),
            text: Color::Indexed(255),
            muted: Color::Indexed(248),
            accent: Color::Indexed(78),
            danger: Color::Indexed(210),
        },
        ColorMode::NoColor => Palette {
            canvas: Color::Reset,
            layer: Color::Reset,
            selected: Color::Reset,
            text: Color::Reset,
            muted: Color::Reset,
            accent: Color::Reset,
            danger: Color::Reset,
        },
        ColorMode::HighContrast => Palette {
            canvas: Color::Black,
            layer: Color::DarkGray,
            selected: Color::White,
            text: Color::White,
            muted: Color::Gray,
            accent: Color::LightGreen,
            danger: Color::LightRed,
        },
    }
}
