use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, List, ListItem, Paragraph, Wrap};
use unicode_width::UnicodeWidthStr;

use crate::{ColorMode, Surface, TuiApp};

#[derive(Clone, Copy)]
struct Palette {
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
    frame.render_widget(Block::new().style(Style::new().bg(palette.canvas)), area);
    if area.width < 44 || area.height < 10 {
        render_tiny(frame, app, area, palette);
    } else if area.width < 78 {
        render_narrow(frame, app, area, palette);
    } else {
        render_wide(frame, app, area, palette);
    }
}

fn render_wide(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, palette: Palette) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(32)])
        .split(area);
    render_navigation(frame, app, columns[0], palette);
    render_content(frame, app, columns[1], palette);
}

fn render_narrow(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, palette: Palette) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(5),
            Constraint::Length(4),
            Constraint::Length(1),
        ])
        .split(area);
    let title = format!("Keith Agent  {}  Tab: next view", app.surface.label());
    frame.render_widget(
        Paragraph::new(title).style(Style::new().bg(palette.layer).fg(palette.text)),
        rows[0],
    );
    render_surface(frame, app, rows[1], palette);
    render_composer(frame, app, rows[2], palette);
    render_status(frame, app, rows[3], palette);
}

fn render_tiny(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, palette: Palette) {
    let status = if app.connected {
        "connected"
    } else {
        "offline"
    };
    let text = Text::from(vec![
        Line::from(Span::styled(
            format!("Keith Agent: {}", app.surface.label()),
            Style::new().fg(palette.text).add_modifier(Modifier::BOLD),
        )),
        Line::from(format!("Status: {status}")),
        Line::from("Terminal is narrow. Resize for navigation and history."),
        Line::from("Ctrl-Q quit  Tab view  Enter send"),
        Line::from(format!("> {}", terminal_safe(&app.composer))),
    ]);
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::new().bg(palette.canvas).fg(palette.text))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_navigation(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, palette: Palette) {
    let items = Surface::ALL.into_iter().map(|surface| {
        let style = if surface == app.surface {
            Style::new()
                .bg(palette.selected)
                .fg(palette.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().bg(palette.layer).fg(palette.muted)
        };
        ListItem::new(Line::from(format!("  {}", surface.label()))).style(style)
    });
    let navigation = List::new(items).block(
        Block::new()
            .title(" Keith Agent ")
            .style(Style::new().bg(palette.layer).fg(palette.text)),
    );
    frame.render_widget(navigation, area);
}

fn render_content(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, palette: Palette) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(5),
            Constraint::Length(5),
            Constraint::Length(1),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(format!(" {}", app.surface.label())).style(
            Style::new()
                .bg(palette.selected)
                .fg(palette.text)
                .add_modifier(Modifier::BOLD),
        ),
        rows[0],
    );
    render_surface(frame, app, rows[1], palette);
    render_composer(frame, app, rows[2], palette);
    render_status(frame, app, rows[3], palette);
}

fn render_surface(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, palette: Palette) {
    let lines = match app.surface {
        Surface::Chat => chat_lines(app, area.height),
        Surface::Sessions => app
            .sessions
            .iter()
            .map(|session| {
                let selected = app.attached_session.as_ref() == Some(&session.session_id);
                format!(
                    "{} {}  {:?}  {}",
                    if selected { ">" } else { " " },
                    terminal_safe(session.title.as_deref().unwrap_or("Untitled session")),
                    session.state,
                    session.session_id
                )
            })
            .collect(),
        surface => projection_lines(app, surface),
    };
    let text = if lines.is_empty() {
        Text::from(Line::from(Span::styled(
            empty_label(app.surface),
            Style::new().fg(palette.muted),
        )))
    } else {
        Text::from(lines.into_iter().map(Line::from).collect::<Vec<_>>())
    };
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::new().bg(palette.canvas).fg(palette.text))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn chat_lines(app: &TuiApp, height: u16) -> Vec<String> {
    let Some(reducer) = &app.reducer else {
        return vec!["Attach a session to load authoritative history.".into()];
    };
    let snapshot = reducer.snapshot();
    let visible = usize::from(height).saturating_sub(1).max(1);
    let total = snapshot.messages.len();
    let end = total.saturating_sub(app.scroll_from_end).min(total);
    let start = end.saturating_sub(visible);
    snapshot.messages[start..end]
        .iter()
        .map(|message| format!("{:?}: {}", message.role, terminal_safe(&message.text)))
        .collect()
}

