use std::collections::HashMap;
use std::str::FromStr;

use keith_agent_types::{
    CURRENT_PROTOCOL_VERSION, ClientId, CommandId, EntityId, ProfileId, SessionId, UtcTimestamp,
    WorkspaceId,
};
use keith_protocol::{
    AttachSession, BackgroundControl, BackgroundMode, BranchRequest, CancelTarget,
    ChildWorkspaceMode, ClientCommand, CommandEnvelope, CommandResult, ConfirmationDecision,
    ConfirmationResolution, CreateChild, CreateGoal, CreateSchedule, CreateSession, DeliveryPolicy,
    ExportFormat, ExportRequest, GoalLimits, MemoryQuery, ModelSelection, ResponsePayload,
    ScheduleExpression, SessionFilter, SteerAction, SubmitPrompt, WireMessage,
};
use keith_ui_model::{
    PersonalIntelligenceProjection, ProjectionReducer, ReductionOutcome, VirtualizationConfig,
    project_personal_intelligence,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct BrowserView<'a> {
    snapshot: Option<&'a keith_protocol::SessionSnapshot>,
    personal: Option<PersonalIntelligenceProjection>,
    resume: Option<keith_protocol::ResumeCursor>,
    sessions: &'a [keith_protocol::SessionSummary],
    last_command: Option<&'a BrowserCommandReceipt>,
    snapshot_required: bool,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum BrowserCommandState {
    Accepted,
    Updated,
    Rejected,
}

#[derive(Serialize)]
struct BrowserCommandReceipt {
    state: BrowserCommandState,
    message: String,
}

/// The browser's only protocol authority. Solid renders the serialized projection and sends the
/// typed envelopes built here; it never reduces daemon events or constructs wire commands.
#[wasm_bindgen]
pub struct BrowserProjection {
    client_id: ClientId,
    reducer: Option<ProjectionReducer>,
    sessions: Vec<keith_protocol::SessionSummary>,
    snapshot_required: bool,
    last_prompt: Option<String>,
    pending_prompts: HashMap<CommandId, String>,
    last_command: Option<BrowserCommandReceipt>,
}

#[wasm_bindgen]
impl BrowserProjection {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            client_id: ClientId::new(),
            reducer: None,
            sessions: Vec::new(),
            snapshot_required: false,
            last_prompt: None,
            pending_prompts: HashMap::new(),
            last_command: None,
        }
    }

    pub fn apply_wire_message(&mut self, encoded: &str) -> Result<String, JsValue> {
        let message = serde_json::from_str::<WireMessage>(encoded).map_err(js_error)?;
        self.reduce(message)?;
        self.current_view()
    }

    pub fn current_view(&self) -> Result<String, JsValue> {
        let snapshot = self.reducer.as_ref().map(ProjectionReducer::snapshot);
        let personal = snapshot.map(project_personal_intelligence);
        let resume = snapshot
            .filter(|_| !self.snapshot_required)
            .map(|snapshot| keith_protocol::ResumeCursor {
                root_tree_id: snapshot.session.root_tree_id.clone(),
                generation: snapshot.generation,
                last_sequence: snapshot.through_sequence,
            });
        serde_json::to_string(&BrowserView {
            snapshot,
            personal,
            resume,
            sessions: &self.sessions,
            last_command: self.last_command.as_ref(),
            snapshot_required: self.snapshot_required,
        })
        .map_err(js_error)
    }

    pub fn list_sessions(&self, profile_id: Option<String>) -> Result<String, JsValue> {
        let profile_id = profile_id
            .map(|value| ProfileId::from_str(&value).map_err(js_error))
            .transpose()?;
        self.command(
            None,
            ClientCommand::ListSessions(SessionFilter {
                profile_id,
                include_archived: false,
            }),
        )
    }

    pub fn create_session(
        &self,
        profile_id: &str,
        workspace_id: &str,
        title: Option<String>,
    ) -> Result<String, JsValue> {
        self.command(
            None,
            ClientCommand::CreateSession(CreateSession {
                profile_id: parse(profile_id)?,
                workspace_id: parse::<WorkspaceId>(workspace_id)?,
                title: title.filter(|value| !value.trim().is_empty()),
            }),
        )
    }

    pub fn attach_session(&self, session_id: &str) -> Result<String, JsValue> {
        let session_id = parse::<SessionId>(session_id)?;
        let resume = self.reducer.as_ref().and_then(|reducer| {
            let snapshot = reducer.snapshot();
            (snapshot.session.session_id == session_id && !self.snapshot_required).then(|| {
                keith_protocol::ResumeCursor {
                    root_tree_id: snapshot.session.root_tree_id.clone(),
                    generation: snapshot.generation,
                    last_sequence: snapshot.through_sequence,
                }
            })
        });
        self.command(
            Some(session_id.clone()),
            ClientCommand::AttachSession(AttachSession { session_id, resume }),
        )
    }

    pub fn submit_prompt(&mut self, session_id: &str, text: String) -> Result<String, JsValue> {
        let session_id = parse::<SessionId>(session_id)?;
        let text = nonempty(text, "message")?;
        let envelope = self.command_envelope(
            Some(session_id.clone()),
            ClientCommand::SubmitPrompt(SubmitPrompt {
                session_id,
                text: text.clone(),
                artifacts: Vec::new(),
                delivery: DeliveryPolicy::Immediate,
                reply_route: None,
            }),
        );
        self.pending_prompts
            .insert(envelope.command_id.clone(), text);
        encode_command(&envelope)
    }

    pub fn steer(&self, session_id: &str, text: String) -> Result<String, JsValue> {
        let session_id = parse::<SessionId>(session_id)?;
        self.command(
            Some(session_id.clone()),
            ClientCommand::Steer(SteerAction {
                session_id,
                text: nonempty(text, "steering message")?,
                delivery: DeliveryPolicy::NextTurnBoundary,
            }),
        )
    }

    pub fn cancel(&self, session_id: &str) -> Result<String, JsValue> {
        let session_id = parse::<SessionId>(session_id)?;
        self.command(
            Some(session_id.clone()),
            ClientCommand::Cancel(CancelTarget::Session(session_id)),
        )
    }

    pub fn retry(&self, session_id: &str) -> Result<String, JsValue> {
        let session_id = parse::<SessionId>(session_id)?;
        let text = self
            .last_prompt
            .clone()
            .ok_or_else(|| JsValue::from_str("There is no safely repeatable message."))?;
        self.command(
            Some(session_id.clone()),
            ClientCommand::SubmitPrompt(SubmitPrompt {
                session_id,
                text,
                artifacts: Vec::new(),
                delivery: DeliveryPolicy::Immediate,
                reply_route: None,
            }),
        )
    }

    pub fn branch(&self, session_id: &str) -> Result<String, JsValue> {
        let session_id = parse::<SessionId>(session_id)?;
        let parent_entry_id = self
            .reducer
            .as_ref()
            .and_then(|reducer| {
                reducer
                    .snapshot()
                    .messages
                    .iter()
                    .rev()
                    .find_map(|item| item.final_id.clone())
            })
            .ok_or_else(|| JsValue::from_str("A completed reply is required before branching."))?;
        self.command(
            Some(session_id.clone()),
            ClientCommand::BranchSession(BranchRequest {
                session_id,
                parent_entry_id: parent_entry_id.0,
                label: None,
            }),
        )
    }

    pub fn resume(&self, session_id: &str) -> Result<String, JsValue> {
        let session_id = parse::<SessionId>(session_id)?;
        self.command(
            Some(session_id.clone()),
            ClientCommand::ResumeSession { session_id },
        )
    }

    pub fn select_model(
        &self,
        session_id: &str,
        provider: String,
        model: String,
    ) -> Result<String, JsValue> {
        let session_id = parse::<SessionId>(session_id)?;
        self.command(
            Some(session_id.clone()),
            ClientCommand::SelectModel(ModelSelection {
                session_id,
                provider: nonempty(provider, "provider")?,
                model: nonempty(model, "model")?,
            }),
        )
    }

    pub fn resolve_confirmation(
        &self,
        session_id: &str,
        confirmation_id: &str,
        allow: bool,
    ) -> Result<String, JsValue> {
        let session_id = parse::<SessionId>(session_id)?;
        self.command(
            Some(session_id),
            ClientCommand::ResolveConfirmation(ConfirmationResolution {
                confirmation_id: EntityId::from_str(confirmation_id).map_err(js_error)?,
                decision: if allow {
                    ConfirmationDecision::AllowOnce
                } else {
                    ConfirmationDecision::Deny
                },
            }),
        )
    }

    pub fn create_goal(&self, session_id: &str, objective: String) -> Result<String, JsValue> {
        let session_id = parse::<SessionId>(session_id)?;
        self.command(
            Some(session_id.clone()),
            ClientCommand::CreateGoal(CreateGoal {
                session_id,
                objective: nonempty(objective, "goal")?,
                limits: no_limits(),
            }),
        )
    }

    pub fn create_child(&self, session_id: &str, objective: String) -> Result<String, JsValue> {
        let session_id = parse::<SessionId>(session_id)?;
        self.command(
            Some(session_id.clone()),
            ClientCommand::CreateChild(CreateChild {
                parent_session_id: session_id,
                objective: nonempty(objective, "delegated objective")?,
                workspace_mode: ChildWorkspaceMode::IsolatedCopy,
                limits: no_limits(),
            }),
        )
    }

    pub fn query_memory(&self, profile_id: &str, query: String) -> Result<String, JsValue> {
        self.command(
            None,
            ClientCommand::QueryMemory(MemoryQuery {
                profile_id: parse(profile_id)?,
                query: nonempty(query, "search")?,
                limit: 20,
            }),
        )
    }

    pub fn create_schedule(
        &self,
        profile_id: &str,
        session_id: Option<String>,
        prompt: String,
        interval_seconds: u64,
    ) -> Result<String, JsValue> {
        if interval_seconds == 0 {
            return Err(JsValue::from_str(
                "Schedule interval must be greater than zero.",
            ));
        }
        let session_id = session_id
            .map(|value| SessionId::from_str(&value).map_err(js_error))
            .transpose()?;
        self.command(
            session_id.clone(),
            ClientCommand::CreateSchedule(CreateSchedule {
                profile_id: parse(profile_id)?,
                session_id,
                expression: ScheduleExpression::IntervalSeconds(interval_seconds),
                time_zone: "UTC".into(),
                prompt: nonempty(prompt, "scheduled request")?,
                reply_route: None,
            }),
        )
    }

    pub fn export_session(&self, session_id: &str) -> Result<String, JsValue> {
        let session_id = parse::<SessionId>(session_id)?;
        self.command(
            Some(session_id.clone()),
            ClientCommand::Export(ExportRequest {
                session_id,
                format: ExportFormat::PortableBundle,
                include_artifacts: true,
            }),
        )
    }

    pub fn set_background(&self, profile_id: &str, mode: &str) -> Result<String, JsValue> {
        let mode = match mode {
            "disabled" => BackgroundMode::Disabled,
            "suggest" => BackgroundMode::Suggest,
            "confirm_selected" => BackgroundMode::ConfirmSelected,
            "bounded" => BackgroundMode::Bounded,
            _ => return Err(JsValue::from_str("Background mode is invalid.")),
        };
        self.command(
            None,
            ClientCommand::SetBackgroundControl(BackgroundControl {
                profile_id: parse(profile_id)?,
                mode,
                pause_until: None,
            }),
        )
    }
}

