#![forbid(unsafe_code)]

mod connection;
mod render;

pub use connection::*;
pub use render::render;

use std::collections::VecDeque;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use keith_agent_types::{ClientId, CommandId, EntityId, SessionId, UtcTimestamp};
use keith_protocol::{
    AttachSession, CancelTarget, ClientCommand, CommandEnvelope, CommandResult, DaemonEvent,
    DeliveryPolicy, EventAcknowledgement, ResponsePayload, SessionFilter, SessionSummary,
    SteerAction, SubmitPrompt, WireMessage,
};
use keith_ui_model::{ProjectionReducer, ReductionOutcome, VirtualizationConfig};
use unicode_width::UnicodeWidthStr;

pub const MAX_COMPOSER_BYTES: usize = 64 * 1_024;
pub const MAX_LOG_LINES: usize = 512;
pub const MAX_PENDING_COMMANDS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Surface {
    Chat,
    Queue,
    Sessions,
    Models,
    Goals,
    Plans,
    Children,
    Tools,
    Kernels,
    Artifacts,
    Schedules,
    Commitments,
    Waiting,
    Confirmations,
    Memory,
    Knowledge,
    Channels,
    Refinement,
    Logs,
    Diagnostics,
}

impl Surface {
    pub const ALL: [Self; 20] = [
        Self::Chat,
        Self::Queue,
        Self::Sessions,
        Self::Models,
        Self::Goals,
        Self::Plans,
        Self::Children,
        Self::Tools,
        Self::Kernels,
        Self::Artifacts,
        Self::Schedules,
        Self::Commitments,
        Self::Waiting,
        Self::Confirmations,
        Self::Memory,
        Self::Knowledge,
        Self::Channels,
        Self::Refinement,
        Self::Logs,
        Self::Diagnostics,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Chat => "Chat",
            Self::Queue => "Queue",
            Self::Sessions => "Sessions",
            Self::Models => "Models",
            Self::Goals => "Goals",
            Self::Plans => "Plans",
            Self::Children => "Children",
            Self::Tools => "Tools",
            Self::Kernels => "Kernels",
            Self::Artifacts => "Artifacts",
            Self::Schedules => "Schedules",
            Self::Commitments => "Commitments",
            Self::Waiting => "Waiting",
            Self::Confirmations => "Confirmations",
            Self::Memory => "Memory",
            Self::Knowledge => "Knowledge",
            Self::Channels => "Channels",
            Self::Refinement => "Refinement",
            Self::Logs => "Logs",
            Self::Diagnostics => "Diagnostics",
        }
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

pub struct TuiApp {
    pub client_id: ClientId,
    pub surface: Surface,
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
    pending_commands: VecDeque<ClientCommand>,
    logs: VecDeque<String>,
}

impl TuiApp {
    pub fn new(accessibility: Accessibility) -> Self {
        Self {
            client_id: ClientId::new(),
            surface: Surface::Chat,
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
            pending_commands: VecDeque::new(),
            logs: VecDeque::new(),
        }
    }

    pub fn logs(&self) -> &VecDeque<String> {
        &self.logs
    }

    pub fn pending_len(&self) -> usize {
        self.pending_commands.len()
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
        let resume = self.reducer.as_ref().map(|reducer| {
            let snapshot = reducer.snapshot();
            keith_protocol::ResumeCursor {
                root_tree_id: snapshot.session.root_tree_id.clone(),
                generation: snapshot.generation,
                last_sequence: snapshot.through_sequence,
            }
        });
        self.attached_session = Some(session_id.clone());
        self.enqueue(ClientCommand::AttachSession(AttachSession {
            session_id,
            resume,
        }));
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> AppAction {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.handle_control_key(key.code);
        }
        match key.code {
            KeyCode::Tab => {
                self.next_surface();
                AppAction::Redraw
            }
            KeyCode::BackTab => {
                self.previous_surface();
                AppAction::Redraw
            }
            KeyCode::Esc => {
                self.surface = Surface::Chat;
                AppAction::Redraw
            }
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
                    ResponsePayload::Sessions(sessions) => self.sessions = sessions,
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
                self.surface = Surface::Sessions;
                self.list_sessions();
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
        self.enqueue(ClientCommand::SubmitPrompt(SubmitPrompt {
            session_id,
            text: text.clone(),
            delivery,
            reply_route: None,
        }));
        self.last_prompt = Some(text);
        self.composer.clear();
        self.cursor_byte = 0;
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
        let Some(session_id) = self.attached_session.clone() else {
            return;
        };
        self.enqueue(ClientCommand::BranchSession(
            keith_protocol::BranchRequest {
                session_id,
                parent_entry_id: message.message_id.as_entity_id().clone(),
                label: None,
            },
        ));
    }

    fn apply_snapshot(&mut self, snapshot: keith_protocol::SessionSnapshot) {
        let virtualization = VirtualizationConfig::new(2_048, 256, 16)
            .expect("fixed virtualization limits are valid");
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

    fn next_surface(&mut self) {
        let index = Surface::ALL
            .iter()
            .position(|surface| *surface == self.surface)
            .unwrap_or(0);
        self.surface = Surface::ALL[(index + 1) % Surface::ALL.len()];
    }

    fn previous_surface(&mut self) {
        let index = Surface::ALL
            .iter()
            .position(|surface| *surface == self.surface)
            .unwrap_or(0);
        self.surface = Surface::ALL[(index + Surface::ALL.len() - 1) % Surface::ALL.len()];
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
    fn every_operator_surface_is_keyboard_reachable_and_renderable_at_all_widths() {
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
            for _ in 0..Surface::ALL.len() {
                visited.push(app.surface);
                let wide = rendered(&app, 120, 32);
                let narrow = rendered(&app, 60, 20);
                let tiny = rendered(&app, 36, 8);
                assert!(wide.contains(app.surface.label()));
                assert!(narrow.contains(app.surface.label()));
                assert!(tiny.contains(app.surface.label()));
                for forbidden in ["┌", "┐", "└", "┘", "│", "─"] {
                    assert!(!wide.contains(forbidden));
                }
                app.handle_key(key(KeyCode::Tab, KeyModifiers::NONE));
            }
            assert_eq!(visited, Surface::ALL);
        }
        let source = include_str!("render.rs").to_ascii_lowercase();
        assert!(!source.contains("purple"));
        assert!(!source.contains("glow"));
        assert!(!source.contains(".borders("));
    }

    #[test]
    fn queue_cancel_retry_branch_resume_and_navigation_remain_protocol_commands() {
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
        assert_eq!(app.surface, Surface::Sessions);
        assert!(matches!(
            app.next_command(),
            Some(ClientCommand::ListSessions(_))
        ));
        app.handle_key(key(KeyCode::BackTab, KeyModifiers::NONE));
        assert_eq!(app.surface, Surface::Queue);
        app.handle_key(key(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.surface, Surface::Chat);
    }
}
