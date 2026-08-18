#![forbid(unsafe_code)]

mod connection;
mod render;

pub use connection::*;
pub use render::{render, settled_transcript_lines};

use std::collections::{HashMap, VecDeque};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use keith_agent_types::{
    ChildId, ClientId, CommandId, EntityId, GoalId, JobId, MessageId, SessionId, UtcTimestamp,
};
use keith_protocol::{
    AttachSession, BackgroundControl, BackgroundMode, CancelTarget, ChildMessageRequest,
    ChildWorkspaceMode, ClientCommand, CommandEnvelope, CommandResult, CreateChild, CreateGoal,
    CreateSchedule, DaemonEvent, DeliveryPolicy, EventAcknowledgement, ExportFormat, ExportRequest,
    GoalLimits, MemoryQuery, ResponsePayload, ScheduleExpression, SessionFilter, SessionSummary,
    SteerAction, SubmitPrompt, UpdateSchedule, WireMessage,
};
use keith_ui_model::{
    ClientParity, OperatorCommand, OperatorSurface, ProjectionReducer, ReductionOutcome,
    VirtualizationConfig, project_personal_intelligence,
};
use unicode_width::UnicodeWidthStr;

pub use keith_ui_model::OperatorSurface as Surface;

pub const MAX_COMPOSER_BYTES: usize = 64 * 1_024;
pub const MAX_LOG_LINES: usize = 512;
pub const MAX_PENDING_COMMANDS: usize = 128;

const OVERLAY_COMMANDS: [(&str, &str); 12] = [
    ("Continue this conversation", "/resume"),
    ("Choose a conversation", "/sessions"),
    ("Choose a model", "/models"),
    ("Review decisions", "/approvals"),
    ("See current work", "/work"),
    ("Search saved context", "/memory "),
    ("Create a goal", "/goal "),
    ("Delegate work", "/child "),
    ("Create a schedule", "/schedule "),
    ("Export this conversation", "/export markdown"),
    ("Stop the current turn", "/stop"),
    ("Open diagnostics", "/diagnostics"),
];