impl Default for BrowserProjection {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserProjection {
    fn reduce(&mut self, message: WireMessage) -> Result<(), JsValue> {
        match message {
            WireMessage::CommandResult(result) => {
                let pending_prompt = self.pending_prompts.remove(&result.command_id);
                match result.result {
                    CommandResult::Accepted { .. } => {
                        if let Some(prompt) = pending_prompt {
                            self.last_prompt = Some(prompt);
                        }
                        self.last_command = Some(BrowserCommandReceipt {
                            state: BrowserCommandState::Accepted,
                            message: "Keith received the request.".into(),
                        });
                    }
                    CommandResult::Rejected(error) => {
                        self.last_command = Some(BrowserCommandReceipt {
                            state: BrowserCommandState::Rejected,
                            message: error.error.message,
                        });
                    }
                    CommandResult::Data(payload) => {
                        if let Some(prompt) = pending_prompt {
                            self.last_prompt = Some(prompt);
                        }
                        match *payload {
                            ResponsePayload::Snapshot(snapshot) => {
                                self.install_snapshot(*snapshot)?;
                            }
                            ResponsePayload::Sessions(sessions) => {
                                self.sessions = sessions;
                            }
                            _ => {}
                        }
                        self.last_command = Some(BrowserCommandReceipt {
                            state: BrowserCommandState::Updated,
                            message: "Keith confirmed the update.".into(),
                        });
                    }
                }
            }
            WireMessage::Snapshot(frame) => self.install_snapshot(*frame.snapshot)?,
            message @ (WireMessage::Event(_) | WireMessage::Terminal(_)) => {
                let Some(event) = message.into_event() else {
                    return Ok(());
                };
                let Some(reducer) = &mut self.reducer else {
                    self.snapshot_required = true;
                    return Ok(());
                };
                match reducer.apply_event(&event) {
                    Ok(ReductionOutcome::Gap) | Err(_) => self.snapshot_required = true,
                    Ok(_) => self.snapshot_required = false,
                }
            }
            WireMessage::ClientHello(_) | WireMessage::ServerHello(_) | WireMessage::Command(_) => {
            }
        }
        Ok(())
    }