#[allow(clippy::too_many_lines)]
fn projection_lines(app: &TuiApp, surface: Surface) -> Vec<String> {
    if surface == Surface::Models {
        let mut providers = vec![
            "Use /model <provider> [model]. Omitting model selects the catalog default.".into(),
        ];
        providers.extend(
            keith_provider_catalog::BUILTIN_PROVIDERS
                .iter()
                .map(|provider| {
                    format!(
                        "{}  {}  default {}",
                        provider.id,
                        terminal_safe(provider.display_name),
                        provider.default_model
                    )
                }),
        );
        return providers;
    }
    if surface == Surface::Logs {
        return app.logs().iter().map(|line| terminal_safe(line)).collect();
    }
    let Some(reducer) = &app.reducer else {
        return Vec::new();
    };
    let snapshot = reducer.snapshot();
    match surface {
        Surface::Goals => snapshot
            .goals
            .iter()
            .map(|goal| format!("{:?}  {}", goal.state, terminal_safe(&goal.objective)))
            .collect(),
        Surface::Queue => snapshot
            .actions
            .iter()
            .map(|action| format!("{}  {}", action.state, terminal_safe(&action.source)))
            .collect(),
        Surface::Models => unreachable!("models are rendered from the installation catalog"),
        Surface::Plans => snapshot
            .plans
            .iter()
            .map(|plan| format!("{}  {}", plan.state, terminal_safe(&plan.summary)))
            .collect(),
        Surface::Children => snapshot
            .children
            .iter()
            .map(|child| format!("{}  {}", child.state, terminal_safe(&child.objective)))
            .collect(),
        Surface::Tools => snapshot
            .tools
            .iter()
            .map(|tool| format!("{}  {}", tool.state, tool.tool_call_id))
            .collect(),
        Surface::Kernels => snapshot
            .kernels
            .iter()
            .map(|kernel| format!("{}  {}", kernel.state, kernel.runtime))
            .collect(),
        Surface::Schedules => snapshot
            .schedules
            .iter()
            .map(|schedule| {
                format!(
                    "{}  next {:?}  paused {}",
                    schedule.job_id, schedule.next_run, schedule.paused
                )
            })
            .collect(),
        Surface::Commitments => snapshot
            .commitments
            .iter()
            .map(|item| format!("{}  {}", item.state, terminal_safe(&item.summary)))
            .collect(),
        Surface::Waiting => snapshot
            .waits
            .iter()
            .map(|wait| format!("{}  {}", wait.state, wait.wait_id))
            .collect(),
        Surface::Confirmations => snapshot
            .confirmations
            .iter()
            .map(|confirmation| {
                format!(
                    "{}  {}",
                    confirmation.confirmation_id,
                    terminal_safe(&confirmation.summary)
                )
            })
            .collect(),
        Surface::Memory => snapshot
            .memory_changes
            .iter()
            .map(|change| format!("{:?}  {}", change.change, terminal_safe(&change.source)))
            .collect(),
        Surface::Diagnostics => vec![
            format!("Generation: {}", snapshot.generation.get()),
            format!("Sequence: {}", snapshot.through_sequence.get()),
            format!("Projection revision: {}", snapshot.revision.get()),
            format!("Stream: {:?}", reducer.stream_state()),
            format!("Composer display width: {}", app.composer_display_width()),
            format!("Queued commands: {}", app.pending_len()),
        ],
        Surface::Logs => unreachable!("logs are rendered without a session projection"),
        Surface::Artifacts => vec!["Artifacts are exposed by tool and export projections.".into()],
        Surface::Knowledge => {
            vec!["Knowledge changes use shared memory and command results.".into()]
        }
        Surface::Channels => vec!["Channel routes and delivery state are daemon-owned.".into()],
        Surface::Settings => vec!["Settings use shared configuration and protocol state.".into()],
        Surface::Refinement => vec!["Refinement diffs and confirmations are daemon-owned.".into()],
        Surface::Chat | Surface::Sessions => Vec::new(),
    }
}