pub fn client_parity() -> ClientParity {
    ClientParity {
        surfaces: OperatorSurface::ALL.into_iter().collect(),
        commands: OperatorCommand::ALL.into_iter().collect(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorMode {
    TrueColor,
    Ansi256,
    NoColor,
    HighContrast,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Accessibility {
    pub color_mode: ColorMode,
    pub reduced_motion: bool,
}

impl Default for Accessibility {
    fn default() -> Self {
        Self {
            color_mode: ColorMode::TrueColor,
            reduced_motion: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppAction {
    None,
    Redraw,
    Quit,
    OpenExternalEditor,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TuiOverlay {
    Sessions,
    Commands,
    Models,
    Approvals,
    Work,
    Memory,
    Diagnostics,
}

impl TuiOverlay {
    pub const ALL: [Self; 7] = [
        Self::Sessions,
        Self::Commands,
        Self::Models,
        Self::Approvals,
        Self::Work,
        Self::Memory,
        Self::Diagnostics,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Sessions => "Conversations",
            Self::Commands => "Commands",
            Self::Models => "Models",
            Self::Approvals => "Needs your decision",
            Self::Work => "Work",
            Self::Memory => "Saved context",
            Self::Diagnostics => "Diagnostics",
        }
    }
}

pub struct TuiApp {
    pub client_id: ClientId,
    pub overlay: Option<TuiOverlay>,
    pub overlay_query: String,
    pub overlay_selection: usize,
    pub accessibility: Accessibility,
    pub composer: String,
    pub cursor_byte: usize,
    pub sessions: Vec<SessionSummary>,
    pub attached_session: Option<SessionId>,
    pub reducer: Option<ProjectionReducer>,
    pub connected: bool,
    pub reconnecting: bool,
    pub quit: bool,
    pub scroll_from_end: usize,
    pub last_prompt: Option<String>,
    settled_messages: HashMap<MessageId, String>,
    in_flight_commands: usize,
    pending_commands: VecDeque<ClientCommand>,
    logs: VecDeque<String>,
}

impl TuiApp {
    pub fn new(accessibility: Accessibility) -> Self {
        Self {
            client_id: ClientId::new(),
            overlay: None,
            overlay_query: String::new(),
            overlay_selection: 0,
            accessibility,
            composer: String::new(),
            cursor_byte: 0,
            sessions: Vec::new(),
            attached_session: None,
            reducer: None,
            connected: false,
            reconnecting: false,
            quit: false,
            scroll_from_end: 0,
            last_prompt: None,
            settled_messages: HashMap::new(),
            in_flight_commands: 0,
            pending_commands: VecDeque::new(),
            logs: VecDeque::new(),
        }
    }

    pub fn logs(&self) -> &VecDeque<String> {
        &self.logs
    }

    pub fn open_overlay(&mut self, overlay: TuiOverlay) {
        self.overlay = Some(overlay);
        self.overlay_query.clear();
        self.overlay_selection = 0;
        if overlay == TuiOverlay::Sessions {
            self.list_sessions();
        }
    }

    pub fn pending_settled_messages(&self) -> Vec<keith_protocol::MessageProjection> {
        self.reducer.as_ref().map_or_else(Vec::new, |reducer| {
            reducer
                .snapshot()
                .messages
                .iter()
                .filter(|message| {
                    message.committed
                        && self
                            .settled_messages
                            .get(&message.message_id)
                            .is_none_or(|text| text != &message.text)
                })
                .cloned()
                .collect()
        })
    }

    pub fn mark_message_settled(&mut self, message: &keith_protocol::MessageProjection) {
        self.settled_messages
            .insert(message.message_id.clone(), message.text.clone());
    }

    pub fn is_message_settled(&self, message: &keith_protocol::MessageProjection) -> bool {
        self.settled_messages
            .get(&message.message_id)
            .is_some_and(|text| text == &message.text)
    }

    pub fn pending_len(&self) -> usize {
        self.pending_commands.len()
    }

    pub const fn in_flight_len(&self) -> usize {
        self.in_flight_commands
    }

    pub fn command_dispatched(&mut self) {
        self.in_flight_commands = self.in_flight_commands.saturating_add(1);
    }

    pub fn command_finished(&mut self) {
        self.in_flight_commands = self.in_flight_commands.saturating_sub(1);
    }

    pub fn report_command_failure(&mut self, error: impl Into<String>) {
        self.log(format!("Command transport failed: {}", error.into()));
    }

    pub fn report_reconnecting(&mut self) {
        self.connected = false;
        self.reconnecting = true;
    }

    pub fn report_reconnected(&mut self) {
        self.connected = true;
        self.reconnecting = false;
        self.log("Reconnected");
    }

    pub fn report_reconnect_failure(&mut self, error: impl Into<String>) {
        self.connected = false;
        self.reconnecting = false;
        self.log(format!("Reconnect failed: {}", error.into()));
    }

    pub fn next_command(&mut self) -> Option<ClientCommand> {
        self.pending_commands.pop_front()
    }

    pub fn list_sessions(&mut self) {
        self.enqueue(ClientCommand::ListSessions(SessionFilter::default()));
    }

    pub fn select_model(&mut self, provider: String, model: String) {
        let Some(session_id) = self.attached_session.clone() else {
            self.log("Select a session before changing models");
            return;
        };
        self.enqueue(ClientCommand::SelectModel(keith_protocol::ModelSelection {
            session_id,
            provider,
            model,
        }));
    }

    pub fn resolve_confirmation(
        &mut self,
        confirmation_id: EntityId,
        decision: keith_protocol::ConfirmationDecision,
    ) {
        self.enqueue(ClientCommand::ResolveConfirmation(
            keith_protocol::ConfirmationResolution {
                confirmation_id,
                decision,
            },
        ));
    }

    pub fn attach(&mut self, session_id: SessionId) {
        let resume = self.resume_cursor();
        self.attached_session = Some(session_id.clone());
        self.enqueue(ClientCommand::AttachSession(AttachSession {
            session_id,
            resume,
        }));
    }

    pub fn resume_attached_session(&mut self) {
        if let Some(session_id) = self.attached_session.clone() {
            self.attach(session_id);
        }
    }

    pub fn resume_cursor(&self) -> Option<keith_protocol::ResumeCursor> {
        self.reducer.as_ref().map(|reducer| {
            let snapshot = reducer.snapshot();
            keith_protocol::ResumeCursor {
                root_tree_id: snapshot.session.root_tree_id.clone(),
                generation: snapshot.generation,
                last_sequence: snapshot.through_sequence,
            }
        })
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> AppAction {
        if self.overlay.is_some() {
            return self.handle_overlay_key(key);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.handle_control_key(key.code);
        }
        match key.code {
            KeyCode::Tab => {
                self.open_overlay(TuiOverlay::Commands);
                AppAction::Redraw
            }
            KeyCode::BackTab => {
                self.open_overlay(TuiOverlay::Sessions);
                AppAction::Redraw
            }
            KeyCode::Esc => AppAction::Redraw,
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                self.insert_text("\n");
                AppAction::Redraw
            }
            KeyCode::Enter => {
                self.submit_prompt(DeliveryPolicy::Immediate);
                AppAction::Redraw
            }
            KeyCode::Backspace => {
                self.backspace();
                AppAction::Redraw
            }
            KeyCode::Delete => {
                self.delete_forward();
                AppAction::Redraw
            }
            KeyCode::Left => {
                self.move_left();
                AppAction::Redraw
            }
            KeyCode::Right => {
                self.move_right();
                AppAction::Redraw
            }
            KeyCode::Home => {
                self.cursor_byte = 0;
                AppAction::Redraw
            }
            KeyCode::End => {
                self.cursor_byte = self.composer.len();
                AppAction::Redraw
            }
            KeyCode::PageUp => {
                self.scroll_from_end = self.scroll_from_end.saturating_add(10);
                AppAction::Redraw
            }
            KeyCode::PageDown => {
                self.scroll_from_end = self.scroll_from_end.saturating_sub(10);
                AppAction::Redraw
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::ALT | KeyModifiers::SUPER) =>
            {
                let mut encoded = [0_u8; 4];
                self.insert_text(character.encode_utf8(&mut encoded));
                AppAction::Redraw
            }
            _ => AppAction::None,
        }
    }

    pub fn handle_paste(&mut self, pasted: &str) -> AppAction {
        let normalized = pasted.replace("\r\n", "\n").replace('\r', "\n");
        self.insert_text(&normalized);
        AppAction::Redraw
    }

    pub fn overlay_rows(&self) -> Vec<String> {
        let Some(overlay) = self.overlay else {
            return Vec::new();
        };
        let query = self.overlay_query.to_lowercase();
        let matches = |value: &str| query.is_empty() || value.to_lowercase().contains(&query);
        match overlay {
            TuiOverlay::Sessions => self
                .sessions
                .iter()
                .filter_map(|session| {
                    let title = session.title.as_deref().unwrap_or("New conversation");
                    matches(title).then(|| title.to_owned())
                })
                .collect(),
            TuiOverlay::Commands => OVERLAY_COMMANDS
                .iter()
                .filter(|(label, command)| matches(label) || matches(command))
                .map(|(label, command)| format!("{label}  {command}"))
                .collect(),
            TuiOverlay::Models => keith_provider_catalog::BUILTIN_PROVIDERS
                .iter()
                .filter(|provider| {
                    matches(provider.display_name)
                        || matches(provider.id)
                        || matches(provider.default_model)
                })
                .map(|provider| format!("{}  {}", provider.display_name, provider.default_model))
                .collect(),
            TuiOverlay::Approvals => self.reducer.as_ref().map_or_else(Vec::new, |reducer| {
                reducer
                    .snapshot()
                    .confirmations
                    .iter()
                    .filter(|confirmation| matches(&confirmation.summary))
                    .map(|confirmation| confirmation.summary.clone())
                    .collect()
            }),
            TuiOverlay::Work => self.reducer.as_ref().map_or_else(Vec::new, |reducer| {
                let personal = project_personal_intelligence(reducer.snapshot());
                personal
                    .needs_you
                    .iter()
                    .chain(&personal.work)
                    .chain(&personal.upcoming)
                    .chain(&personal.completed)
                    .filter(|item| matches(&item.title) || matches(&item.state_label))
                    .map(|item| format!("{}  {}", item.state_label, item.title))
                    .collect()
            }),
            TuiOverlay::Memory => self.reducer.as_ref().map_or_else(Vec::new, |reducer| {
                project_personal_intelligence(reducer.snapshot())
                    .saved_context
                    .into_iter()
                    .filter(|item| matches(&item.title) || matches(&item.state_label))
                    .map(|item| format!("{}  {}", item.state_label, item.title))
                    .collect()
            }),
            TuiOverlay::Diagnostics => self
                .diagnostic_rows()
                .into_iter()
                .filter(|row| matches(row))
                .collect(),
        }
    }

    fn diagnostic_rows(&self) -> Vec<String> {
        let Some(reducer) = &self.reducer else {
            return vec!["No conversation is attached".into()];
        };
        let snapshot = reducer.snapshot();
        vec![
            format!("Generation {}", snapshot.generation.get()),
            format!("Sequence {}", snapshot.through_sequence.get()),
            format!("Projection revision {}", snapshot.revision.get()),
            format!("Stream {:?}", reducer.stream_state()),
            format!("Composer width {}", self.composer_display_width()),
            format!("Queued commands {}", self.pending_len()),
            format!("In-flight commands {}", self.in_flight_len()),
        ]
    }

    fn handle_overlay_key(&mut self, key: KeyEvent) -> AppAction {
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'q'))
        {
            self.quit = true;
            return AppAction::Quit;
        }
        if key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('a' | 'd'))
            && self.overlay == Some(TuiOverlay::Approvals)
        {
            let allow = key.code == KeyCode::Char('a');
            self.resolve_selected_confirmation(allow);
            self.overlay = None;
            return AppAction::Redraw;
        }
        match key.code {
            KeyCode::Esc => {
                self.overlay = None;
                self.overlay_query.clear();
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let current = self.overlay.unwrap_or(TuiOverlay::Commands);
                let index = TuiOverlay::ALL
                    .iter()
                    .position(|candidate| *candidate == current)
                    .unwrap_or(0);
                let step = if key.code == KeyCode::Tab {
                    1
                } else {
                    TuiOverlay::ALL.len() - 1
                };
                self.open_overlay(TuiOverlay::ALL[(index + step) % TuiOverlay::ALL.len()]);
            }
            KeyCode::Up => self.overlay_selection = self.overlay_selection.saturating_sub(1),
            KeyCode::Down => {
                self.overlay_selection = self
                    .overlay_selection
                    .saturating_add(1)
                    .min(self.overlay_rows().len().saturating_sub(1));
            }
            KeyCode::Backspace => {
                self.overlay_query.pop();
                self.overlay_selection = 0;
            }
            KeyCode::Enter => self.activate_overlay_selection(),
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::ALT | KeyModifiers::SUPER) =>
            {
                self.overlay_query.push(character);
                self.overlay_selection = 0;
            }
            _ => return AppAction::None,
        }
        AppAction::Redraw
    }

    fn activate_overlay_selection(&mut self) {
        match self.overlay {
            Some(TuiOverlay::Sessions) => {
                let query = self.overlay_query.to_lowercase();
                let selected = self
                    .sessions
                    .iter()
                    .filter(|session| {
                        query.is_empty()
                            || session
                                .title
                                .as_deref()
                                .unwrap_or("New conversation")
                                .to_lowercase()
                                .contains(&query)
                    })
                    .nth(self.overlay_selection)
                    .map(|session| session.session_id.clone());
                if let Some(session_id) = selected {
                    self.attach(session_id);
                    self.overlay = None;
                }
            }
            Some(TuiOverlay::Commands) => {
                let query = self.overlay_query.to_lowercase();
                let selected = OVERLAY_COMMANDS
                    .iter()
                    .filter(|(label, command)| {
                        query.is_empty()
                            || label.to_lowercase().contains(&query)
                            || command.contains(&query)
                    })
                    .nth(self.overlay_selection)
                    .map(|(_, command)| *command);
                if let Some(command) = selected {
                    self.apply_palette_command(command);
                }
            }
            Some(TuiOverlay::Models) => {
                let query = self.overlay_query.to_lowercase();
                let selected = keith_provider_catalog::BUILTIN_PROVIDERS
                    .iter()
                    .filter(|provider| {
                        query.is_empty()
                            || provider.display_name.to_lowercase().contains(&query)
                            || provider.id.contains(&query)
                            || provider.default_model.contains(&query)
                    })
                    .nth(self.overlay_selection)
                    .map(|provider| (provider.id.to_owned(), provider.default_model.to_owned()));
                if let Some((provider, model)) = selected {
                    self.select_model(provider, model);
                    self.overlay = None;
                }
            }
            Some(
                TuiOverlay::Approvals
                | TuiOverlay::Work
                | TuiOverlay::Memory
                | TuiOverlay::Diagnostics,
            )
            | None => {}
        }
    }

    fn apply_palette_command(&mut self, command: &str) {
        match command {
            "/sessions" => self.open_overlay(TuiOverlay::Sessions),
            "/models" => self.open_overlay(TuiOverlay::Models),
            "/approvals" => self.open_overlay(TuiOverlay::Approvals),
            "/work" => self.open_overlay(TuiOverlay::Work),
            "/diagnostics" => self.open_overlay(TuiOverlay::Diagnostics),
            "/stop" => {
                if let Some(session_id) = self.attached_session.clone() {
                    self.enqueue(ClientCommand::Cancel(CancelTarget::Session(session_id)));
                }
                self.overlay = None;
            }
            "/resume" => {
                if let Some(session_id) = self.attached_session.clone() {
                    self.enqueue(ClientCommand::ResumeSession { session_id });
                }
                self.overlay = None;
            }
            command => {
                self.replace_composer(command.to_owned());
                self.overlay = None;
            }
        }
    }

    fn resolve_selected_confirmation(&mut self, allow: bool) {
        let query = self.overlay_query.to_lowercase();
        let confirmation = self.reducer.as_ref().and_then(|reducer| {
            reducer
                .snapshot()
                .confirmations
                .iter()
                .filter(|confirmation| {
                    query.is_empty() || confirmation.summary.to_lowercase().contains(&query)
                })
                .nth(self.overlay_selection)
                .map(|confirmation| confirmation.confirmation_id.clone())
        });
        if let Some(confirmation_id) = confirmation {
            self.resolve_confirmation(
                confirmation_id,
                if allow {
                    keith_protocol::ConfirmationDecision::AllowOnce
                } else {
                    keith_protocol::ConfirmationDecision::Deny
                },
            );
        }
    }

    pub fn replace_composer(&mut self, content: String) {
        self.composer = bounded_text(content, MAX_COMPOSER_BYTES);
        self.cursor_byte = self.composer.len();
    }

    pub fn apply_wire_message(&mut self, message: WireMessage) {
        match message {
            WireMessage::ServerHello(_) => {
                self.connected = true;
                self.reconnecting = false;
                self.log("Connected");
            }
            WireMessage::CommandResult(result) => match result.result {
                CommandResult::Data(payload) => match *payload {
                    ResponsePayload::Sessions(sessions) => {
                        let first = sessions.first().map(|session| session.session_id.clone());
                        self.sessions = sessions;
                        if self.attached_session.is_none()
                            && let Some(session_id) = first
                        {
                            self.attach(session_id);
                        }
                    }
                    ResponsePayload::Snapshot(snapshot) => self.apply_snapshot(*snapshot),
                    other => self.log(format!("Received {} projection", payload_label(&other))),
                },
                CommandResult::Accepted { .. } => self.log("Command accepted"),
                CommandResult::Rejected(error) => {
                    self.log(format!("Command rejected: {}", error.error.message));
                }
            },
            WireMessage::Event(envelope) => {
                let acknowledgement = EventAcknowledgement {
                    root_tree_id: envelope.root_tree_id.clone(),
                    generation: envelope.generation,
                    through_sequence: envelope.sequence,
                };
                if let Some(reducer) = &mut self.reducer {
                    match reducer.apply_event(&envelope) {
                        Ok(
                            ReductionOutcome::Applied | ReductionOutcome::AppliedCoalesced { .. },
                        ) => {
                            self.enqueue(ClientCommand::AcknowledgeEvents(acknowledgement));
                        }
                        Ok(ReductionOutcome::Gap) => {
                            self.reconnecting = true;
                            if let Some(session_id) = self.attached_session.clone() {
                                self.attach(session_id);
                            }
                        }
                        Ok(
                            ReductionOutcome::SnapshotReplaced
                            | ReductionOutcome::Duplicate
                            | ReductionOutcome::StaleGeneration,
                        ) => {}
                        Err(error) => self.log(format!("Projection error: {error}")),
                    }
                } else if let DaemonEvent::Snapshot(snapshot) = envelope.event {
                    self.apply_snapshot(*snapshot);
                }
            }
            message @ (WireMessage::Snapshot(_) | WireMessage::Terminal(_)) => {
                if let Some(envelope) = message.into_event() {
                    self.apply_wire_message(WireMessage::Event(envelope));
                }
            }
            WireMessage::ClientHello(_) | WireMessage::Command(_) => {}
        }
    }

    pub fn command_envelope(&self, command: ClientCommand) -> CommandEnvelope {
        CommandEnvelope {
            protocol: keith_agent_types::CURRENT_PROTOCOL_VERSION,
            command_id: CommandId::new(),
            client_id: self.client_id.clone(),
            sent_at: UtcTimestamp::now().unwrap_or(UtcTimestamp::UNIX_EPOCH),
            session_id: self.attached_session.clone(),
            command,
        }
    }

    pub fn composer_display_width(&self) -> usize {
        UnicodeWidthStr::width(self.composer.as_str())
    }

    fn handle_control_key(&mut self, code: KeyCode) -> AppAction {
        match code {
            KeyCode::Char('c' | 'q') => {
                self.quit = true;
                AppAction::Quit
            }
            KeyCode::Char('e') => AppAction::OpenExternalEditor,
            KeyCode::Char('k') => {
                self.steer();
                AppAction::Redraw
            }
            KeyCode::Char('x') => {
                if let Some(session_id) = self.attached_session.clone() {
                    self.enqueue(ClientCommand::Cancel(CancelTarget::Session(session_id)));
                    self.log("Cancellation requested for the active turn");
                }
                AppAction::Redraw
            }
            KeyCode::Char('r') => {
                if let Some(prompt) = self.last_prompt.clone() {
                    self.composer = prompt;
                    self.cursor_byte = self.composer.len();
                    self.submit_prompt(DeliveryPolicy::Immediate);
                }
                AppAction::Redraw
            }
            KeyCode::Char('b') => {
                self.branch_from_latest();
                AppAction::Redraw
            }
            KeyCode::Char('s') => {
                self.open_overlay(TuiOverlay::Sessions);
                AppAction::Redraw
            }
            KeyCode::Char('p') => {
                self.open_overlay(TuiOverlay::Commands);
                AppAction::Redraw
            }
            KeyCode::Char('m') => {
                self.open_overlay(TuiOverlay::Models);
                AppAction::Redraw
            }
            KeyCode::Char('g') => {
                self.open_overlay(TuiOverlay::Work);
                AppAction::Redraw
            }
            KeyCode::Char('y') => {
                self.open_overlay(TuiOverlay::Memory);
                AppAction::Redraw
            }
            KeyCode::Char('d') => {
                self.open_overlay(TuiOverlay::Approvals);
                AppAction::Redraw
            }
            KeyCode::Char('u') => {
                if let Some(session_id) = self.attached_session.clone() {
                    self.enqueue(ClientCommand::ResumeSession { session_id });
                }
                AppAction::Redraw
            }
            _ => AppAction::None,
        }
    }

    fn submit_prompt(&mut self, delivery: DeliveryPolicy) {
        let text = self.composer.trim().to_owned();
        if text.is_empty() {
            return;
        }
        let Some(session_id) = self.attached_session.clone() else {
            self.log("Select a session before sending a prompt");
            return;
        };
        if let Some(selection) = text.strip_prefix("/model ") {
            let mut parts = selection.split_whitespace();
            let Some(provider) = parts.next() else {
                self.log("Usage: /model <provider> [model]");
                return;
            };
            let Some(provider_spec) = keith_provider_catalog::provider(provider) else {
                self.log("Unknown provider. Open the Models view for supported provider IDs");
                return;
            };
            let model = parts.next().unwrap_or(provider_spec.default_model);
            if parts.next().is_some() {
                self.log("Usage: /model <provider> [model]");
                return;
            }
            self.select_model(provider.to_owned(), model.to_owned());
            self.composer.clear();
            self.cursor_byte = 0;
            return;
        }
        if text.starts_with('/') && self.handle_slash_command(&session_id, &text) {
            self.composer.clear();
            self.cursor_byte = 0;
            return;
        }
        self.enqueue(ClientCommand::SubmitPrompt(SubmitPrompt {
            session_id,
            text: text.clone(),
            artifacts: Vec::new(),
            delivery,
            reply_route: None,
        }));
        self.last_prompt = Some(text);
        self.composer.clear();
        self.cursor_byte = 0;
    }

    #[allow(clippy::too_many_lines)]
    fn handle_slash_command(&mut self, session_id: &SessionId, input: &str) -> bool {
        let (command, argument) = input
            .split_once(' ')
            .map_or((input, ""), |(command, argument)| {
                (command, argument.trim())
            });
        let limits = || GoalLimits {
            max_turns: Some(100),
            max_tokens: Some(1_000_000),
            deadline: None,
        };
        match command {
            "/sessions" => self.open_overlay(TuiOverlay::Sessions),
            "/commands" | "/help" => self.open_overlay(TuiOverlay::Commands),
            "/models" => self.open_overlay(TuiOverlay::Models),
            "/approvals" => self.open_overlay(TuiOverlay::Approvals),
            "/work" => self.open_overlay(TuiOverlay::Work),
            "/memory" if argument.is_empty() => self.open_overlay(TuiOverlay::Memory),
            "/diagnostics" => self.open_overlay(TuiOverlay::Diagnostics),
            "/stop" => self.enqueue(ClientCommand::Cancel(CancelTarget::Session(
                session_id.clone(),
            ))),
            "/resume" => self.enqueue(ClientCommand::ResumeSession {
                session_id: session_id.clone(),
            }),
            "/goal" if !argument.is_empty() => {
                self.enqueue(ClientCommand::CreateGoal(CreateGoal {
                    session_id: session_id.clone(),
                    objective: argument.into(),
                    limits: limits(),
                }));
            }
            "/goals" => self.enqueue(ClientCommand::ListGoals {
                session_id: session_id.clone(),
            }),
            "/child" if !argument.is_empty() => {
                self.enqueue(ClientCommand::CreateChild(CreateChild {
                    parent_session_id: session_id.clone(),
                    objective: argument.into(),
                    workspace_mode: ChildWorkspaceMode::SharedWorkspace,
                    limits: limits(),
                }));
            }
            "/children" => self.enqueue(ClientCommand::ListChildren {
                session_id: session_id.clone(),
            }),
            "/child-message" => {
                let Some((id, text)) = argument.split_once(' ') else {
                    self.log("Usage: /child-message <child-id> <text>");
                    return true;
                };
                let Ok(child_id) = id.parse::<ChildId>() else {
                    self.log("Child identifier is invalid");
                    return true;
                };
                self.enqueue(ClientCommand::SendChildMessage(ChildMessageRequest {
                    child_id,
                    text: text.trim().into(),
                    artifact_ids: Vec::new(),
                }));
            }
            "/archive-child" => {
                let Ok(child_id) = argument.parse::<ChildId>() else {
                    self.log("Usage: /archive-child <child-id>");
                    return true;
                };
                self.enqueue(ClientCommand::ArchiveChild { child_id });
            }
            "/memory" if !argument.is_empty() => {
                let Some(profile_id) = self
                    .reducer
                    .as_ref()
                    .map(|reducer| reducer.snapshot().session.profile_id.clone())
                else {
                    self.log("Attach a session before querying memory");
                    return true;
                };
                self.enqueue(ClientCommand::QueryMemory(MemoryQuery {
                    profile_id,
                    query: argument.into(),
                    limit: 20,
                }));
            }
            "/schedule" => {
                let Some((seconds, prompt)) = argument.split_once(' ') else {
                    self.log("Usage: /schedule <interval-seconds> <prompt>");
                    return true;
                };
                let Ok(seconds) = seconds.parse::<u64>() else {
                    self.log("Schedule interval must be an integer number of seconds");
                    return true;
                };
                let Some(profile_id) = self
                    .reducer
                    .as_ref()
                    .map(|reducer| reducer.snapshot().session.profile_id.clone())
                else {
                    self.log("Attach a session before creating a schedule");
                    return true;
                };
                self.enqueue(ClientCommand::CreateSchedule(CreateSchedule {
                    profile_id,
                    session_id: Some(session_id.clone()),
                    expression: ScheduleExpression::IntervalSeconds(seconds),
                    time_zone: "UTC".into(),
                    prompt: prompt.trim().into(),
                    reply_route: None,
                }));
            }
            "/pause-schedule" | "/resume-schedule" => {
                let Ok(job_id) = argument.parse::<JobId>() else {
                    self.log("Usage: /pause-schedule <job-id> or /resume-schedule <job-id>");
                    return true;
                };
                self.enqueue(ClientCommand::UpdateSchedule(UpdateSchedule {
                    job_id,
                    expression: None,
                    prompt: None,
                    paused: Some(command == "/pause-schedule"),
                }));
            }
            "/delete-schedule" => {
                let Ok(job_id) = argument.parse::<JobId>() else {
                    self.log("Usage: /delete-schedule <job-id>");
                    return true;
                };
                self.enqueue(ClientCommand::DeleteSchedule { job_id });
            }
            "/export" => self.enqueue(ClientCommand::Export(ExportRequest {
                session_id: session_id.clone(),
                format: match argument {
                    "jsonl" => ExportFormat::JsonLines,
                    "markdown" | "md" => ExportFormat::Markdown,
                    "" | "bundle" => ExportFormat::PortableBundle,
                    _ => {
                        self.log("Usage: /export [jsonl|markdown|bundle]");
                        return true;
                    }
                },
                include_artifacts: true,
            })),
            "/select-branch" => {
                let Ok(leaf_entry_id) = argument.parse::<EntityId>() else {
                    self.log("Usage: /select-branch <entry-id>");
                    return true;
                };
                self.enqueue(ClientCommand::SelectBranch(keith_protocol::SelectBranch {
                    session_id: session_id.clone(),
                    leaf_entry_id,
                }));
            }
            "/cancel-goal" => {
                let Ok(goal_id) = argument.parse::<GoalId>() else {
                    self.log("Usage: /cancel-goal <goal-id>");
                    return true;
                };
                self.enqueue(ClientCommand::Cancel(CancelTarget::Goal(goal_id)));
            }
            "/cancel-child" => {
                let Ok(child_id) = argument.parse::<ChildId>() else {
                    self.log("Usage: /cancel-child <child-id>");
                    return true;
                };
                self.enqueue(ClientCommand::Cancel(CancelTarget::Child(child_id)));
            }
            "/background" => {
                let mode = match argument {
                    "disabled" | "off" => BackgroundMode::Disabled,
                    "suggest" => BackgroundMode::Suggest,
                    "confirm" => BackgroundMode::ConfirmSelected,
                    "bounded" => BackgroundMode::Bounded,
                    _ => {
                        self.log("Usage: /background <disabled|suggest|confirm|bounded>");
                        return true;
                    }
                };
                let Some(profile_id) = self
                    .reducer
                    .as_ref()
                    .map(|reducer| reducer.snapshot().session.profile_id.clone())
                else {
                    self.log("Attach a session before changing background control");
                    return true;
                };
                self.enqueue(ClientCommand::SetBackgroundControl(BackgroundControl {
                    profile_id,
                    mode,
                    pause_until: None,
                }));
            }
            _ if input.starts_with('/') => {
                self.log("Unknown command. Use /help for available commands");
            }
            _ => return false,
        }
        true
    }

    fn steer(&mut self) {
        let text = self.composer.trim().to_owned();
        let Some(session_id) = self.attached_session.clone() else {
            return;
        };
        if text.is_empty() {
            return;
        }
        self.enqueue(ClientCommand::Steer(SteerAction {
            session_id,
            text,
            delivery: DeliveryPolicy::NextTurnBoundary,
        }));
        self.composer.clear();
        self.cursor_byte = 0;
    }

    fn branch_from_latest(&mut self) {
        let Some(reducer) = &self.reducer else {
            return;
        };
        let Some(message) = reducer.snapshot().messages.last() else {
            return;
        };
        let Some(parent_entry_id) = message.final_id.clone() else {
            self.log("A completed Keith reply is required before branching");
            return;
        };
        let Some(session_id) = self.attached_session.clone() else {
            return;
        };
        self.enqueue(ClientCommand::BranchSession(
            keith_protocol::BranchRequest {
                session_id,
                parent_entry_id: parent_entry_id.0,
                label: None,
            },
        ));
    }

    fn apply_snapshot(&mut self, snapshot: keith_protocol::SessionSnapshot) {
        let virtualization = VirtualizationConfig::new(2_048, 256, 16)
            .expect("fixed virtualization limits are valid");
        let replace = self.reducer.as_ref().is_some_and(|reducer| {
            reducer.snapshot().session.session_id != snapshot.session.session_id
                || reducer.snapshot().session.root_tree_id != snapshot.session.root_tree_id
        });
        if replace {
            self.reducer = None;
            self.settled_messages.clear();
        }
        match &mut self.reducer {
            Some(reducer) => {
                if let Err(error) = reducer.apply_snapshot(snapshot) {
                    self.log(format!("Snapshot rejected: {error}"));
                }
            }
            None => match ProjectionReducer::new(snapshot, virtualization) {
                Ok(reducer) => self.reducer = Some(reducer),
                Err(error) => self.log(format!("Snapshot rejected: {error}")),
            },
        }
    }

    fn enqueue(&mut self, command: ClientCommand) {
        if self.pending_commands.len() == MAX_PENDING_COMMANDS {
            self.log("Command queue is full");
            return;
        }
        self.pending_commands.push_back(command);
    }

    fn log(&mut self, message: impl Into<String>) {
        self.logs.push_back(message.into());
        while self.logs.len() > MAX_LOG_LINES {
            self.logs.pop_front();
        }
    }

    fn insert_text(&mut self, text: &str) {
        let available = MAX_COMPOSER_BYTES.saturating_sub(self.composer.len());
        let text = bounded_str(text, available);
        self.composer.insert_str(self.cursor_byte, text);
        self.cursor_byte += text.len();
    }

    fn backspace(&mut self) {
        if self.cursor_byte == 0 {
            return;
        }
        let previous = self.composer[..self.cursor_byte]
            .char_indices()
            .next_back()
            .map_or(0, |(index, _)| index);
        self.composer.drain(previous..self.cursor_byte);
        self.cursor_byte = previous;
    }

    fn delete_forward(&mut self) {
        if self.cursor_byte == self.composer.len() {
            return;
        }
        let width = self.composer[self.cursor_byte..]
            .chars()
            .next()
            .map_or(0, char::len_utf8);
        self.composer
            .drain(self.cursor_byte..self.cursor_byte + width);
    }

    fn move_left(&mut self) {
        if self.cursor_byte > 0 {
            self.cursor_byte = self.composer[..self.cursor_byte]
                .char_indices()
                .next_back()
                .map_or(0, |(index, _)| index);
        }
    }

    fn move_right(&mut self) {
        if self.cursor_byte < self.composer.len() {
            self.cursor_byte += self.composer[self.cursor_byte..]
                .chars()
                .next()
                .map_or(0, char::len_utf8);
        }
    }
}

fn payload_label(payload: &ResponsePayload) -> &'static str {
    match payload {
        ResponsePayload::Profiles(_) => "profile",
        ResponsePayload::Sessions(_) => "session",
        ResponsePayload::Snapshot(_) => "snapshot",
        ResponsePayload::Goal(_) => "goal",
        ResponsePayload::Child(_) => "child",
        ResponsePayload::Schedule(_) => "schedule",
        ResponsePayload::Memory(_) => "memory",
        ResponsePayload::Export(_) => "export",
        ResponsePayload::Background(_) => "background",
        ResponsePayload::Artifact(_) => "artifact",
        ResponsePayload::DeliveryClaim(_) => "delivery",
    }
}

fn bounded_text(text: String, limit: usize) -> String {
    if text.len() <= limit {
        text
    } else {
        bounded_str(&text, limit).to_owned()
    }
}

fn bounded_str(text: &str, limit: usize) -> &str {
    let mut boundary = limit.min(text.len());
    while !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &text[..boundary]
}

pub fn selected_entry_id(app: &TuiApp) -> Option<EntityId> {
    app.reducer
        .as_ref()?
        .snapshot()
        .messages
        .last()
        .map(|message| message.message_id.as_entity_id().clone())
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEventKind, KeyEventState};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use super::*;

    fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    fn rendered(app: &TuiApp, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal.draw(|frame| render(frame, app)).unwrap();
        let buffer = terminal.backend().buffer();
        let mut output = String::new();
        for y in 0..height {
            for x in 0..width {
                output.push_str(buffer.cell((x, y)).unwrap().symbol());
            }
            output.push('\n');
        }
        output
    }

    #[test]
    fn keyboard_paste_unicode_and_command_intents_are_bounded() {
        let mut app = TuiApp::new(Accessibility::default());
        app.attach(SessionId::new());
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::AttachSession(_))
        ));
        for character in ['H', 'é', '界'] {
            app.handle_key(key(KeyCode::Char(character), KeyModifiers::NONE));
        }
        app.handle_key(key(KeyCode::Enter, KeyModifiers::ALT));
        app.handle_paste("line\r\nsecond");
        assert_eq!(app.composer, "Hé界\nline\nsecond");
        assert_eq!(app.cursor_byte, app.composer.len());
        assert!(app.composer_display_width() >= 14);
        app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::SubmitPrompt(SubmitPrompt { text, .. }))
                if text == "Hé界\nline\nsecond"
        ));
        assert!(app.composer.is_empty());

        app.handle_paste(&"x".repeat(MAX_COMPOSER_BYTES + 10));
        assert_eq!(app.composer.len(), MAX_COMPOSER_BYTES);
        app.handle_key(key(KeyCode::Backspace, KeyModifiers::NONE));
        assert_eq!(app.composer.len(), MAX_COMPOSER_BYTES - 1);
        assert_eq!(
            app.handle_key(key(KeyCode::Char('e'), KeyModifiers::CONTROL)),
            AppAction::OpenExternalEditor
        );
    }

    #[test]
    fn every_temporary_overlay_is_searchable_and_conversation_survives_all_widths() {
        assert!(client_parity().is_full());
        for mode in [
            ColorMode::TrueColor,
            ColorMode::Ansi256,
            ColorMode::NoColor,
            ColorMode::HighContrast,
        ] {
            let mut app = TuiApp::new(Accessibility {
                color_mode: mode,
                reduced_motion: true,
            });
            let mut visited = Vec::new();
            for overlay in TuiOverlay::ALL {
                app.open_overlay(overlay);
                visited.push(app.overlay.unwrap());
                let wide = rendered(&app, 120, 32);
                let narrow = rendered(&app, 60, 20);
                let tiny = rendered(&app, 36, 8);
                assert!(wide.contains(overlay.label()));
                assert!(narrow.contains(overlay.label()));
                assert!(tiny.contains("Keith"));
                for forbidden in ["┌", "┐", "└", "┘", "│", "─"] {
                    assert!(!wide.contains(forbidden));
                }
                app.handle_key(key(KeyCode::Char('q'), KeyModifiers::NONE));
                assert_eq!(app.overlay_query, "q");
                app.handle_key(key(KeyCode::Esc, KeyModifiers::NONE));
                assert!(app.overlay.is_none());
            }
            assert_eq!(visited, TuiOverlay::ALL);
        }
        let source = include_str!("render.rs").to_ascii_lowercase();
        assert!(!source.contains("purple"));
        assert!(!source.contains("glow"));
        assert!(!source.contains(".borders("));
    }

    #[test]
    fn terminal_control_sequences_are_neutralized_before_rendering() {
        let output = render::terminal_safe("safe\u{1b}[2J\u{7}still visible");
        assert_eq!(output, "safe�[2J�still visible");
    }

    #[test]
    fn settled_transcript_wraps_unicode_and_preserves_code_indentation() {
        let message = keith_protocol::MessageProjection {
            message_id: MessageId::new(),
            final_id: Some(keith_agent_types::EntryId::new()),
            role: keith_protocol::MessageRole::Assistant,
            text: "result\n    let value = \"界界界界界\";\u{1b}[2J".into(),
            committed: true,
        };
        let rendered = settled_transcript_lines(&message, 18, ColorMode::NoColor)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>();
        assert_eq!(rendered.first().map(String::as_str), Some("  Keith"));
        assert!(rendered.iter().any(|line| line.starts_with("      let")));
        assert!(rendered.iter().any(|line| line.contains('�')));
        assert!(rendered.len() >= 4);
    }

    #[test]
    fn queue_retry_branch_resume_and_navigation_remain_protocol_commands() {
        let mut app = TuiApp::new(Accessibility::default());
        let session_id = SessionId::new();
        app.attach(session_id.clone());
        app.next_command();
        app.replace_composer("first prompt".into());
        app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::SubmitPrompt(_))
        ));
        app.handle_key(key(KeyCode::Char('r'), KeyModifiers::CONTROL));
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::SubmitPrompt(_))
        ));
        app.handle_key(key(KeyCode::Char('x'), KeyModifiers::CONTROL));
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::Cancel(CancelTarget::Session(id))) if id == session_id
        ));
        app.handle_key(key(KeyCode::Char('u'), KeyModifiers::CONTROL));
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::ResumeSession { session_id: id }) if id == session_id
        ));
        app.select_model("provider".into(), "model".into());
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::SelectModel(selection))
                if selection.provider == "provider" && selection.model == "model"
        ));
        app.replace_composer("/model openai gpt-4.1-mini".into());
        app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::SelectModel(selection))
                if selection.provider == "openai" && selection.model == "gpt-4.1-mini"
        ));
        app.replace_composer("/model deepseek".into());
        app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::SelectModel(selection))
                if selection.provider == "deepseek" && selection.model == "deepseek-chat"
        ));
        app.replace_composer("/goal Ship the integration".into());
        app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::CreateGoal(CreateGoal { objective, .. }))
                if objective == "Ship the integration"
        ));
        app.replace_composer("/child Verify the integration".into());
        app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::CreateChild(CreateChild { objective, .. }))
                if objective == "Verify the integration"
        ));
        app.replace_composer("/export markdown".into());
        app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::Export(ExportRequest {
                format: ExportFormat::Markdown,
                ..
            }))
        ));
        let goal_id = GoalId::new();
        app.replace_composer(format!("/cancel-goal {goal_id}"));
        app.handle_key(key(KeyCode::Enter, KeyModifiers::NONE));
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::Cancel(CancelTarget::Goal(id))) if id == goal_id
        ));
        let confirmation_id = EntityId::new();
        app.resolve_confirmation(
            confirmation_id.clone(),
            keith_protocol::ConfirmationDecision::AllowOnce,
        );
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::ResolveConfirmation(resolution))
                if resolution.confirmation_id == confirmation_id
        ));
        app.handle_key(key(KeyCode::Char('s'), KeyModifiers::CONTROL));
        assert_eq!(app.overlay, Some(TuiOverlay::Sessions));
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::ListSessions(_))
        ));
        app.handle_key(key(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.overlay.is_none());
        app.handle_key(key(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.overlay, Some(TuiOverlay::Commands));
    }
}
