#![forbid(unsafe_code)]

use keith_agent_types::{ClientId, Generation, ProfileId, RootTreeId, SessionId, UtcTimestamp};
use keith_protocol::{
    ClientCommand, CommandResult, CreateSession, ModelSelection, ProfileSummary, SessionSnapshot,
    SessionState, SubmitPrompt,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSession {
    pub session_id: SessionId,
    pub root_tree_id: RootTreeId,
    pub profile_id: ProfileId,
    pub title: Option<String>,
    pub archived: bool,
    pub created_at: UtcTimestamp,
}

#[allow(clippy::missing_errors_doc)]
pub trait CommandRuntime: Send + Sync {
    fn profiles(&self) -> Result<Vec<ProfileSummary>, String>;
    fn sessions(&self) -> Result<Vec<RuntimeSession>, String>;
    fn create_default_session(&self, title: Option<String>) -> Result<RuntimeSession, String>;
    fn create_session(&self, request: &CreateSession) -> Result<RuntimeSession, String>;
    fn select_model(&self, selection: &ModelSelection) -> Result<(), String>;
    fn run_prompt(
        &self,
        prompt: &SubmitPrompt,
        generation: Generation,
    ) -> Result<SessionSnapshot, String>;
    fn snapshot(
        &self,
        session_id: &SessionId,
        generation: Generation,
        state: SessionState,
    ) -> Result<SessionSnapshot, String>;
    fn execute_feature(
        &self,
        client_id: &ClientId,
        scope_session_id: Option<&SessionId>,
        command: &ClientCommand,
        generation: Generation,
    ) -> Result<CommandResult, String>;
    fn maintain(&self) -> Result<(), String>;
}
