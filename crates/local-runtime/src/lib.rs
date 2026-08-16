#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use keith_action_store::{
    ActionInboxConfig, ActionLimits, ActionPayload, ActionPriority, ActionSource,
    DeliveryPolicy as ActionDeliveryPolicy, PersistentActionInbox, PumpContext,
    ReplyRoute as ActionReplyRoute, SessionAction,
};
use keith_agent_loop::{AgentLoop, AgentLoopConfig, ConservativeCompactor, NoSteering};
use keith_agent_types::{
    ActionId, CURRENT_SCHEMA_VERSION, ClientId, EntityId, EntryId, Generation, MessageId,
    ProfileId, Revision, RootTreeId, SessionId, TimeZoneName, UtcTimestamp, WorkerId, WorkspaceId,
};
use keith_artifacts::{
    ArtifactLimits, ArtifactReference, ArtifactScope, ArtifactService, ArtifactSource,
    DisplayMetadata, NewArtifact, RetentionPolicy,
};
use keith_configuration::{
    AgentProfile, AutonomyMode, ModelRoute as ProfileModelRoute,
    ModelSelection as ProfileModelSelection, NotificationSettings, ProfileAutonomy,
    RefinementSettings, ThinkingLevel, ToolPermission,
};
use keith_credentials::{EncryptedCredentialStore, MasterKey, ProviderCredentialResolver};
use keith_goals::{
    GoalEdit, GoalLimits as RuntimeGoalLimits, GoalState as RuntimeGoalState, LinkUpdate,
    PersistentGoalService,
};
use keith_model_registry::{
    CredentialResolver, ModelRegistry, ModelRoute, ModelSelection, RegistryError,
};
use keith_profile::{ProfileError, ProfileRegistry, ProfileResources, RegisteredProfile};
use keith_protocol::{
    ActionProjection, BackgroundMode, BackgroundProjection, BranchRequest, CancelTarget,
    ChildProjection, ClientCommand, CommandResult, CreateChild, CreateGoal, CreateSchedule,
    ExportFormat, ExportProjection, ExportRequest, GoalProjection, GoalState, MemoryQuery,
    MemoryResult, MessageProjection, MessageRole as ProjectionMessageRole, PresenceProjection,
    PresenceState, ProfileSummary, ResponsePayload, ScheduleExpression, ScheduleProjection,
    SelectBranch, SessionSnapshot, SessionState, SessionSummary, SteerAction, ToolProjection,
    UpdateGoal, UpdateSchedule, UsageProjection,
};
use keith_provider_adapters::{
    AmazonBedrockProvider, AnthropicProvider, OpenAiProvider, OpenAiResponsesProvider,
    ProviderHttpConfig,
};
use keith_provider_catalog::{
    BUILTIN_PROVIDERS, ProviderAuthentication, ProviderTransport, provider as provider_spec,
};
use keith_provider_core::{
    CancellationToken, ContentBlock as ProviderContentBlock, Message as ProviderMessage,
    MessageRole as ProviderMessageRole, ModelRequest, ProviderError,
};
use keith_retrieval::{RankWeights, RetrievalLimits, RetrievalService};
use keith_runtime_api::{CommandRuntime, RuntimeSession};
use keith_scheduler::{
    JobState, JobUpdate, MissedRunPolicy, NewScheduledJob, ScheduleSpec, Scheduler, SchedulerConfig,
};
use keith_session_store::{
    ContentBlock as StoredContentBlock, MessageRole as StoredMessageRole, NewSession, SessionEntry,
    SessionEntryPayload, SessionKind, SessionManifest, SessionStore, SessionStoreError,
    StoredMessage, WriterIdentity,
};
use keith_state_store::{EmbeddedStore, FileBackupHook, StoreError};
use keith_state_store_core::{
    AtomicStateRepository, Collection, RecordMutation, VersionedRecord, WritePrecondition,
};
use keith_subagents::{
    ChildCancellation, ChildCoordinator, ChildLimits, ChildMessageKind, ChildMessageSender,
    ChildRetention, ChildSpec, ChildStatus, ChildWorkspaceMode, ParentAuthority,
};
use keith_tool_core::{
    ConfirmationMode, ExecutionDecision, ExecutionRules, ManagedTool, ProgressSink, Readiness,
    Repeatability, ToolBehavior, ToolDefinition, ToolExecutionError, ToolInvocation, ToolManager,
    ToolManagerConfig, ToolManagerError,
};
use keith_tool_runner_core::{
    ExpectedPreimage, IsolationRequest, ProcessLimits, RestrictedProcessRunner, RunRequest,
    WorkspaceFs, WorkspaceLimits,
};
use thiserror::Error;

const DEFAULT_CREDENTIAL_REFERENCE: &str = "default";
const DEFAULT_OPENAI_MODEL: &str = "gpt-4.1-mini";

type GoalService = PersistentGoalService<EmbeddedStore, EmbeddedStore>;
type ChildService = ChildCoordinator<EmbeddedStore>;
type LocalScheduler = Scheduler<EmbeddedStore, PersistentActionInbox<EmbeddedStore>>;

pub struct LocalRuntimeConfig {
    pub data_root: PathBuf,
    pub credential_root: PathBuf,
    pub credential_key: MasterKey,
    pub workspace_root: PathBuf,
    pub openai_base_url: String,
    pub anthropic_base_url: String,
    pub provider_base_urls: BTreeMap<String, String>,
}

pub struct LocalRuntime {
    profiles: ProfileRegistry<EmbeddedStore>,
    sessions: SessionStore,
    actions: PersistentActionInbox<EmbeddedStore>,
    goals: GoalService,
    children: ChildService,
    scheduler: LocalScheduler,
    scheduler_claimant: EntityId,
    retrieval: RetrievalService,
    background: EmbeddedStore,
    credentials: EncryptedCredentialStore,
    models: ModelRegistry,
    artifacts: Arc<ArtifactService>,
    available_providers: BTreeSet<String>,
    active_cancellations: Mutex<BTreeMap<SessionId, CancellationToken>>,
}

