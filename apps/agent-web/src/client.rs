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
    snapshot_required: bool,
}

/// The browser's only protocol authority. Solid renders the serialized projection and sends the
/// typed envelopes built here; it never reduces daemon events or constructs wire commands.
#[wasm_bindgen]
pub struct BrowserProjection {
    client_id: ClientId,
    reducer: Option<ProjectionReducer>,
    snapshot_required: bool,
    last_prompt: Option<String>,
}

#[wasm_bindgen]
impl BrowserProjection {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            client_id: ClientId::new(),
            reducer: None,
            snapshot_required: false,
            last_prompt: None,
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
        self.last_prompt = Some(text.clone());
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
                if let CommandResult::Data(payload) = result.result
                    && let ResponsePayload::Snapshot(snapshot) = *payload
                {
                    self.install_snapshot(*snapshot)?;
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
        self.snapshot_required = false;
        Ok(())
    }

    fn command(
        &self,
        session_id: Option<SessionId>,
        command: ClientCommand,
    ) -> Result<String, JsValue> {
        serde_json::to_string(&CommandEnvelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            command_id: CommandId::new(),
            client_id: self.client_id.clone(),
            sent_at: UtcTimestamp::now().unwrap_or(UtcTimestamp::UNIX_EPOCH),
            session_id,
            command,
        })
        .map_err(js_error)
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