    fn install_snapshot(
        &mut self,
        snapshot: keith_protocol::SessionSnapshot,
    ) -> Result<(), JsValue> {
        let replace = self.reducer.as_ref().is_none_or(|reducer| {
            reducer.snapshot().session.session_id != snapshot.session.session_id
                || reducer.snapshot().session.root_tree_id != snapshot.session.root_tree_id
        });
        if replace {
            self.reducer =
                Some(ProjectionReducer::new(snapshot, virtualization()).map_err(js_error)?);
        } else if let Some(reducer) = &mut self.reducer {
            reducer.apply_snapshot(snapshot).map_err(js_error)?;
        }
        if let Some(snapshot) = self.reducer.as_ref().map(ProjectionReducer::snapshot) {
            upsert_session(&mut self.sessions, snapshot.session.clone());
        }
        self.snapshot_required = false;
        Ok(())
    }

    fn command(
        &self,
        session_id: Option<SessionId>,
        command: ClientCommand,
    ) -> Result<String, JsValue> {
        encode_command(&self.command_envelope(session_id, command))
    }

    fn command_envelope(
        &self,
        session_id: Option<SessionId>,
        command: ClientCommand,
    ) -> CommandEnvelope {
        CommandEnvelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            command_id: CommandId::new(),
            client_id: self.client_id.clone(),
            sent_at: UtcTimestamp::now().unwrap_or(UtcTimestamp::UNIX_EPOCH),
            session_id,
            command,
        }
    }
}