#[derive(Debug, Error)]
pub enum LocalRuntimeError {
    #[error("runtime I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("runtime clock failed: {0}")]
    Clock(#[from] keith_agent_types::TimestampError),
    #[error("runtime state failed: {0}")]
    State(#[from] StoreError),
    #[error("profile operation failed: {0}")]
    Profile(#[from] ProfileError),
    #[error("session operation failed: {0}")]
    Session(#[from] SessionStoreError),
    #[error("credential operation failed: {0}")]
    Credential(#[from] keith_credentials::CredentialError),
    #[error("model operation failed: {0}")]
    Model(#[from] RegistryError),
    #[error("provider setup failed: {0}")]
    Provider(#[from] ProviderError),
    #[error("artifact operation failed: {0}")]
    Artifact(#[from] keith_artifacts::ArtifactError),
    #[error("action operation failed: {0}")]
    Action(#[from] keith_action_store::ActionStoreError),
    #[error("goal operation failed: {0}")]
    Goal(#[from] keith_goals::GoalError),
    #[error("child operation failed: {0}")]
    Child(#[from] keith_subagents::ChildError),
    #[error("schedule operation failed: {0}")]
    Schedule(#[from] keith_scheduler::SchedulerError),
    #[error("retrieval operation failed: {0}")]
    Retrieval(#[from] keith_retrieval::RetrievalError),
    #[error("runtime JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("tool setup failed: {0}")]
    Tool(#[from] ToolManagerError),
    #[error("agent turn failed: {0}")]
    Agent(#[from] keith_agent_loop::AgentLoopError),
    #[error("profile {0} was not found")]
    MissingProfile(ProfileId),
    #[error("session {0} does not belong to profile {1}")]
    SessionProfileMismatch(SessionId, ProfileId),
    #[error("workspace identity does not belong to the profile")]
    WorkspaceMismatch,
    #[error("provider {0} is not supported by this installation")]
    UnsupportedProvider(String),
    #[error("runtime request is invalid: {0}")]
    Invalid(String),
    #[error("runtime state lock was poisoned")]
    LockPoisoned,
    #[error("runtime command is not implemented by the local composition")]
    UnsupportedCommand,
}

#[allow(clippy::missing_errors_doc)]
impl LocalRuntime {
    #[allow(clippy::too_many_lines)]
    pub fn open(config: LocalRuntimeConfig) -> Result<Self, LocalRuntimeError> {
        fs::create_dir_all(&config.data_root)?;
        let state_path = config.data_root.join("state.sqlite");
        let state = EmbeddedStore::open(&state_path, Some(&FileBackupHook))?;
        let profiles = ProfileRegistry::new(state);
        let sessions = SessionStore::open(config.data_root.join("agent-sessions"))?;
        let credentials =
            EncryptedCredentialStore::open(config.credential_root, config.credential_key)?;
        let models = ModelRegistry::new();
        models.register_provider(Arc::new(OpenAiProvider::new(ProviderHttpConfig::new(
            config.openai_base_url,
        )?)?))?;
        models.register_provider(Arc::new(AnthropicProvider::new(ProviderHttpConfig::new(
            config.anthropic_base_url,
        )?)?))?;
        let mut available_providers = BTreeSet::from(["openai".into(), "anthropic".into()]);
        for provider in BUILTIN_PROVIDERS {
            if matches!(provider.id, "openai" | "anthropic") {
                continue;
            }
            let base_url = config
                .provider_base_urls
                .get(provider.id)
                .map(String::as_str)
                .or(provider.default_base_url);
            let Some(base_url) = base_url else {
                continue;
            };
            let configuration = ProviderHttpConfig::new(base_url)?;
            match provider.transport {
                ProviderTransport::OpenAiChat | ProviderTransport::GoogleGenerativeAi => {
                    if provider.id == "openai-codex" {
                        models.register_provider(Arc::new(OpenAiResponsesProvider::codex(
                            configuration,
                            provider.default_model,
                        )?))?;
                    } else {
                        models.register_provider(Arc::new(OpenAiProvider::compatible(
                            provider.id,
                            configuration,
                            provider.default_model,
                            false,
                        )?))?;
                    }
                }
                ProviderTransport::AnthropicMessages => {
                    let mut adapter = AnthropicProvider::compatible(
                        provider.id,
                        configuration,
                        provider.default_model,
                        provider.authentication != ProviderAuthentication::ApiKeyHeader,
                    )?;
                    if provider.authentication == ProviderAuthentication::CloudflareApiToken {
                        adapter = adapter.with_credential_header("cf-aig-authorization", true)?;
                    }
                    if provider.id == "github-copilot" {
                        for (name, value) in [
                            ("user-agent", "GitHubCopilotChat/0.35.0"),
                            ("editor-version", "vscode/1.107.0"),
                            ("editor-plugin-version", "copilot-chat/0.35.0"),
                            ("copilot-integration-id", "vscode-chat"),
                        ] {
                            adapter = adapter.with_default_header(name, value)?;
                        }
                    }
                    models.register_provider(Arc::new(adapter))?;
                }
                ProviderTransport::AzureOpenAi => {
                    models.register_provider(Arc::new(
                        OpenAiProvider::compatible(
                            provider.id,
                            configuration,
                            provider.default_model,
                            false,
                        )?
                        .with_api_key_header(),
                    ))?;
                }
                ProviderTransport::AmazonBedrock => {
                    models.register_provider(Arc::new(AmazonBedrockProvider::new(
                        configuration,
                        provider.default_model,
                    )?))?;
                }
            }
            available_providers.insert(provider.id.into());
        }
        let artifacts = Arc::new(ArtifactService::open(
            config.data_root.join("artifacts"),
            ArtifactLimits::default(),
        )?);
        let actions = PersistentActionInbox::new(
            EmbeddedStore::open(&state_path, Some(&FileBackupHook))?,
            ActionInboxConfig::default(),
        )?;
        let goal_actions = PersistentActionInbox::new(
            EmbeddedStore::open(&state_path, Some(&FileBackupHook))?,
            ActionInboxConfig::default(),
        )?;
        let goals = PersistentGoalService::new(
            EmbeddedStore::open(&state_path, Some(&FileBackupHook))?,
            goal_actions,
        );
        let children = ChildCoordinator::open(
            config.data_root.join("children"),
            EmbeddedStore::open(&state_path, Some(&FileBackupHook))?,
            Arc::clone(&artifacts),
        )?;
        let schedule_repository =
            Arc::new(EmbeddedStore::open(&state_path, Some(&FileBackupHook))?);
        let schedule_sink = Arc::new(PersistentActionInbox::new(
            EmbeddedStore::open(&state_path, Some(&FileBackupHook))?,
            ActionInboxConfig::default(),
        )?);
        let scheduler = Scheduler::new(
            schedule_repository,
            schedule_sink,
            SchedulerConfig::default(),
        )?;
        let retrieval = RetrievalService::open(
            config.data_root.join("retrieval"),
            RetrievalLimits::default(),
            RankWeights::default(),
            None,
        )?;
        let background = EmbeddedStore::open(&state_path, Some(&FileBackupHook))?;
        let runtime = Self {
            profiles,
            sessions,
            actions,
            goals,
            children,
            scheduler,
            scheduler_claimant: EntityId::new(),
            retrieval,
            background,
            credentials,
            models,
            artifacts,
            available_providers,
            active_cancellations: Mutex::new(BTreeMap::new()),
        };
        runtime.bootstrap_default_profile(&config.workspace_root)?;
        runtime.register_child_roots()?;
        runtime.children.recover_active()?;
        Ok(runtime)
    }

    pub fn profiles(&self) -> Result<Vec<ProfileSummary>, LocalRuntimeError> {
        Ok(self
            .profiles
            .list()?
            .into_iter()
            .map(|profile| ProfileSummary {
                id: profile.profile.id,
                workspace_id: profile.profile.workspace_id,
                display_name: profile.profile.display_name,
                enabled: profile.enabled,
            })
            .collect())
    }

    pub fn registered_profiles(&self) -> Result<Vec<RegisteredProfile>, LocalRuntimeError> {
        self.profiles.list().map_err(LocalRuntimeError::from)
    }

    pub fn sessions(&self) -> Result<Vec<SessionManifest>, LocalRuntimeError> {
        self.sessions.discover().map_err(LocalRuntimeError::from)
    }

    pub fn create_session(
        &self,
        profile_id: &ProfileId,
        workspace_id: &WorkspaceId,
        title: Option<String>,
    ) -> Result<SessionManifest, LocalRuntimeError> {
        let profile = self.profile(profile_id)?;
        if &profile.profile.workspace_id != workspace_id {
            return Err(LocalRuntimeError::WorkspaceMismatch);
        }
        let now = UtcTimestamp::now()?;
        let session = self.sessions.create(NewSession {
            kind: SessionKind::Root,
            session_id: SessionId::new(),
            root_tree_id: RootTreeId::new(),
            parent_session_id: None,
            profile_id: profile_id.clone(),
            workspace_id: workspace_id.clone(),
            created_at: now,
            label: title,
            profile_snapshot: None,
        })?;
        self.children.register_root(ParentAuthority {
            session_id: session.session_id.clone(),
            root_tree_id: session.root_tree_id.clone(),
            profile_id: profile.profile.id.clone(),
            workspace_id: profile.profile.workspace_id.clone(),
            workspace_root: profile.resources.workspace_root.clone(),
            allowed_tools: allowed_tools(&profile),
        })?;
        Ok(session)
    }

    pub fn select_model(
        &self,
        session_id: &SessionId,
        provider: String,
        model: String,
    ) -> Result<(), LocalRuntimeError> {
        self.ensure_supported_provider(&provider)?;
        let manifest = self.sessions.manifest(session_id)?;
        let mut profile = self.profile(&manifest.profile_id)?;
        profile.profile.model_route.provider = provider;
        profile.profile.model_route.model = model;
        profile.profile.model_route.fallbacks.clear();
        let revision = profile.revision;
        profile.updated_at = UtcTimestamp::now()?;
        self.profiles.update(profile, revision)?;
        Ok(())
    }

    pub fn run_prompt(
        &self,
        session_id: &SessionId,
        text: &str,
        generation: Generation,
    ) -> Result<SessionSnapshot, LocalRuntimeError> {
        let manifest = self.sessions.manifest(session_id)?;
        let profile = self.profile(&manifest.profile_id)?;
        self.prepare_model_route(&profile)?;
        let tools = Self::tool_manager(&profile)?;
        let definitions = tools
            .discover()?
            .available
            .into_iter()
            .map(|definition| definition.model_definition())
            .collect();
        let identity = WriterIdentity {
            worker_id: WorkerId::new(),
            owner_instance: EntityId::new(),
            generation,
            acquired_at: UtcTimestamp::now()?,
        };
        let mut writer = self.sessions.acquire_writer(session_id, identity)?;
        let parent = writer.manifest().active_leaf.clone();
        writer.append(
            parent,
            UtcTimestamp::now()?,
            SessionEntryPayload::UserMessage {
                message: StoredMessage {
                    role: StoredMessageRole::User,
                    content: vec![StoredContentBlock::Text {
                        text: text.to_owned(),
                    }],
                    provider_metadata: BTreeMap::new(),
                },
            },
        )?;
        let request = Self::model_request(&profile, &writer.active_ancestry()?, definitions)?;
        let spill = self.artifacts.scoped_spill(
            ArtifactScope {
                root_tree_id: manifest.root_tree_id.clone(),
                session_id: session_id.clone(),
                profile_id: manifest.profile_id.clone(),
            },
            ArtifactSource::Tool,
            "auto",
            RetentionPolicy::Retain,
        );
        let resolver = ProviderCredentialResolver::new(&self.credentials);
        let cancellation = CancellationToken::default();
        {
            let mut active = self
                .active_cancellations
                .lock()
                .map_err(|_| LocalRuntimeError::LockPoisoned)?;
            if active
                .insert(session_id.clone(), cancellation.clone())
                .is_some()
            {
                return Err(LocalRuntimeError::Invalid(
                    "a turn is already active for this session".into(),
                ));
            }
        }
        let result = AgentLoop::new(
            &self.models,
            &manifest.profile_id,
            &resolver,
            &tools,
            &spill,
            &ConservativeCompactor,
            &NoSteering,
            &mut writer,
            AgentLoopConfig::default(),
        )
        .run(request, &cancellation);
        self.active_cancellations
            .lock()
            .map_err(|_| LocalRuntimeError::LockPoisoned)?
            .remove(session_id);
        result?;
        drop(writer);
        self.snapshot(session_id, generation, SessionState::Ready)
    }

    #[allow(clippy::too_many_lines)]
    pub fn snapshot(
        &self,
        session_id: &SessionId,
        generation: Generation,
        state: SessionState,
    ) -> Result<SessionSnapshot, LocalRuntimeError> {
        let manifest = self.sessions.manifest(session_id)?;
        let index = self.sessions.load_index(session_id)?;
        let entries = manifest
            .active_leaf
            .as_ref()
            .map(|leaf| index.ancestry(leaf))
            .transpose()?
            .unwrap_or_default();
        let mut messages = Vec::new();
        let mut tools = Vec::new();
        let mut usage = UsageProjection::default();
        for entry in &entries {
            match &entry.payload {
                SessionEntryPayload::UserMessage { message } => messages.push(message_projection(
                    entry,
                    ProjectionMessageRole::User,
                    &message.content,
                )),
                SessionEntryPayload::AssistantMessage { message } => messages.push(
                    message_projection(entry, ProjectionMessageRole::Assistant, &message.content),
                ),
                SessionEntryPayload::ToolCall { call_id, .. } => tools.push(ToolProjection {
                    tool_call_id: call_id.clone(),
                    state: "running".into(),
                    terminal: false,
                }),
                SessionEntryPayload::ToolResult {
                    call_id,
                    content,
                    is_error,
                } => {
                    messages.push(message_projection(
                        entry,
                        ProjectionMessageRole::Tool,
                        content,
                    ));
                    if let Some(tool) = tools.iter_mut().find(|tool| tool.tool_call_id == *call_id)
                    {
                        tool.state = if *is_error { "failed" } else { "succeeded" }.into();
                        tool.terminal = true;
                    }
                }
                SessionEntryPayload::Usage {
                    input_tokens,
                    output_tokens,
                    ..
                } => {
                    usage.input_tokens = usage.input_tokens.saturating_add(*input_tokens);
                    usage.output_tokens = usage.output_tokens.saturating_add(*output_tokens);
                }
                _ => {}
            }
        }
        let updated_at = entries
            .last()
            .map_or(manifest.created_at, |entry| entry.timestamp);
        let actions = self
            .actions
            .list_session(session_id)?
            .into_iter()
            .map(|record| ActionProjection {
                action_id: record.action.id,
                source: action_source_name(&record.action.source).into(),
                state: action_state_name(record.state).into(),
                created_at: record.action.created_at,
            })
            .collect::<Vec<_>>();
        let active_action = actions
            .iter()
            .find(|action| action.state == "running")
            .cloned();
        let goals = self
            .goals
            .list_session(session_id)?
            .iter()
            .map(goal_projection)
            .collect::<Vec<_>>();
        let children = self
            .children
            .list_parent(session_id)?
            .iter()
            .map(child_projection)
            .collect::<Vec<_>>();
        let schedules = self
            .scheduler
            .projections_for_session(session_id)?
            .iter()
            .map(schedule_projection)
            .collect::<Vec<_>>();
        Ok(SessionSnapshot {
            session: SessionSummary {
                session_id: manifest.session_id.clone(),
                root_tree_id: manifest.root_tree_id.clone(),
                profile_id: manifest.profile_id.clone(),
                title: manifest.label.clone(),
                state,
                updated_at,
            },
            generation,
            through_sequence: keith_agent_types::Sequence::ZERO,
            active_action,
            actions,
            messages,
            goals,
            plans: Vec::new(),
            children,
            kernels: Vec::new(),
            commitments: Vec::new(),
            schedules,
            tools,
            confirmations: Vec::new(),
            waits: Vec::new(),
            deliveries: Vec::new(),
            memory_changes: Vec::new(),
            usage,
            presence: PresenceProjection {
                session_id: manifest.session_id,
                goal_id: None,
                state: PresenceState::Available,
                updated_at,
                next_wake: None,
                safe_error: None,
            },
            revision: Revision::new(u64::try_from(entries.len()).unwrap_or(u64::MAX)),
        })
    }

    fn branch_session(
        &self,
        request: &BranchRequest,
        generation: Generation,
    ) -> Result<SessionSnapshot, LocalRuntimeError> {
        let leaf = EntryId::from(request.parent_entry_id.clone());
        let mut writer = self.sessions.acquire_writer(
            &request.session_id,
            runtime_writer_identity(generation, UtcTimestamp::now()?),
        )?;
        writer.select_leaf(&leaf)?;
        if let Some(label) = &request.label {
            writer.label_branch(label.clone(), &leaf)?;
        }
        drop(writer);
        self.snapshot(&request.session_id, generation, SessionState::Ready)
    }

    fn select_branch(
        &self,
        request: &SelectBranch,
        generation: Generation,
    ) -> Result<SessionSnapshot, LocalRuntimeError> {
        let leaf = EntryId::from(request.leaf_entry_id.clone());
        let mut writer = self.sessions.acquire_writer(
            &request.session_id,
            runtime_writer_identity(generation, UtcTimestamp::now()?),
        )?;
        writer.select_leaf(&leaf)?;
        drop(writer);
        self.snapshot(&request.session_id, generation, SessionState::Ready)
    }

    fn create_goal(&self, request: &CreateGoal) -> Result<GoalProjection, LocalRuntimeError> {
        self.sessions.manifest(&request.session_id)?;
        let now = UtcTimestamp::now()?;
        let goal = self.goals.create(
            request.session_id.clone(),
            request.objective.clone(),
            goal_limits(&request.limits, now, RuntimeGoalLimits::default())?,
            now,
        )?;
        Ok(goal_projection(&goal))
    }

    fn update_goal(
        &self,
        scope_session_id: Option<&SessionId>,
        request: &UpdateGoal,
    ) -> Result<GoalProjection, LocalRuntimeError> {
        let now = UtcTimestamp::now()?;
        let current = self
            .goals
            .get(&request.goal_id)?
            .ok_or_else(|| LocalRuntimeError::Invalid("goal was not found".into()))?;
        ensure_session_scope(scope_session_id, &current.session_id)?;
        if request.objective.is_some() || request.limits.is_some() {
            self.goals.edit(
                &request.goal_id,
                GoalEdit {
                    objective: request.objective.clone(),
                    limits: request
                        .limits
                        .as_ref()
                        .map(|limits| goal_limits(limits, now, current.limits))
                        .transpose()?,
                    plan: LinkUpdate::Keep,
                    waiting_condition: LinkUpdate::Keep,
                },
                now,
            )?;
        }
        let goal = if let Some(state) = request.state {
            self.set_goal_state(&request.goal_id, state, now)?
        } else {
            self.goals
                .get(&request.goal_id)?
                .ok_or_else(|| LocalRuntimeError::Invalid("goal was not found".into()))?
        };
        Ok(goal_projection(&goal))
    }

    fn set_goal_state(
        &self,
        goal_id: &keith_agent_types::GoalId,
        state: GoalState,
        now: UtcTimestamp,
    ) -> Result<keith_goals::Goal, LocalRuntimeError> {
        let mut current = self
            .goals
            .get(goal_id)?
            .ok_or_else(|| LocalRuntimeError::Invalid("goal was not found".into()))?;
        let desired = runtime_goal_state(state);
        if current.state == desired {
            return Ok(current);
        }
        if desired == RuntimeGoalState::Running && current.state == RuntimeGoalState::Draft {
            current = self
                .goals
                .transition(goal_id, RuntimeGoalState::Ready, None, now)?;
        }
        match desired {
            RuntimeGoalState::Paused => self.goals.pause(goal_id, now).map_err(Into::into),
            RuntimeGoalState::Blocked => self
                .goals
                .block(goal_id, "Blocked by operator", now)
                .map_err(Into::into),
            RuntimeGoalState::Cancelled => self
                .goals
                .cancel(goal_id, "Cancelled by operator", now)
                .map_err(Into::into),
            RuntimeGoalState::Complete => self
                .goals
                .transition(goal_id, desired, Some("Completed by operator".into()), now)
                .map_err(Into::into),
            RuntimeGoalState::Failed => self
                .goals
                .transition(
                    goal_id,
                    desired,
                    Some("Marked failed by operator".into()),
                    now,
                )
                .map_err(Into::into),
            RuntimeGoalState::Running
                if matches!(
                    current.state,
                    RuntimeGoalState::Paused | RuntimeGoalState::Blocked
                ) =>
            {
                self.goals.resume(goal_id, now).map_err(Into::into)
            }
            RuntimeGoalState::Draft => Err(LocalRuntimeError::Invalid(
                "a goal cannot transition back to draft".into(),
            )),
            _ => self
                .goals
                .transition(goal_id, desired, None, now)
                .map_err(Into::into),
        }
    }

    fn create_child(&self, request: &CreateChild) -> Result<ChildProjection, LocalRuntimeError> {
        let parent = self.sessions.manifest(&request.parent_session_id)?;
        let profile = self.profile(&parent.profile_id)?;
        let child = self.children.create(
            ChildSpec {
                parent_session_id: request.parent_session_id.clone(),
                objective: request.objective.clone(),
                workspace_mode: child_workspace_mode(request.workspace_mode),
                requested_tools: allowed_tools(&profile),
                provider: profile.profile.model_route.provider.clone(),
                model: profile.profile.model_route.model.clone(),
                limits: child_limits(&profile, &request.limits),
                cancellation: ChildCancellation::Propagate,
                retention: ChildRetention::Retain,
            },
            UtcTimestamp::now()?,
        )?;
        let link_result = (|| -> Result<(), LocalRuntimeError> {
            let mut writer = self.sessions.acquire_writer(
                &request.parent_session_id,
                runtime_writer_identity(Generation::ZERO, UtcTimestamp::now()?),
            )?;
            let parent_entry = writer.manifest().active_leaf.clone();
            writer.append(
                parent_entry,
                UtcTimestamp::now()?,
                SessionEntryPayload::ChildLinked {
                    child_id: child.id.clone(),
                    child_session_id: child.session_id.clone(),
                },
            )?;
            Ok(())
        })();
        if let Err(error) = link_result {
            let _ = self.children.cancel(
                &child.id,
                "Parent session link could not be committed",
                UtcTimestamp::now()?,
            );
            return Err(error);
        }
        Ok(child_projection(&keith_subagents::ChildProjection::from(
            &child,
        )))
    }

    fn send_child_message(
        &self,
        scope_session_id: Option<&SessionId>,
        request: &keith_protocol::ChildMessageRequest,
    ) -> Result<ChildProjection, LocalRuntimeError> {
        let child = self.children.projection(&request.child_id)?;
        ensure_session_scope(scope_session_id, &child.parent_session_id)?;
        let now = UtcTimestamp::now()?;
        if !request.text.trim().is_empty() {
            self.children.send_message(
                &request.child_id,
                ChildMessageSender::Parent,
                ChildMessageKind::Text {
                    text: request.text.clone(),
                },
                now,
            )?;
        }
        if !request.artifact_ids.is_empty() {
            let parent = self.sessions.manifest(&child.parent_session_id)?;
            let references = request
                .artifact_ids
                .iter()
                .cloned()
                .map(|id| ArtifactReference {
                    id,
                    root_tree_id: parent.root_tree_id.clone(),
                    profile_id: parent.profile_id.clone(),
                })
                .collect();
            self.children.send_message(
                &request.child_id,
                ChildMessageSender::Parent,
                ChildMessageKind::Artifacts { references },
                now,
            )?;
        }
        self.children
            .projection(&request.child_id)
            .map(|projection| child_projection(&projection))
            .map_err(Into::into)
    }

    fn archive_child(
        &self,
        scope_session_id: Option<&SessionId>,
        child_id: &keith_agent_types::ChildId,
    ) -> Result<ChildProjection, LocalRuntimeError> {
        let now = UtcTimestamp::now()?;
        let current = self.children.projection(child_id)?;
        ensure_session_scope(scope_session_id, &current.parent_session_id)?;
        if !current.status.is_terminal() {
            self.children
                .cancel(child_id, "Cancelled before archival", now)?;
        }
        let child = self.children.archive(child_id, now)?;
        Ok(child_projection(&keith_subagents::ChildProjection::from(
            &child,
        )))
    }

    fn create_schedule(
        &self,
        request: &CreateSchedule,
    ) -> Result<ScheduleProjection, LocalRuntimeError> {
        let session_id = match &request.session_id {
            Some(session_id) => {
                let manifest = self.sessions.manifest(session_id)?;
                if manifest.profile_id != request.profile_id {
                    return Err(LocalRuntimeError::SessionProfileMismatch(
                        session_id.clone(),
                        request.profile_id.clone(),
                    ));
                }
                session_id.clone()
            }
            None => self
                .sessions()?
                .into_iter()
                .find(|session| session.profile_id == request.profile_id && !session.archived)
                .map(|session| session.session_id)
                .ok_or_else(|| {
                    LocalRuntimeError::Invalid(
                        "a schedule requires an existing session for its profile".into(),
                    )
                })?,
        };
        let now = UtcTimestamp::now()?;
        let job = self.scheduler.create(
            NewScheduledJob {
                profile_id: request.profile_id.clone(),
                session_id,
                schedule: schedule_spec(&request.expression, &request.time_zone, now)?,
                action: ActionPayload::Scheduled {
                    instruction: request.prompt.clone(),
                },
                limits: ActionLimits::default(),
                reply_route: request.reply_route.as_ref().map(action_reply_route),
                missed_run: MissedRunPolicy::RunOnce,
            },
            now,
        )?;
        Ok(schedule_projection_from_job(&job))
    }

    fn update_schedule(
        &self,
        scope_session_id: Option<&SessionId>,
        request: &UpdateSchedule,
    ) -> Result<ScheduleProjection, LocalRuntimeError> {
        let now = UtcTimestamp::now()?;
        ensure_session_scope(
            scope_session_id,
            &self.scheduler.session_id(&request.job_id)?,
        )?;
        let time_zone = self
            .scheduler
            .projections()?
            .into_iter()
            .find(|projection| projection.job_id == request.job_id)
            .and_then(|projection| match projection.schedule {
                ScheduleSpec::Calendar { time_zone, .. } => Some(time_zone),
                _ => None,
            })
            .unwrap_or_else(|| "UTC".into());
        let mut job = self.scheduler.update(
            &request.job_id,
            JobUpdate {
                schedule: request
                    .expression
                    .as_ref()
                    .map(|expression| schedule_spec(expression, &time_zone, now))
                    .transpose()?,
                action: request
                    .prompt
                    .clone()
                    .map(|instruction| ActionPayload::Scheduled { instruction }),
                limits: None,
                reply_route: None,
                missed_run: None,
            },
            now,
        )?;
        if let Some(paused) = request.paused {
            if paused && job.state == JobState::Active {
                job = self.scheduler.pause(&request.job_id, now)?;
            } else if !paused && job.state == JobState::Paused {
                job = self.scheduler.resume(&request.job_id, now)?;
            }
        }
        Ok(schedule_projection_from_job(&job))
    }

    fn delete_schedule(
        &self,
        scope_session_id: Option<&SessionId>,
        job_id: &keith_agent_types::JobId,
    ) -> Result<(), LocalRuntimeError> {
        ensure_session_scope(scope_session_id, &self.scheduler.session_id(job_id)?)?;
        self.scheduler.delete(job_id, UtcTimestamp::now()?)?;
        Ok(())
    }

    fn query_memory(&self, request: &MemoryQuery) -> Result<Vec<MemoryResult>, LocalRuntimeError> {
        let profile = self.profile(&request.profile_id)?;
        self.retrieval.rebuild_workspace(
            &request.profile_id,
            &profile.resources.workspace_root,
            UtcTimestamp::now()?,
        )?;
        Ok(self
            .retrieval
            .search(&request.profile_id, &request.query, request.limit)?
            .into_iter()
            .map(|result| MemoryResult {
                source: result.source_path,
                excerpt: result.excerpt,
                score_micros: score_micros(result.merged_score),
            })
            .collect())
    }

    fn export_session(
        &self,
        request: &ExportRequest,
    ) -> Result<ExportProjection, LocalRuntimeError> {
        let export = self.sessions.export(&request.session_id)?;
        let scope = ArtifactScope {
            root_tree_id: export.manifest.root_tree_id.clone(),
            session_id: export.manifest.session_id.clone(),
            profile_id: export.manifest.profile_id.clone(),
        };
        let (media_type, extension, bytes) = match request.format {
            ExportFormat::JsonLines => (
                "application/x-ndjson",
                "jsonl",
                session_json_lines(&export)?,
            ),
            ExportFormat::Markdown => (
                "text/markdown",
                "md",
                session_markdown(&export).into_bytes(),
            ),
            ExportFormat::PortableBundle => {
                let bytes = if request.include_artifacts {
                    let artifacts = self
                        .artifacts
                        .list(&scope)?
                        .into_iter()
                        .map(|metadata| {
                            let reference = ArtifactReference::from(&metadata);
                            self.artifacts.export(&scope, &reference).map(|artifact| {
                                serde_json::json!({
                                    "metadata": artifact.metadata,
                                    "content": artifact.content,
                                })
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    serde_json::to_vec(&serde_json::json!({
                        "format": "keith-session-portable-bundle",
                        "session": export,
                        "artifacts": artifacts,
                    }))?
                } else {
                    serde_json::to_vec(&export)?
                };
                ("application/vnd.keith.session+json", "json", bytes)
            }
        };
        let metadata = self.artifacts.create(NewArtifact {
            scope,
            source: ArtifactSource::User,
            media_type,
            bytes: &bytes,
            created_at: UtcTimestamp::now()?,
            display: Some(DisplayMetadata {
                name: Some(format!("session-export.{extension}")),
                description: Some("Portable session export".into()),
            }),
            retention: RetentionPolicy::Retain,
        })?;
        Ok(ExportProjection {
            artifact_id: metadata.id,
            media_type: metadata.media_type,
            byte_length: metadata.byte_length,
        })
    }

    fn set_background_control(
        &self,
        request: &keith_protocol::BackgroundControl,
    ) -> Result<BackgroundProjection, LocalRuntimeError> {
        self.profile(&request.profile_id)?;
        let projection = BackgroundProjection {
            profile_id: request.profile_id.clone(),
            mode: request.mode,
            pause_until: request.pause_until,
        };
        let current = self.background.get_record(
            Collection::ActiveOperations,
            request.profile_id.as_entity_id(),
        )?;
        let (revision, precondition) = if let Some(record) = current {
            (
                record.revision.checked_next().ok_or_else(|| {
                    LocalRuntimeError::Invalid("background-control revision overflowed".into())
                })?,
                WritePrecondition::Exact(record.revision),
            )
        } else {
            (Revision::ZERO, WritePrecondition::Missing)
        };
        self.background.transact(&[RecordMutation::Put {
            collection: Collection::ActiveOperations,
            record: VersionedRecord {
                version: CURRENT_SCHEMA_VERSION,
                id: request.profile_id.as_entity_id().clone(),
                revision,
                updated_at: UtcTimestamp::now()?,
                payload: serde_json::json!({
                    "kind": "background_control",
                    "projection": projection,
                }),
            },
            precondition,
        }])?;
        Ok(projection)
    }

    fn resolve_confirmation(
        &self,
        request: &keith_protocol::ConfirmationResolution,
    ) -> Result<(), LocalRuntimeError> {
        let current = self
            .background
            .get_record(Collection::ActiveOperations, &request.confirmation_id)?
            .ok_or_else(|| LocalRuntimeError::Invalid("confirmation was not found".into()))?;
        if current
            .payload
            .get("kind")
            .and_then(serde_json::Value::as_str)
            != Some("confirmation")
        {
            return Err(LocalRuntimeError::Invalid(
                "confirmation was not found".into(),
            ));
        }
        let revision = current
            .revision
            .checked_next()
            .ok_or_else(|| LocalRuntimeError::Invalid("confirmation revision overflowed".into()))?;
        self.background.transact(&[RecordMutation::Put {
            collection: Collection::ActiveOperations,
            record: VersionedRecord {
                version: CURRENT_SCHEMA_VERSION,
                id: request.confirmation_id.clone(),
                revision,
                updated_at: UtcTimestamp::now()?,
                payload: serde_json::json!({
                    "kind": "confirmation",
                    "resolved": true,
                    "decision": request.decision,
                }),
            },
            precondition: WritePrecondition::Exact(current.revision),
        }])?;
        Ok(())
    }

    fn cancel_target(
        &self,
        scope_session_id: Option<&SessionId>,
        target: &CancelTarget,
    ) -> Result<CommandResult, LocalRuntimeError> {
        let now = UtcTimestamp::now()?;
        match target {
            CancelTarget::Action(action_id) => {
                let action = self
                    .actions
                    .get(action_id)?
                    .ok_or_else(|| LocalRuntimeError::Invalid("action was not found".into()))?;
                ensure_session_scope(scope_session_id, &action.action.session_id)?;
                self.actions
                    .cancel(action_id, now, "Cancelled by operator")?;
                Ok(CommandResult::Accepted {
                    action_id: Some(action_id.clone()),
                })
            }
            CancelTarget::Goal(goal_id) => {
                let current = self
                    .goals
                    .get(goal_id)?
                    .ok_or_else(|| LocalRuntimeError::Invalid("goal was not found".into()))?;
                ensure_session_scope(scope_session_id, &current.session_id)?;
                let goal = self.goals.cancel(goal_id, "Cancelled by operator", now)?;
                Ok(CommandResult::Data(Box::new(ResponsePayload::Goal(
                    goal_projection(&goal),
                ))))
            }
            CancelTarget::Session(session_id) => {
                ensure_session_scope(scope_session_id, session_id)?;
                if let Some(token) = self
                    .active_cancellations
                    .lock()
                    .map_err(|_| LocalRuntimeError::LockPoisoned)?
                    .get(session_id)
                    .cloned()
                {
                    token.cancel();
                }
                self.children.parent_unavailable(session_id, now)?;
                self.sessions
                    .archive_session(session_id, runtime_writer_identity(Generation::ZERO, now))?;
                Ok(CommandResult::Accepted { action_id: None })
            }
            CancelTarget::Child(child_id) => {
                let current = self.children.projection(child_id)?;
                ensure_session_scope(scope_session_id, &current.parent_session_id)?;
                let child = self
                    .children
                    .cancel(child_id, "Cancelled by operator", now)?;
                Ok(CommandResult::Data(Box::new(ResponsePayload::Child(
                    child_projection(&keith_subagents::ChildProjection::from(&child)),
                ))))
            }
        }
    }

    fn steer(
        &self,
        client_id: &ClientId,
        request: &SteerAction,
        generation: Generation,
    ) -> Result<CommandResult, LocalRuntimeError> {
        if request.text.trim().is_empty() {
            return Err(LocalRuntimeError::Invalid(
                "steering text cannot be empty".into(),
            ));
        }
        self.sessions.manifest(&request.session_id)?;
        let action_id = ActionId::new();
        self.actions.submit(
            SessionAction {
                id: action_id.clone(),
                session_id: request.session_id.clone(),
                source: ActionSource::Steering {
                    client_id: client_id.clone(),
                },
                delivery: action_delivery(request.delivery),
                priority: ActionPriority::Interrupt,
                created_at: UtcTimestamp::now()?,
                not_before: None,
                deadline: None,
                limits: ActionLimits::default(),
                reply_route: Some(ActionReplyRoute::Client {
                    client_id: client_id.clone(),
                }),
                payload: ActionPayload::Steering {
                    text: request.text.clone(),
                },
            },
            UtcTimestamp::now()?,
        )?;
        match self.drain_session_actions(&request.session_id, generation, true)? {
            Some(snapshot) => Ok(CommandResult::Data(Box::new(ResponsePayload::Snapshot(
                Box::new(snapshot),
            )))),
            None => Ok(CommandResult::Accepted {
                action_id: Some(action_id),
            }),
        }
    }

    fn drain_session_actions(
        &self,
        session_id: &SessionId,
        generation: Generation,
        operator_initiated: bool,
    ) -> Result<Option<SessionSnapshot>, LocalRuntimeError> {
        if !operator_initiated && !self.background_allowed(session_id, UtcTimestamp::now()?)? {
            return Ok(None);
        }
        let mut last_snapshot = None;
        for _ in 0..64 {
            let Some(selected) = self.actions.select_next(
                session_id,
                UtcTimestamp::now()?,
                &PumpContext {
                    active_action: None,
                    at_turn_boundary: true,
                    session_idle: true,
                },
            )?
            else {
                break;
            };
            let action_id = selected.record.action.id.clone();
            self.actions
                .mark_running(&action_id, UtcTimestamp::now()?)?;
            let text = self.action_text(&selected.record.action.payload)?;
            match self.run_prompt(session_id, &text, generation) {
                Ok(snapshot) => {
                    self.actions.complete(&action_id, UtcTimestamp::now()?)?;
                    last_snapshot = Some(snapshot);
                }
                Err(error) => {
                    self.actions
                        .fail(&action_id, UtcTimestamp::now()?, error.to_string())?;
                    return Err(error);
                }
            }
        }
        Ok(last_snapshot)
    }

    fn action_text(&self, payload: &ActionPayload) -> Result<String, LocalRuntimeError> {
        match payload {
            ActionPayload::Prompt { text }
            | ActionPayload::Steering { text }
            | ActionPayload::FollowUp { text }
            | ActionPayload::Scheduled { instruction: text }
            | ActionPayload::ChildMessage { text, .. }
            | ActionPayload::ChannelMessage { text, .. } => Ok(text.clone()),
            ActionPayload::ContinueGoal { goal_id } => self
                .goals
                .get(goal_id)?
                .map(|goal| format!("Continue the active goal: {}", goal.objective))
                .ok_or_else(|| {
                    LocalRuntimeError::Invalid("continuation goal was not found".into())
                }),
            ActionPayload::Awareness { summary, .. } => Ok(summary.clone()),
            ActionPayload::SystemMaintenance { operation } => Ok(operation.clone()),
            ActionPayload::ResumeWaiting { .. } | ActionPayload::Refinement { .. } => Err(
                LocalRuntimeError::Invalid("queued action requires its owning service".into()),
            ),
        }
    }

    fn background_allowed(
        &self,
        session_id: &SessionId,
        now: UtcTimestamp,
    ) -> Result<bool, LocalRuntimeError> {
        let manifest = self.sessions.manifest(session_id)?;
        let Some(record) = self.background.get_record(
            Collection::ActiveOperations,
            manifest.profile_id.as_entity_id(),
        )?
        else {
            return Ok(true);
        };
        let projection = record
            .payload
            .get("projection")
            .cloned()
            .map(serde_json::from_value::<BackgroundProjection>)
            .transpose()?;
        Ok(projection.is_none_or(|control| {
            control.mode != BackgroundMode::Disabled
                && control
                    .pause_until
                    .is_none_or(|pause_until| pause_until <= now)
        }))
    }

    fn maintain_runtime(&self) -> Result<(), LocalRuntimeError> {
        let now = UtcTimestamp::now()?;
        let attempts = self.scheduler.tick(&self.scheduler_claimant, now)?;
        for attempt in attempts {
            let Some(action) = self.actions.get(&attempt.action_id)? else {
                self.scheduler.finish_attempt(
                    &attempt.attempt_id,
                    false,
                    Some("scheduled action disappeared after enqueue".into()),
                    UtcTimestamp::now()?,
                )?;
                continue;
            };
            let result =
                self.drain_session_actions(&action.action.session_id, Generation::ZERO, false);
            let state = self.actions.get(&attempt.action_id)?;
            let succeeded = state
                .as_ref()
                .is_some_and(|record| record.state == keith_action_store::ActionState::Completed);
            let mut detail = result.err().map(|error| error.to_string());
            if state.as_ref().is_some_and(|record| {
                matches!(
                    record.state,
                    keith_action_store::ActionState::Queued
                        | keith_action_store::ActionState::Admitted
                        | keith_action_store::ActionState::Waiting
                )
            }) {
                let reason = "background execution is disabled, paused, or blocked by earlier work";
                self.actions
                    .cancel(&attempt.action_id, UtcTimestamp::now()?, reason)?;
                detail = Some(reason.into());
            }
            self.scheduler.finish_attempt(
                &attempt.attempt_id,
                succeeded,
                detail,
                UtcTimestamp::now()?,
            )?;
        }
        Ok(())
    }

    fn register_child_roots(&self) -> Result<(), LocalRuntimeError> {
        for session in self.sessions()? {
            if session.archived {
                continue;
            }
            let profile = self.profile(&session.profile_id)?;
            self.children.register_root(ParentAuthority {
                session_id: session.session_id,
                root_tree_id: session.root_tree_id,
                profile_id: profile.profile.id.clone(),
                workspace_id: profile.profile.workspace_id.clone(),
                workspace_root: profile.resources.workspace_root.clone(),
                allowed_tools: allowed_tools(&profile),
            })?;
        }
        Ok(())
    }

    fn bootstrap_default_profile(&self, workspace_root: &Path) -> Result<(), LocalRuntimeError> {
        if !self.profiles.list()?.is_empty() {
            return Ok(());
        }
        fs::create_dir_all(workspace_root)?;
        let workspace_root = fs::canonicalize(workspace_root)?;
        let keith_root = workspace_root.join(".keith");
        let memory_root = keith_root.join("memory");
        let schedule_root = keith_root.join("schedules");
        for directory in [
            memory_root.join("daily"),
            schedule_root.clone(),
            keith_root.join("state"),
            keith_root.join("knowledge"),
            keith_root.join("skills"),
            keith_root.join("summaries"),
            keith_root.join("artifacts"),
            keith_root.join("backups"),
            keith_root.join("runtime"),
        ] {
            fs::create_dir_all(directory)?;
        }
        write_if_missing(
            &keith_root.join("PERSONA.md"),
            "You are Keith Agent, a precise local assistant that completes work and verifies results.\n",
        )?;
        write_if_missing(
            &keith_root.join("USER.md"),
            "The operator expects direct, complete, evidence-backed work.\n",
        )?;
        write_if_missing(
            &keith_root.join("RULES.md"),
            "Stay inside the configured workspace and use tools only when they advance the request.\n",
        )?;
        write_if_missing(
            &keith_root.join("MEMORY.md"),
            "# Durable memory\n\nUser-approved long-term facts and preferences live here.\n",
        )?;
        let now = UtcTimestamp::now()?;
        self.profiles.register(RegisteredProfile {
            profile: AgentProfile {
                version: CURRENT_SCHEMA_VERSION,
                id: ProfileId::new(),
                display_name: "Keith".into(),
                workspace_id: WorkspaceId::new(),
                persona_file: ".keith/PERSONA.md".into(),
                user_file: ".keith/USER.md".into(),
                rule_files: vec![".keith/RULES.md".into()],
                model_route: ProfileModelRoute {
                    provider: "openai".into(),
                    model: DEFAULT_OPENAI_MODEL.into(),
                    fallbacks: Vec::new(),
                    credential_ref: Some(DEFAULT_CREDENTIAL_REFERENCE.into()),
                },
                thinking: ThinkingLevel::Medium,
                tool_rules: BTreeMap::from([
                    ("read".into(), ToolPermission::Allow),
                    ("write".into(), ToolPermission::Allow),
                    ("list".into(), ToolPermission::Allow),
                    ("search".into(), ToolPermission::Allow),
                    ("bash".into(), ToolPermission::Allow),
                ]),
                enabled_skills: vec!["repository-awareness".into()],
                enabled_mcp_servers: Vec::new(),
                enabled_plugins: Vec::new(),
                channels: vec!["web".into(), "terminal".into()],
                autonomy: ProfileAutonomy {
                    mode: AutonomyMode::Bounded,
                    max_children: 4,
                    max_depth: 3,
                    daily_token_budget: 1_000_000,
                },
                notifications: NotificationSettings {
                    quiet_hours_start: "22:00".into(),
                    quiet_hours_end: "08:00".into(),
                    time_zone: TimeZoneName::parse("UTC")
                        .map_err(|error| ProfileError::Invalid(error.to_string()))?,
                    daily_limit: 24,
                },
                refinement: RefinementSettings {
                    enabled: true,
                    require_confirmation: true,
                    editable_targets: BTreeSet::from([
                        "persona".into(),
                        "rules".into(),
                        "skills".into(),
                    ]),
                },
            },
            resources: ProfileResources {
                workspace_root,
                memory_root,
                schedule_root,
            },
            enabled: true,
            authorized_callers: BTreeSet::from(["local-operator".into()]),
            revision: Revision::ZERO,
            updated_at: now,
        })?;
        Ok(())
    }

    fn profile(&self, profile_id: &ProfileId) -> Result<RegisteredProfile, LocalRuntimeError> {
        self.profiles
            .get(profile_id)?
            .ok_or_else(|| LocalRuntimeError::MissingProfile(profile_id.clone()))
    }

    fn ensure_supported_provider(&self, provider: &str) -> Result<(), LocalRuntimeError> {
        if self.available_providers.contains(provider) {
            Ok(())
        } else {
            let detail = provider_spec(provider).map_or_else(
                || provider.to_owned(),
                |provider| {
                    if provider.default_base_url.is_none() {
                        format!("{0} (configure --provider-base-url {0}=URL)", provider.id)
                    } else if provider.authentication == ProviderAuthentication::OAuth {
                        format!("{} (OAuth login is not configured)", provider.id)
                    } else {
                        provider.id.to_owned()
                    }
                },
            );
            Err(LocalRuntimeError::UnsupportedProvider(detail))
        }
    }

    fn prepare_model_route(&self, profile: &RegisteredProfile) -> Result<(), LocalRuntimeError> {
        let selected = &profile.profile.model_route;
        self.ensure_supported_provider(&selected.provider)?;
        let resolver = ProviderCredentialResolver::new(&self.credentials);
        let credential =
            resolver.resolve(&selected.provider, selected.credential_ref.as_deref())?;
        self.models
            .refresh_models(&selected.provider, &credential)?;
        self.models
            .register_configured_model(&selected.provider, &selected.model)?;
        for fallback in &selected.fallbacks {
            self.ensure_supported_provider(&fallback.provider)?;
            if fallback.provider != selected.provider {
                let credential =
                    resolver.resolve(&fallback.provider, selected.credential_ref.as_deref())?;
                self.models
                    .refresh_models(&fallback.provider, &credential)?;
            }
            self.models
                .register_configured_model(&fallback.provider, &fallback.model)?;
        }
        self.models.set_profile_route(
            profile.profile.id.clone(),
            ModelRoute {
                primary: ModelSelection {
                    provider: selected.provider.clone(),
                    model: selected.model.clone(),
                    credential_ref: selected.credential_ref.clone(),
                },
                fallbacks: selected
                    .fallbacks
                    .iter()
                    .map(|fallback: &ProfileModelSelection| ModelSelection {
                        provider: fallback.provider.clone(),
                        model: fallback.model.clone(),
                        credential_ref: selected.credential_ref.clone(),
                    })
                    .collect(),
                classification: None,
                summarization: None,
                review: None,
                vision: None,
            },
        )?;
        Ok(())
    }

    fn model_request(
        profile: &RegisteredProfile,
        entries: &[SessionEntry],
        tools: Vec<keith_provider_core::ToolDefinition>,
    ) -> Result<ModelRequest, LocalRuntimeError> {
        let mut system = Vec::new();
        for path in std::iter::once(&profile.profile.persona_file)
            .chain(std::iter::once(&profile.profile.user_file))
            .chain(profile.profile.rule_files.iter())
        {
            let content = fs::read_to_string(profile.resources.workspace_root.join(path))?;
            system.push(ProviderContentBlock::Text { text: content });
        }
        system.push(ProviderContentBlock::Text {
            text: format!(
                "Workspace: {}. Use the provided tools to inspect and modify it when needed.",
                profile.resources.workspace_root.display()
            ),
        });
        Ok(ModelRequest {
            request_id: EntityId::new(),
            model: profile.profile.model_route.model.clone(),
            system,
            messages: provider_messages(entries),
            tools,
            max_output_tokens: Some(16_384),
            temperature: None,
            reasoning_effort: Some(thinking_effort(profile.profile.thinking).into()),
        })
    }

    fn tool_manager(profile: &RegisteredProfile) -> Result<ToolManager, LocalRuntimeError> {
        let installation = ExecutionRules {
            default: ExecutionDecision::Allow,
            per_tool: BTreeMap::new(),
        };
        let profile_rules = ExecutionRules {
            default: ExecutionDecision::Deny,
            per_tool: profile
                .profile
                .tool_rules
                .iter()
                .map(|(name, permission)| (name.clone(), execution_decision(*permission)))
                .collect(),
        };
        let mut manager = ToolManager::new(
            installation,
            profile_rules,
            Arc::new(|_: &ToolInvocation, _: &ToolDefinition| false),
            ToolManagerConfig::default(),
        );
        let workspace = Arc::new(WorkspaceFs::open(
            &profile.resources.workspace_root,
            WorkspaceLimits::default(),
        )?);
        manager.register(Arc::new(ReadTool::new(Arc::clone(&workspace))))?;
        manager.register(Arc::new(WriteTool::new(Arc::clone(&workspace))))?;
        manager.register(Arc::new(ListTool::new(Arc::clone(&workspace))))?;
        manager.register(Arc::new(SearchTool::new(workspace)))?;
        manager.register(Arc::new(BashTool::new(&profile.resources.workspace_root)?))?;
        Ok(manager)
    }
}

fn ensure_session_scope(
    scope_session_id: Option<&SessionId>,
    actual_session_id: &SessionId,
) -> Result<(), LocalRuntimeError> {
    if scope_session_id.is_some_and(|scope| scope != actual_session_id) {
        Err(LocalRuntimeError::Invalid(
            "command target is outside the attached session".into(),
        ))
    } else {
        Ok(())
    }
}

fn runtime_writer_identity(generation: Generation, acquired_at: UtcTimestamp) -> WriterIdentity {
    WriterIdentity {
        worker_id: WorkerId::new(),
        owner_instance: EntityId::new(),
        generation,
        acquired_at,
    }
}

fn action_source_name(source: &ActionSource) -> &'static str {
    match source {
        ActionSource::Interactive { .. } => "interactive",
        ActionSource::Channel { .. } => "channel",
        ActionSource::Schedule { .. } => "schedule",
        ActionSource::Child { .. } => "child",
        ActionSource::Steering { .. } => "steering",
        ActionSource::FollowUp => "follow_up",
        ActionSource::Waiting { .. } => "waiting",
        ActionSource::Awareness { .. } => "awareness",
        ActionSource::Refinement { .. } => "refinement",
        ActionSource::AutonomousContinuation { .. } => "autonomous_continuation",
    }
}

const fn action_state_name(state: keith_action_store::ActionState) -> &'static str {
    match state {
        keith_action_store::ActionState::Queued => "queued",
        keith_action_store::ActionState::Admitted => "admitted",
        keith_action_store::ActionState::Running => "running",
        keith_action_store::ActionState::Waiting => "waiting",
        keith_action_store::ActionState::Completed => "completed",
        keith_action_store::ActionState::Failed => "failed",
        keith_action_store::ActionState::Cancelled => "cancelled",
        keith_action_store::ActionState::Expired => "expired",
    }
}

fn goal_limits(
    limits: &keith_protocol::GoalLimits,
    now: UtcTimestamp,
    mut result: RuntimeGoalLimits,
) -> Result<RuntimeGoalLimits, LocalRuntimeError> {
    if let Some(max_turns) = limits.max_turns {
        result.max_turns = max_turns;
    }
    if let Some(max_tokens) = limits.max_tokens {
        result.max_tokens = max_tokens;
    }
    if let Some(deadline) = limits.deadline {
        let remaining = deadline.unix_millis().saturating_sub(now.unix_millis());
        result.max_elapsed_ms = u64::try_from(remaining).map_err(|_| {
            LocalRuntimeError::Invalid("goal deadline must be in the future".into())
        })?;
    }
    Ok(result)
}

const fn runtime_goal_state(state: GoalState) -> RuntimeGoalState {
    match state {
        GoalState::Draft => RuntimeGoalState::Draft,
        GoalState::Ready => RuntimeGoalState::Ready,
        GoalState::Running => RuntimeGoalState::Running,
        GoalState::Waiting => RuntimeGoalState::Waiting,
        GoalState::Reviewing => RuntimeGoalState::Reviewing,
        GoalState::Paused => RuntimeGoalState::Paused,
        GoalState::Blocked => RuntimeGoalState::Blocked,
        GoalState::Complete => RuntimeGoalState::Complete,
        GoalState::Failed => RuntimeGoalState::Failed,
        GoalState::Cancelled => RuntimeGoalState::Cancelled,
    }
}

const fn protocol_goal_state(state: RuntimeGoalState) -> GoalState {
    match state {
        RuntimeGoalState::Draft => GoalState::Draft,
        RuntimeGoalState::Ready => GoalState::Ready,
        RuntimeGoalState::Running => GoalState::Running,
        RuntimeGoalState::Waiting => GoalState::Waiting,
        RuntimeGoalState::Reviewing => GoalState::Reviewing,
        RuntimeGoalState::Paused => GoalState::Paused,
        RuntimeGoalState::Blocked => GoalState::Blocked,
        RuntimeGoalState::Complete => GoalState::Complete,
        RuntimeGoalState::Failed => GoalState::Failed,
        RuntimeGoalState::Cancelled => GoalState::Cancelled,
    }
}

fn goal_projection(goal: &keith_goals::Goal) -> GoalProjection {
    GoalProjection {
        goal_id: goal.id.clone(),
        objective: goal.objective.clone(),
        state: protocol_goal_state(goal.state),
    }
}

fn allowed_tools(profile: &RegisteredProfile) -> BTreeSet<String> {
    profile
        .profile
        .tool_rules
        .iter()
        .filter(|(_, permission)| **permission != ToolPermission::Deny)
        .map(|(name, _)| name.clone())
        .collect()
}

const fn child_workspace_mode(mode: keith_protocol::ChildWorkspaceMode) -> ChildWorkspaceMode {
    match mode {
        keith_protocol::ChildWorkspaceMode::ReadOnlyParent => ChildWorkspaceMode::ReadOnlyParent,
        keith_protocol::ChildWorkspaceMode::IsolatedCopy => ChildWorkspaceMode::IsolatedCopy,
        keith_protocol::ChildWorkspaceMode::DedicatedWorkspace => {
            ChildWorkspaceMode::DedicatedWorkspace
        }
        keith_protocol::ChildWorkspaceMode::SharedWorkspace => ChildWorkspaceMode::SharedParent,
    }
}

fn child_limits(profile: &RegisteredProfile, limits: &keith_protocol::GoalLimits) -> ChildLimits {
    let mut result = ChildLimits {
        max_depth: profile.profile.autonomy.max_depth,
        max_direct_children: profile.profile.autonomy.max_children,
        ..ChildLimits::default()
    };
    if let Some(turns) = limits.max_turns {
        result.max_messages = turns.max(1);
    }
    if let Some(deadline) = limits.deadline {
        let remaining = deadline
            .unix_millis()
            .saturating_sub(UtcTimestamp::now().map_or(0, UtcTimestamp::unix_millis));
        result.max_runtime_ms = u64::try_from(remaining).unwrap_or(1).max(1);
    }
    result
}

fn child_projection(child: &keith_subagents::ChildProjection) -> ChildProjection {
    ChildProjection {
        child_id: child.id.clone(),
        session_id: child.session_id.clone(),
        objective: child.objective.clone(),
        state: child_status_name(child.status).into(),
    }
}

const fn child_status_name(status: ChildStatus) -> &'static str {
    match status {
        ChildStatus::Starting => "starting",
        ChildStatus::Running => "running",
        ChildStatus::Waiting => "waiting",
        ChildStatus::Complete => "complete",
        ChildStatus::Failed => "failed",
        ChildStatus::Cancelled => "cancelled",
        ChildStatus::Orphaned => "orphaned",
        ChildStatus::Archived => "archived",
    }
}

fn schedule_spec(
    expression: &ScheduleExpression,
    time_zone: &str,
    now: UtcTimestamp,
) -> Result<ScheduleSpec, LocalRuntimeError> {
    match expression {
        ScheduleExpression::Once(at) => Ok(ScheduleSpec::Once { at: *at }),
        ScheduleExpression::IntervalSeconds(seconds) => Ok(ScheduleSpec::Interval {
            every_ms: seconds.checked_mul(1_000).ok_or_else(|| {
                LocalRuntimeError::Invalid("schedule interval is too large".into())
            })?,
            anchor: now,
        }),
        ScheduleExpression::Calendar(expression) => Ok(ScheduleSpec::Calendar {
            expression: expression.clone(),
            time_zone: time_zone.to_owned(),
        }),
    }
}

fn protocol_schedule_expression(schedule: &ScheduleSpec) -> ScheduleExpression {
    match schedule {
        ScheduleSpec::Once { at } => ScheduleExpression::Once(*at),
        ScheduleSpec::Interval { every_ms, .. } => {
            ScheduleExpression::IntervalSeconds(every_ms.saturating_add(999) / 1_000)
        }
        ScheduleSpec::Calendar { expression, .. } => {
            ScheduleExpression::Calendar(expression.clone())
        }
    }
}

fn schedule_projection(projection: &keith_scheduler::ScheduleProjection) -> ScheduleProjection {
    ScheduleProjection {
        job_id: projection.job_id.clone(),
        expression: protocol_schedule_expression(&projection.schedule),
        next_run: projection.next_run,
        paused: projection.state == JobState::Paused,
    }
}

fn schedule_projection_from_job(job: &keith_scheduler::ScheduledJob) -> ScheduleProjection {
    ScheduleProjection {
        job_id: job.id.clone(),
        expression: protocol_schedule_expression(&job.schedule),
        next_run: job.next_run,
        paused: job.state == JobState::Paused,
    }
}

fn action_reply_route(route: &keith_protocol::ReplyRoute) -> ActionReplyRoute {
    ActionReplyRoute::Channel {
        channel: route.channel.clone(),
        conversation_id: route.conversation.clone(),
        thread_id: route.thread.clone(),
    }
}

const fn action_delivery(delivery: keith_protocol::DeliveryPolicy) -> ActionDeliveryPolicy {
    match delivery {
        keith_protocol::DeliveryPolicy::Immediate => ActionDeliveryPolicy::Immediate,
        keith_protocol::DeliveryPolicy::NextTurnBoundary => ActionDeliveryPolicy::NextTurnBoundary,
        keith_protocol::DeliveryPolicy::WhenIdle => ActionDeliveryPolicy::WhenIdle,
    }
}

fn score_micros(score: f32) -> u32 {
    format!("{:.0}", f64::from(score.clamp(0.0, 1.0)) * 1_000_000.0)
        .parse()
        .unwrap_or(0)
}

fn session_json_lines(
    export: &keith_session_store::SessionExport,
) -> Result<Vec<u8>, LocalRuntimeError> {
    let mut bytes = Vec::new();
    bytes.extend(serde_json::to_vec(&export.manifest)?);
    bytes.push(b'\n');
    for entry in &export.entries {
        bytes.extend(serde_json::to_vec(entry)?);
        bytes.push(b'\n');
    }
    Ok(bytes)
}

fn session_markdown(export: &keith_session_store::SessionExport) -> String {
    let mut output = format!(
        "# {}\n\n",
        export
            .manifest
            .label
            .as_deref()
            .unwrap_or("Keith session export")
    );
    for entry in &export.entries {
        let (role, text) = match &entry.payload {
            SessionEntryPayload::UserMessage { message } => ("User", stored_text(&message.content)),
            SessionEntryPayload::AssistantMessage { message } => {
                ("Assistant", stored_text(&message.content))
            }
            SessionEntryPayload::ToolResult { content, .. } => ("Tool", stored_text(content)),
            _ => continue,
        };
        writeln!(output, "## {role}\n\n{text}\n").expect("writing to an owned String cannot fail");
    }
    output
}

fn write_if_missing(path: &Path, content: &str) -> Result<(), std::io::Error> {
    if !path.exists() {
        fs::write(path, content)?;
    }
    Ok(())
}

fn thinking_effort(level: ThinkingLevel) -> &'static str {
    match level {
        ThinkingLevel::Minimal => "minimal",
        ThinkingLevel::Low => "low",
        ThinkingLevel::Medium => "medium",
        ThinkingLevel::High => "high",
    }
}

fn execution_decision(permission: ToolPermission) -> ExecutionDecision {
    match permission {
        ToolPermission::Deny => ExecutionDecision::Deny,
        ToolPermission::Confirm => ExecutionDecision::Confirm,
        ToolPermission::Allow => ExecutionDecision::Allow,
    }
}

fn provider_messages(entries: &[SessionEntry]) -> Vec<ProviderMessage> {
    let mut messages = Vec::<ProviderMessage>::new();
    for entry in entries {
        match &entry.payload {
            SessionEntryPayload::UserMessage { message } => messages.push(ProviderMessage {
                role: ProviderMessageRole::User,
                content: provider_text_content(&message.content),
            }),
            SessionEntryPayload::AssistantMessage { message } => messages.push(ProviderMessage {
                role: ProviderMessageRole::Assistant,
                content: provider_text_content(&message.content),
            }),
            SessionEntryPayload::ToolCall {
                call_id,
                name,
                arguments,
            } => {
                let call = ProviderContentBlock::ToolCall {
                    id: call_id.clone(),
                    name: name.clone(),
                    arguments: arguments.clone(),
                };
                if let Some(message) = messages
                    .last_mut()
                    .filter(|message| message.role == ProviderMessageRole::Assistant)
                {
                    message.content.push(call);
                } else {
                    messages.push(ProviderMessage {
                        role: ProviderMessageRole::Assistant,
                        content: vec![call],
                    });
                }
            }
            SessionEntryPayload::ToolResult {
                call_id,
                content,
                is_error,
            } => messages.push(ProviderMessage {
                role: ProviderMessageRole::Tool,
                content: vec![ProviderContentBlock::ToolResult {
                    call_id: call_id.clone(),
                    content: stored_text(content),
                    is_error: *is_error,
                }],
            }),
            _ => {}
        }
    }
    messages
}

fn provider_text_content(content: &[StoredContentBlock]) -> Vec<ProviderContentBlock> {
    let text = stored_text(content);
    if text.is_empty() {
        Vec::new()
    } else {
        vec![ProviderContentBlock::Text { text }]
    }
}

fn stored_text(content: &[StoredContentBlock]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            StoredContentBlock::Text { text } => Some(text.clone()),
            StoredContentBlock::Reasoning { .. } => None,
            StoredContentBlock::Artifact {
                artifact_id,
                media_type,
            } => Some(format!("Artifact {artifact_id} ({media_type})")),
            StoredContentBlock::Resource { uri, title } => Some(
                title
                    .as_ref()
                    .map_or_else(|| uri.clone(), |title| format!("{title}: {uri}")),
            ),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn message_projection(
    entry: &SessionEntry,
    role: ProjectionMessageRole,
    content: &[StoredContentBlock],
) -> MessageProjection {
    MessageProjection {
        message_id: MessageId::from(entry.id.as_entity_id().clone()),
        role,
        text: stored_text(content),
        committed: true,
    }
}

fn string_argument(invocation: &ToolInvocation, name: &str) -> Result<String, ToolExecutionError> {
    invocation
        .arguments
        .get(name)
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| ToolExecutionError::new(format!("missing string argument {name}")))
}

#[allow(clippy::needless_pass_by_value)]
fn tool_definition(
    name: &str,
    description: &str,
    properties: serde_json::Value,
    required: &[&str],
    behavior: ToolBehavior,
) -> ToolDefinition {
    ToolDefinition {
        name: name.into(),
        version: "1".into(),
        description: description.into(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        }),
        output_schema: serde_json::json!({"type": "string"}),
        behavior,
        repeatability: Repeatability::Safe,
        confirmation: ConfirmationMode::Never,
        timeout_ms: 120_000,
        output_limit_bytes: 4 * 1_024 * 1_024,
    }
}

struct ReadTool {
    definition: ToolDefinition,
    workspace: Arc<WorkspaceFs>,
}

impl ReadTool {
    fn new(workspace: Arc<WorkspaceFs>) -> Self {
        Self {
            definition: tool_definition(
                "read",
                "Read a UTF-8 or binary file inside the workspace",
                serde_json::json!({"path": {"type": "string"}}),
                &["path"],
                ToolBehavior::READ_ONLY,
            ),
            workspace,
        }
    }
}

impl ManagedTool for ReadTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn readiness(&self) -> Readiness {
        Readiness::Ready
    }

    fn execute(
        &self,
        invocation: &ToolInvocation,
        _progress: &mut dyn ProgressSink,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, ToolExecutionError> {
        let path = string_argument(invocation, "path")?;
        self.workspace
            .read(path, cancellation)
            .map_err(|error| ToolExecutionError::new(error.to_string()))
    }
}

struct WriteTool {
    definition: ToolDefinition,
    workspace: Arc<WorkspaceFs>,
}

impl WriteTool {
    fn new(workspace: Arc<WorkspaceFs>) -> Self {
        Self {
            definition: tool_definition(
                "write",
                "Atomically write a file inside the workspace",
                serde_json::json!({
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                }),
                &["path", "content"],
                ToolBehavior {
                    reads_state: true,
                    writes_state: true,
                    uses_network: false,
                    starts_processes: false,
                    parallel_safe: false,
                },
            ),
            workspace,
        }
    }
}

impl ManagedTool for WriteTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn readiness(&self) -> Readiness {
        Readiness::Ready
    }

    fn execute(
        &self,
        invocation: &ToolInvocation,
        _progress: &mut dyn ProgressSink,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, ToolExecutionError> {
        let path = string_argument(invocation, "path")?;
        let content = string_argument(invocation, "content")?;
        let change = self
            .workspace
            .write_atomic(
                path,
                content.as_bytes(),
                &ExpectedPreimage::Any,
                cancellation,
            )
            .map_err(|error| ToolExecutionError::new(error.to_string()))?;
        serde_json::to_vec(&change).map_err(|error| ToolExecutionError::new(error.to_string()))
    }
}

struct ListTool {
    definition: ToolDefinition,
    workspace: Arc<WorkspaceFs>,
}

impl ListTool {
    fn new(workspace: Arc<WorkspaceFs>) -> Self {
        Self {
            definition: tool_definition(
                "list",
                "List files and directories inside the workspace",
                serde_json::json!({"path": {"type": "string"}}),
                &["path"],
                ToolBehavior::READ_ONLY,
            ),
            workspace,
        }
    }
}

impl ManagedTool for ListTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn readiness(&self) -> Readiness {
        Readiness::Ready
    }

    fn execute(
        &self,
        invocation: &ToolInvocation,
        _progress: &mut dyn ProgressSink,
        _cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, ToolExecutionError> {
        let path = string_argument(invocation, "path")?;
        let entries = self
            .workspace
            .list(path)
            .map_err(|error| ToolExecutionError::new(error.to_string()))?;
        let lines = entries
            .into_iter()
            .map(|entry| {
                format!(
                    "{}\t{}\t{}",
                    if entry.is_directory {
                        "directory"
                    } else {
                        "file"
                    },
                    entry.bytes,
                    entry.name.to_string_lossy()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(lines.into_bytes())
    }
}

struct SearchTool {
    definition: ToolDefinition,
    workspace: Arc<WorkspaceFs>,
}

impl SearchTool {
    fn new(workspace: Arc<WorkspaceFs>) -> Self {
        Self {
            definition: tool_definition(
                "search",
                "Search workspace files for literal text",
                serde_json::json!({
                    "path": {"type": "string"},
                    "query": {"type": "string"}
                }),
                &["path", "query"],
                ToolBehavior::READ_ONLY,
            ),
            workspace,
        }
    }
}

impl ManagedTool for SearchTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn readiness(&self) -> Readiness {
        Readiness::Ready
    }

    fn execute(
        &self,
        invocation: &ToolInvocation,
        _progress: &mut dyn ProgressSink,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, ToolExecutionError> {
        let path = string_argument(invocation, "path")?;
        let query = string_argument(invocation, "query")?;
        let matches = self
            .workspace
            .search(path, &query, cancellation)
            .map_err(|error| ToolExecutionError::new(error.to_string()))?;
        Ok(matches
            .into_iter()
            .map(|item| format!("{}:{}:{}", item.path.display(), item.line, item.text))
            .collect::<Vec<_>>()
            .join("\n")
            .into_bytes())
    }
}

struct BashTool {
    definition: ToolDefinition,
    runner: RestrictedProcessRunner,
    program: PathBuf,
}

impl BashTool {
    fn new(workspace_root: &Path) -> Result<Self, LocalRuntimeError> {
        let program = PathBuf::from("/bin/bash");
        let runner = RestrictedProcessRunner::new(
            workspace_root,
            [program.clone()],
            BTreeSet::new(),
            BTreeMap::from([(
                "PATH".into(),
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin".into(),
            )]),
        )?;
        Ok(Self {
            definition: tool_definition(
                "bash",
                "Run a shell command in the workspace",
                serde_json::json!({"command": {"type": "string"}}),
                &["command"],
                ToolBehavior {
                    reads_state: true,
                    writes_state: true,
                    uses_network: true,
                    starts_processes: true,
                    parallel_safe: false,
                },
            ),
            runner,
            program,
        })
    }
}

impl ManagedTool for BashTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    fn readiness(&self) -> Readiness {
        Readiness::Ready
    }

    fn execute(
        &self,
        invocation: &ToolInvocation,
        _progress: &mut dyn ProgressSink,
        cancellation: &CancellationToken,
    ) -> Result<Vec<u8>, ToolExecutionError> {
        let command = string_argument(invocation, "command")?;
        let request = RunRequest {
            program: self.program.clone(),
            arguments: vec!["-lc".into(), command],
            working_directory: PathBuf::from("."),
            environment: BTreeMap::new(),
            isolation: IsolationRequest::TrustedWorkspace,
            limits: ProcessLimits::default(),
        };
        let mut chunks = Vec::new();
        let result = self
            .runner
            .run(
                &request,
                cancellation,
                &mut |chunk: &keith_tool_runner_core::OutputChunk| {
                    chunks.extend_from_slice(&chunk.bytes);
                },
            )
            .map_err(|error| ToolExecutionError::new(error.to_string()))?;
        chunks.extend_from_slice(format!("\nexit_code={:?}", result.exit_code).as_bytes());
        Ok(chunks)
    }
}

impl From<keith_tool_runner_core::WorkspaceError> for LocalRuntimeError {
    fn from(error: keith_tool_runner_core::WorkspaceError) -> Self {
        Self::Tool(ToolManagerError::Unready(error.to_string()))
    }
}

impl From<keith_tool_runner_core::RunError> for LocalRuntimeError {
    fn from(error: keith_tool_runner_core::RunError) -> Self {
        Self::Tool(ToolManagerError::Unready(error.to_string()))
    }
}

impl CommandRuntime for LocalRuntime {
    fn profiles(&self) -> Result<Vec<ProfileSummary>, String> {
        LocalRuntime::profiles(self).map_err(|error| error.to_string())
    }

    fn sessions(&self) -> Result<Vec<RuntimeSession>, String> {
        LocalRuntime::sessions(self)
            .map(|sessions| sessions.iter().map(runtime_session).collect())
            .map_err(|error| error.to_string())
    }

    fn create_default_session(&self, title: Option<String>) -> Result<RuntimeSession, String> {
        let profile = self
            .registered_profiles()
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|profile| profile.enabled)
            .ok_or_else(|| "no enabled runtime profile is available".to_owned())?;
        LocalRuntime::create_session(
            self,
            &profile.profile.id,
            &profile.profile.workspace_id,
            title,
        )
        .map(|session| runtime_session(&session))
        .map_err(|error| error.to_string())
    }

    fn create_session(
        &self,
        request: &keith_protocol::CreateSession,
    ) -> Result<RuntimeSession, String> {
        LocalRuntime::create_session(
            self,
            &request.profile_id,
            &request.workspace_id,
            request.title.clone(),
        )
        .map(|session| runtime_session(&session))
        .map_err(|error| error.to_string())
    }

    fn select_model(&self, selection: &keith_protocol::ModelSelection) -> Result<(), String> {
        LocalRuntime::select_model(
            self,
            &selection.session_id,
            selection.provider.clone(),
            selection.model.clone(),
        )
        .map_err(|error| error.to_string())
    }

    fn run_prompt(
        &self,
        prompt: &keith_protocol::SubmitPrompt,
        generation: Generation,
    ) -> Result<SessionSnapshot, String> {
        LocalRuntime::run_prompt(self, &prompt.session_id, &prompt.text, generation)
            .map_err(|error| error.to_string())
    }

    fn snapshot(
        &self,
        session_id: &SessionId,
        generation: Generation,
        state: SessionState,
    ) -> Result<SessionSnapshot, String> {
        LocalRuntime::snapshot(self, session_id, generation, state)
            .map_err(|error| error.to_string())
    }

    fn execute_feature(
        &self,
        client_id: &ClientId,
        scope_session_id: Option<&SessionId>,
        command: &ClientCommand,
        generation: Generation,
    ) -> Result<CommandResult, String> {
        let result = match command {
            ClientCommand::BranchSession(request) => {
                LocalRuntime::branch_session(self, request, generation).map(|snapshot| {
                    CommandResult::Data(Box::new(ResponsePayload::Snapshot(Box::new(snapshot))))
                })
            }
            ClientCommand::SelectBranch(request) => {
                LocalRuntime::select_branch(self, request, generation).map(|snapshot| {
                    CommandResult::Data(Box::new(ResponsePayload::Snapshot(Box::new(snapshot))))
                })
            }
            ClientCommand::Steer(request) => {
                LocalRuntime::steer(self, client_id, request, generation)
            }
            ClientCommand::Cancel(target) => {
                LocalRuntime::cancel_target(self, scope_session_id, target)
            }
            ClientCommand::CreateGoal(request) => LocalRuntime::create_goal(self, request)
                .map(|goal| CommandResult::Data(Box::new(ResponsePayload::Goal(goal)))),
            ClientCommand::UpdateGoal(request) => {
                LocalRuntime::update_goal(self, scope_session_id, request)
                    .map(|goal| CommandResult::Data(Box::new(ResponsePayload::Goal(goal))))
            }
            ClientCommand::ListGoals { session_id } => {
                LocalRuntime::snapshot(self, session_id, generation, SessionState::Ready).map(
                    |snapshot| {
                        CommandResult::Data(Box::new(ResponsePayload::Snapshot(Box::new(snapshot))))
                    },
                )
            }
            ClientCommand::ListChildren { session_id } => {
                LocalRuntime::snapshot(self, session_id, generation, SessionState::Ready).map(
                    |snapshot| {
                        CommandResult::Data(Box::new(ResponsePayload::Snapshot(Box::new(snapshot))))
                    },
                )
            }
            ClientCommand::CreateChild(request) => LocalRuntime::create_child(self, request)
                .map(|child| CommandResult::Data(Box::new(ResponsePayload::Child(child)))),
            ClientCommand::SendChildMessage(request) => {
                LocalRuntime::send_child_message(self, scope_session_id, request)
                    .map(|child| CommandResult::Data(Box::new(ResponsePayload::Child(child))))
            }
            ClientCommand::ArchiveChild { child_id } => {
                LocalRuntime::archive_child(self, scope_session_id, child_id)
                    .map(|child| CommandResult::Data(Box::new(ResponsePayload::Child(child))))
            }
            ClientCommand::CreateSchedule(request) => LocalRuntime::create_schedule(self, request)
                .map(|schedule| CommandResult::Data(Box::new(ResponsePayload::Schedule(schedule)))),
            ClientCommand::UpdateSchedule(request) => {
                LocalRuntime::update_schedule(self, scope_session_id, request).map(|schedule| {
                    CommandResult::Data(Box::new(ResponsePayload::Schedule(schedule)))
                })
            }
            ClientCommand::DeleteSchedule { job_id } => self
                .delete_schedule(scope_session_id, job_id)
                .map(|()| CommandResult::Accepted { action_id: None }),
            ClientCommand::QueryMemory(request) => LocalRuntime::query_memory(self, request)
                .map(|memory| CommandResult::Data(Box::new(ResponsePayload::Memory(memory)))),
            ClientCommand::ResolveConfirmation(request) => {
                LocalRuntime::resolve_confirmation(self, request)
                    .map(|()| CommandResult::Accepted { action_id: None })
            }
            ClientCommand::Export(request) => LocalRuntime::export_session(self, request)
                .map(|export| CommandResult::Data(Box::new(ResponsePayload::Export(export)))),
            ClientCommand::SetBackgroundControl(request) => {
                LocalRuntime::set_background_control(self, request).map(|control| {
                    CommandResult::Data(Box::new(ResponsePayload::Background(control)))
                })
            }
            ClientCommand::ListProfiles
            | ClientCommand::ListSessions(_)
            | ClientCommand::CreateSession(_)
            | ClientCommand::AttachSession(_)
            | ClientCommand::DetachSession { .. }
            | ClientCommand::AcknowledgeEvents(_)
            | ClientCommand::ResumeSession { .. }
            | ClientCommand::SubmitPrompt(_)
            | ClientCommand::SelectModel(_) => Err(LocalRuntimeError::UnsupportedCommand),
        };
        result.map_err(|error| error.to_string())
    }

    fn maintain(&self) -> Result<(), String> {
        self.maintain_runtime().map_err(|error| error.to_string())
    }
}

fn runtime_session(session: &SessionManifest) -> RuntimeSession {
    RuntimeSession {
        session_id: session.session_id.clone(),
        root_tree_id: session.root_tree_id.clone(),
        profile_id: session.profile_id.clone(),
        title: session.label.clone(),
        archived: session.archived,
        created_at: session.created_at,
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use keith_credentials::{CredentialOwner, CredentialRef, SecretValue};

    use super::*;

    struct ProviderServer {
        base_url: String,
        requests: mpsc::Receiver<String>,
        thread: Option<thread::JoinHandle<()>>,
    }

    impl ProviderServer {
        fn start(responses: Vec<String>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let (sender, requests) = mpsc::channel();
            let thread = thread::spawn(move || {
                for response in responses {
                    let (mut stream, _) = listener.accept().unwrap();
                    let request = read_request(&mut stream);
                    sender.send(request).unwrap();
                    stream.write_all(response.as_bytes()).unwrap();
                    stream.flush().unwrap();
                }
            });
            Self {
                base_url: format!("http://{address}"),
                requests,
                thread: Some(thread),
            }
        }

        fn request(&self) -> String {
            self.requests.recv_timeout(Duration::from_secs(5)).unwrap()
        }
    }

    impl Drop for ProviderServer {
        fn drop(&mut self) {
            if let Some(thread) = self.thread.take() {
                thread.join().unwrap();
            }
        }
    }

    fn read_request(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let read = stream.read(&mut buffer).unwrap();
            if read == 0 {
                break;
            }
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(header_end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
                let headers = String::from_utf8_lossy(&bytes[..header_end + 4]);
                let length = headers
                    .lines()
                    .find_map(|line| {
                        line.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|value| value.trim().parse::<usize>().ok())
                    })
                    .unwrap_or(0);
                if bytes.len() >= header_end + 4 + length {
                    break;
                }
            }
        }
        String::from_utf8(bytes).unwrap()
    }

    fn response(content_type: &str, body: &str) -> String {
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn clean_install_runs_real_provider_tool_turn_and_resumes_after_restart() {
        let models = r#"{"data":[{"id":"gpt-4.1-mini"}]}"#;
        let tool_turn = concat!(
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call_write\",\"function\":{\"name\":\"write\",\"arguments\":\"{\\\"path\\\":\\\"provider-proof.txt\\\",\\\"content\\\":\\\"real provider tool turn\\\\n\\\"}\"}}]},\"finish_reason\":\"tool_calls\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":11,\"completion_tokens\":7}}\n\n",
            "data: [DONE]\n\n"
        );
        let final_turn = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"The provider wrote the proof file.\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":19,\"completion_tokens\":8}}\n\n",
            "data: [DONE]\n\n"
        );
        let server = ProviderServer::start(vec![
            response("application/json", models),
            response("text/event-stream", tool_turn),
            response("text/event-stream", final_turn),
        ]);
        let root = tempfile::tempdir().unwrap();
        let data_root = root.path().join("data");
        let credential_root = data_root.join("credentials");
        let workspace_root = root.path().join("workspace");
        let key = [23_u8; 32];
        let runtime = LocalRuntime::open(LocalRuntimeConfig {
            data_root: data_root.clone(),
            credential_root: credential_root.clone(),
            credential_key: MasterKey::from_bytes(key),
            workspace_root: workspace_root.clone(),
            openai_base_url: server.base_url.clone(),
            anthropic_base_url: server.base_url.clone(),
            provider_base_urls: BTreeMap::new(),
        })
        .unwrap();
        runtime
            .credentials
            .put(
                CredentialRef::new(
                    DEFAULT_CREDENTIAL_REFERENCE,
                    CredentialOwner::Provider("openai".into()),
                )
                .unwrap(),
                SecretValue::new("provider-integration-secret").unwrap(),
                UtcTimestamp::now().unwrap(),
            )
            .unwrap();
        let profile = runtime.registered_profiles().unwrap().remove(0);
        let session = runtime
            .create_session(
                &profile.profile.id,
                &profile.profile.workspace_id,
                Some("Provider integration".into()),
            )
            .unwrap();
        let snapshot = runtime
            .run_prompt(
                &session.session_id,
                "Write the provider proof file.",
                Generation::new(1),
            )
            .unwrap();
        assert_eq!(
            fs::read_to_string(workspace_root.join("provider-proof.txt")).unwrap(),
            "real provider tool turn\n"
        );
        assert!(snapshot.tools.iter().any(|tool| tool.terminal));
        assert!(snapshot.messages.iter().any(|message| {
            message.role == ProjectionMessageRole::Assistant
                && message.text == "The provider wrote the proof file."
        }));
        assert!(
            snapshot
                .messages
                .iter()
                .any(|message| message.role == ProjectionMessageRole::Tool)
        );
        assert_eq!(snapshot.usage.input_tokens, 30);
        assert_eq!(snapshot.usage.output_tokens, 15);

        let discovery_request = server.request();
        let first_turn_request = server.request();
        let second_turn_request = server.request();
        assert!(discovery_request.starts_with("GET /v1/models "));
        assert!(discovery_request.contains("authorization: Bearer provider-integration-secret"));
        assert!(first_turn_request.starts_with("POST /v1/chat/completions "));
        assert!(first_turn_request.contains("\"name\":\"write\""));
        assert!(second_turn_request.contains("\"role\":\"tool\""));
        assert!(
            !first_turn_request
                .split("\r\n\r\n")
                .nth(1)
                .unwrap()
                .contains("provider-integration-secret")
        );

        drop(runtime);
        let restarted = LocalRuntime::open(LocalRuntimeConfig {
            data_root,
            credential_root,
            credential_key: MasterKey::from_bytes(key),
            workspace_root,
            openai_base_url: server.base_url.clone(),
            anthropic_base_url: server.base_url.clone(),
            provider_base_urls: BTreeMap::new(),
        })
        .unwrap();
        let resumed = restarted
            .snapshot(&session.session_id, Generation::new(2), SessionState::Ready)
            .unwrap();
        assert_eq!(resumed.messages, snapshot.messages);
        assert_eq!(resumed.tools, snapshot.tools);
        assert_eq!(resumed.usage, snapshot.usage);
    }

    #[test]
    fn union_catalog_registers_every_provider_when_deployment_endpoints_are_supplied() {
        let root = tempfile::tempdir().unwrap();
        let overrides = BUILTIN_PROVIDERS
            .iter()
            .filter(|provider| provider.default_base_url.is_none())
            .map(|provider| (provider.id.to_owned(), "http://127.0.0.1:65535".to_owned()))
            .collect();
        let runtime = LocalRuntime::open(LocalRuntimeConfig {
            data_root: root.path().join("data"),
            credential_root: root.path().join("credentials"),
            credential_key: MasterKey::from_bytes([91; 32]),
            workspace_root: root.path().join("workspace"),
            openai_base_url: "http://127.0.0.1:65535".into(),
            anthropic_base_url: "http://127.0.0.1:65535".into(),
            provider_base_urls: overrides,
        })
        .unwrap();
        let expected = BUILTIN_PROVIDERS
            .iter()
            .map(|provider| provider.id.to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(runtime.available_providers, expected);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn advertised_commands_use_durable_domain_services_and_survive_restart() {
        let root = tempfile::tempdir().unwrap();
        let data_root = root.path().join("data");
        let credential_root = root.path().join("credentials");
        let workspace_root = root.path().join("workspace");
        let key = [37_u8; 32];
        let configuration = || LocalRuntimeConfig {
            data_root: data_root.clone(),
            credential_root: credential_root.clone(),
            credential_key: MasterKey::from_bytes(key),
            workspace_root: workspace_root.clone(),
            openai_base_url: "http://127.0.0.1:65535".into(),
            anthropic_base_url: "http://127.0.0.1:65535".into(),
            provider_base_urls: BTreeMap::new(),
        };
        let runtime = LocalRuntime::open(configuration()).unwrap();
        let profile = runtime.registered_profiles().unwrap().remove(0);
        let session = runtime
            .create_session(
                &profile.profile.id,
                &profile.profile.workspace_id,
                Some("Feature composition".into()),
            )
            .unwrap();
        let mut writer = runtime
            .sessions
            .acquire_writer(
                &session.session_id,
                runtime_writer_identity(Generation::new(3), UtcTimestamp::now().unwrap()),
            )
            .unwrap();
        let first_entry = writer
            .append(
                None,
                UtcTimestamp::now().unwrap(),
                SessionEntryPayload::UserMessage {
                    message: StoredMessage {
                        role: StoredMessageRole::User,
                        content: vec![StoredContentBlock::Text {
                            text: "branch point".into(),
                        }],
                        provider_metadata: BTreeMap::new(),
                    },
                },
            )
            .unwrap();
        drop(writer);

        let client_id = ClientId::new();
        let branch = runtime
            .execute_feature(
                &client_id,
                Some(&session.session_id),
                &ClientCommand::BranchSession(BranchRequest {
                    session_id: session.session_id.clone(),
                    parent_entry_id: first_entry.id.as_entity_id().clone(),
                    label: Some("alternate".into()),
                }),
                Generation::new(3),
            )
            .unwrap();
        assert!(matches!(branch, CommandResult::Data(_)));

        let goal = runtime
            .execute_feature(
                &client_id,
                Some(&session.session_id),
                &ClientCommand::CreateGoal(CreateGoal {
                    session_id: session.session_id.clone(),
                    objective: "Exercise durable feature wiring".into(),
                    limits: keith_protocol::GoalLimits {
                        max_turns: Some(12),
                        max_tokens: Some(50_000),
                        deadline: None,
                    },
                }),
                Generation::new(3),
            )
            .unwrap();
        let goal_id = match goal {
            CommandResult::Data(payload) => match *payload {
                ResponsePayload::Goal(goal) => goal.goal_id,
                other => panic!("unexpected goal response: {other:?}"),
            },
            other => panic!("unexpected command response: {other:?}"),
        };
        runtime
            .execute_feature(
                &client_id,
                Some(&session.session_id),
                &ClientCommand::UpdateGoal(UpdateGoal {
                    goal_id: goal_id.clone(),
                    objective: None,
                    state: Some(GoalState::Running),
                    limits: None,
                }),
                Generation::new(3),
            )
            .unwrap();
        let other_session = runtime
            .create_session(
                &profile.profile.id,
                &profile.profile.workspace_id,
                Some("Cross-scope probe".into()),
            )
            .unwrap();
        assert!(
            runtime
                .execute_feature(
                    &client_id,
                    Some(&other_session.session_id),
                    &ClientCommand::UpdateGoal(UpdateGoal {
                        goal_id,
                        objective: Some("Cross-session overwrite".into()),
                        state: None,
                        limits: None,
                    }),
                    Generation::new(3),
                )
                .unwrap_err()
                .contains("outside the attached session")
        );

        let child = runtime
            .execute_feature(
                &client_id,
                Some(&session.session_id),
                &ClientCommand::CreateChild(CreateChild {
                    parent_session_id: session.session_id.clone(),
                    objective: "Inspect the feature composition".into(),
                    workspace_mode: keith_protocol::ChildWorkspaceMode::SharedWorkspace,
                    limits: keith_protocol::GoalLimits {
                        max_turns: Some(8),
                        max_tokens: Some(10_000),
                        deadline: None,
                    },
                }),
                Generation::new(3),
            )
            .unwrap();
        let child_id = match child {
            CommandResult::Data(payload) => match *payload {
                ResponsePayload::Child(child) => child.child_id,
                other => panic!("unexpected child response: {other:?}"),
            },
            other => panic!("unexpected command response: {other:?}"),
        };
        runtime
            .execute_feature(
                &client_id,
                Some(&session.session_id),
                &ClientCommand::SendChildMessage(keith_protocol::ChildMessageRequest {
                    child_id: child_id.clone(),
                    text: "Return a status update".into(),
                    artifact_ids: Vec::new(),
                }),
                Generation::new(3),
            )
            .unwrap();

        let schedule = runtime
            .execute_feature(
                &client_id,
                Some(&session.session_id),
                &ClientCommand::CreateSchedule(CreateSchedule {
                    profile_id: profile.profile.id.clone(),
                    session_id: Some(session.session_id.clone()),
                    expression: ScheduleExpression::IntervalSeconds(86_400),
                    time_zone: "UTC".into(),
                    prompt: "Prepare a daily status".into(),
                    reply_route: None,
                }),
                Generation::new(3),
            )
            .unwrap();
        assert!(matches!(schedule, CommandResult::Data(_)));

        fs::write(
            workspace_root.join("MEMORY.md"),
            "# Durable facts\nThe sapphire launch code belongs to the feature test.\n",
        )
        .unwrap();
        let memory = runtime
            .execute_feature(
                &client_id,
                Some(&session.session_id),
                &ClientCommand::QueryMemory(MemoryQuery {
                    profile_id: profile.profile.id.clone(),
                    query: "sapphire launch".into(),
                    limit: 5,
                }),
                Generation::new(3),
            )
            .unwrap();
        assert!(matches!(
            memory,
            CommandResult::Data(payload)
                if matches!(*payload, ResponsePayload::Memory(ref results) if !results.is_empty())
        ));

        let exported = runtime
            .execute_feature(
                &client_id,
                Some(&session.session_id),
                &ClientCommand::Export(ExportRequest {
                    session_id: session.session_id.clone(),
                    format: ExportFormat::PortableBundle,
                    include_artifacts: false,
                }),
                Generation::new(3),
            )
            .unwrap();
        assert!(matches!(exported, CommandResult::Data(_)));
        runtime
            .execute_feature(
                &client_id,
                Some(&session.session_id),
                &ClientCommand::SetBackgroundControl(keith_protocol::BackgroundControl {
                    profile_id: profile.profile.id.clone(),
                    mode: BackgroundMode::Disabled,
                    pause_until: None,
                }),
                Generation::new(3),
            )
            .unwrap();

        drop(runtime);
        let restarted = LocalRuntime::open(configuration()).unwrap();
        let snapshot = restarted
            .snapshot(&session.session_id, Generation::new(4), SessionState::Ready)
            .unwrap();
        assert_eq!(snapshot.goals.len(), 1);
        assert_eq!(snapshot.goals[0].state, GoalState::Running);
        assert_eq!(snapshot.children.len(), 1);
        assert_eq!(snapshot.children[0].child_id, child_id);
        assert_eq!(snapshot.schedules.len(), 1);
        assert_eq!(
            restarted
                .sessions
                .manifest(&session.session_id)
                .unwrap()
                .branch_labels
                .get("alternate"),
            Some(&first_entry.id)
        );
    }
}