fn render_composer(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, palette: Palette) {
    let label = if app.attached_session.is_some() {
        " Message  Enter send  Alt-Enter newline  Ctrl-K steer  Ctrl-E editor "
    } else {
        " Message  Select a session before sending "
    };
    frame.render_widget(
        Paragraph::new(terminal_safe(&app.composer))
            .block(Block::new().title(label))
            .style(Style::new().bg(palette.layer).fg(palette.text))
            .wrap(Wrap { trim: false }),
        area,
    );
    if area.width > 2 && area.height > 1 {
        let before = &app.composer[..app.cursor_byte];
        let line = before.lines().count().saturating_sub(1);
        let column = before.lines().next_back().map_or(0, UnicodeWidthStr::width);
        let x = area
            .x
            .saturating_add(u16::try_from(column).unwrap_or(u16::MAX))
            .min(area.right().saturating_sub(1));
        let y = area
            .y
            .saturating_add(1)
            .saturating_add(u16::try_from(line).unwrap_or(u16::MAX))
            .min(area.bottom().saturating_sub(1));
        frame.set_cursor_position(Position::new(x, y));
    }
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

fn render_status(frame: &mut Frame<'_>, app: &TuiApp, area: Rect, palette: Palette) {
    let connection = if app.reconnecting {
        "reconnecting"
    } else if app.connected {
        "connected"
    } else {
        "offline"
    };
    let presence = app
        .reducer
        .as_ref()
        .map_or("unavailable".into(), |reducer| {
            format!("{:?}", reducer.snapshot().presence.state)
        });
    let status = format!(
        " {connection}  presence {presence}  Tab view  Ctrl-S sessions  Ctrl-X cancel  Ctrl-Q quit"
    );
    let color = if app.connected {
        palette.accent
    } else {
        palette.danger
    };
    frame.render_widget(
        Paragraph::new(status).style(Style::new().bg(palette.layer).fg(color)),
        area,
    );
}

const fn empty_label(surface: Surface) -> &'static str {
    match surface {
        Surface::Chat => "No messages",
        Surface::Queue => "No queued actions",
        Surface::Sessions => "No sessions",
        Surface::Models => "No model selection",
        Surface::Goals => "No goals",
        Surface::Plans => "No plans",
        Surface::Children => "No children",
        Surface::Tools => "No tool calls",
        Surface::Kernels => "No kernels",
        Surface::Artifacts => "No artifacts",
        Surface::Schedules => "No schedules",
        Surface::Commitments => "No commitments",
        Surface::Waiting => "No waits",
        Surface::Confirmations => "No confirmations",
        Surface::Memory => "No memory changes",
        Surface::Knowledge => "No knowledge changes",
        Surface::Channels => "No channel activity",
        Surface::Settings => "No settings changes",
        Surface::Refinement => "No refinements",
        Surface::Logs => "No logs",
        Surface::Diagnostics => "No diagnostics",
    }
}

const fn palette(mode: ColorMode) -> Palette {
    match mode {
        ColorMode::TrueColor => Palette {
            canvas: Color::Rgb(18, 21, 24),
            layer: Color::Rgb(29, 34, 39),
            selected: Color::Rgb(42, 52, 59),
            text: Color::Rgb(236, 239, 241),
            muted: Color::Rgb(166, 176, 184),
            accent: Color::Rgb(100, 210, 170),
            danger: Color::Rgb(240, 125, 125),
        },
        ColorMode::Ansi256 => Palette {
            canvas: Color::Indexed(234),
            layer: Color::Indexed(236),
            selected: Color::Indexed(239),
            text: Color::Indexed(255),
            muted: Color::Indexed(248),
            accent: Color::Indexed(79),
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