fn encode_command(envelope: &CommandEnvelope) -> Result<String, JsValue> {
    serde_json::to_string(envelope).map_err(js_error)
}

fn upsert_session(
    sessions: &mut Vec<keith_protocol::SessionSummary>,
    session: keith_protocol::SessionSummary,
) {
    if let Some(existing) = sessions
        .iter_mut()
        .find(|candidate| candidate.session_id == session.session_id)
    {
        *existing = session;
    } else {
        sessions.insert(0, session);
    }
}

fn virtualization() -> VirtualizationConfig {
    VirtualizationConfig::new(5_000, 240, 20)
        .expect("the browser virtualization constants are valid")
}

fn no_limits() -> GoalLimits {
    GoalLimits {
        max_turns: None,
        max_tokens: None,
        deadline: None,
    }
}

fn parse<T>(value: &str) -> Result<T, JsValue>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    T::from_str(value).map_err(js_error)
}

fn nonempty(value: String, label: &str) -> Result<String, JsValue> {
    if value.trim().is_empty() {
        Err(JsValue::from_str(&format!("{label} cannot be empty.")))
    } else {
        Ok(value)
    }
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[cfg(test)]
mod tests {
    use keith_agent_types::{CommonError, ErrorCode};
    use keith_protocol::{CommandError, CommandResultEnvelope};

    use super::*;

    fn result(command_id: CommandId, result: CommandResult) -> WireMessage {
        WireMessage::CommandResult(CommandResultEnvelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            command_id,
            completed_at: UtcTimestamp::UNIX_EPOCH,
            result,
        })
    }

    fn envelope(encoded: &str) -> CommandEnvelope {
        serde_json::from_str(encoded).unwrap()
    }

    #[test]
    fn only_confirmed_prompts_become_safely_retryable() {
        let mut projection = BrowserProjection::new();
        let session = SessionId::new();
        let first = envelope(
            &projection
                .submit_prompt(&session.to_string(), "confirmed request".into())
                .unwrap(),
        );
        projection
            .reduce(result(
                first.command_id,
                CommandResult::Accepted { action_id: None },
            ))
            .unwrap();

        let uncertain = envelope(
            &projection
                .submit_prompt(&session.to_string(), "unknown outcome".into())
                .unwrap(),
        );
        let retry = envelope(&projection.retry(&session.to_string()).unwrap());
        assert!(matches!(
            retry.command,
            ClientCommand::SubmitPrompt(SubmitPrompt { text, .. }) if text == "confirmed request"
        ));

        let uncertain_id = uncertain.command_id.to_string();
        projection
            .reduce(result(
                uncertain.command_id,
                CommandResult::Rejected(CommandError {
                    error: CommonError::new(
                        ErrorCode::InvalidInput,
                        "Request was not accepted",
                        false,
                    ),
                    unsupported_feature: None,
                }),
            ))
            .unwrap();
        let view = projection.current_view().unwrap();
        let parsed = serde_json::from_str::<serde_json::Value>(&view).unwrap();
        assert_eq!(parsed["last_command"]["state"], "rejected");
        assert_eq!(
            parsed["last_command"]["message"],
            "Request was not accepted"
        );
        assert!(!view.contains(&uncertain_id));
    }
}
