#![forbid(unsafe_code)]

use keith_agent_types::{ProfileId, RootTreeId, SessionId, UtcTimestamp, WorkspaceId};
use keith_model_registry::{
    ModelPurpose, ModelRegistry, ModelRoute as RegistryModelRoute,
    ModelSelection as RegistryModelSelection, RegistryError, ResolvedRoute,
};
use keith_profile::{ProfileError, ProfileRegistry, RegisteredProfile};
use keith_session_store::{
    NewSession, ProfileSnapshotMetadata, SessionKind, SessionManifest, SessionStore,
    SessionStoreError, WriterIdentity,
};
use keith_state_store_core::ProfileRepository;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplyRoute {
    pub channel: String,
    pub destination: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileRefreshPolicy {
    KeepPinned,
    ApplyLatest,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionPolicy {
    pub profile_refresh: ProfileRefreshPolicy,
    pub memory_enabled: bool,
    pub schedules_enabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteRequest {
    pub profile_id: Option<ProfileId>,
    pub workspace_id: Option<WorkspaceId>,
    pub caller: String,
    pub reply: ReplyRoute,
    pub session_policy: SessionPolicy,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedModelConfiguration {
    pub provider: String,
    pub model: String,
    pub credential_ref: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolvedProfileSnapshot {
    pub profile: RegisteredProfile,
    pub model: ResolvedModelConfiguration,
    pub reply: ReplyRoute,
    pub session_policy: SessionPolicy,
    pub resolved_at: UtcTimestamp,
}

pub struct ResolvedSessionRoute {
    pub snapshot: ResolvedProfileSnapshot,
    pub model_route: ResolvedRoute,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewRootSession {
    pub session_id: SessionId,
    pub root_tree_id: RootTreeId,
    pub created_at: UtcTimestamp,
    pub label: Option<String>,
}

#[derive(Debug, Error)]
pub enum RouteError {
    #[error("no profile matches the requested route")]
    Missing,
    #[error("the route matches more than one profile")]
    Ambiguous,
    #[error("profile {0} is disabled")]
    Disabled(ProfileId),
    #[error("the caller is not authorized for profile {0}")]
    Unauthorized(ProfileId),
    #[error("reply channel {channel} is not enabled for profile {profile}")]
    ChannelDisabled { profile: ProfileId, channel: String },
    #[error("route input is invalid: {0}")]
    Invalid(String),
    #[error("the model registry route differs from the profile snapshot")]
    ModelRouteMismatch,
    #[error("the requested route does not match the durable session identity")]
    SessionIdentityMismatch,
    #[error("the session has no resolved profile snapshot")]
    MissingSnapshot,
    #[error("profile registry failed: {0}")]
    Profile(#[from] ProfileError),
    #[error("model registry failed: {0}")]
    Model(#[from] RegistryError),
    #[error("session storage failed: {0}")]
    Session(#[from] SessionStoreError),
    #[error("route snapshot serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub struct RouteResolver<'a, R> {
    profiles: &'a ProfileRegistry<R>,
    models: &'a ModelRegistry,
    sessions: &'a SessionStore,
}

impl<'a, R> RouteResolver<'a, R>
where
    R: ProfileRepository,
{
    pub const fn new(
        profiles: &'a ProfileRegistry<R>,
        models: &'a ModelRegistry,
        sessions: &'a SessionStore,
    ) -> Self {
        Self {
            profiles,
            models,
            sessions,
        }
    }

    /// Deliberately binds a profile's configured model route to discovered real providers.
    ///
    /// # Errors
    ///
    /// Returns an error when a provider/model is missing or the route is invalid.
    pub fn synchronize_model_route(&self, profile: &RegisteredProfile) -> Result<(), RouteError> {
        let configured = &profile.profile.model_route;
        self.models.set_profile_route(
            profile.profile.id.clone(),
            RegistryModelRoute {
                primary: RegistryModelSelection {
                    provider: configured.provider.clone(),
                    model: configured.model.clone(),
                    credential_ref: configured.credential_ref.clone(),
                },
                fallbacks: configured
                    .fallbacks
                    .iter()
                    .map(|selection| RegistryModelSelection {
                        provider: selection.provider.clone(),
                        model: selection.model.clone(),
                        credential_ref: None,
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

    /// Resolves every profile, workspace, model, policy, and reply decision before session work.
    ///
    /// # Errors
    ///
    /// Returns visible errors for missing, ambiguous, disabled, unauthorized, or invalid routes.
    pub fn resolve(
        &self,
        request: &RouteRequest,
        now: UtcTimestamp,
    ) -> Result<ResolvedSessionRoute, RouteError> {
        validate_request(request)?;
        let profiles = self.profiles.list()?;
        let mut candidates = profiles
            .into_iter()
            .filter(|profile| {
                request
                    .profile_id
                    .as_ref()
                    .is_none_or(|id| &profile.profile.id == id)
                    && request
                        .workspace_id
                        .as_ref()
                        .is_none_or(|id| &profile.profile.workspace_id == id)
            })
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return Err(RouteError::Missing);
        }
        if candidates.len() != 1 {
            return Err(RouteError::Ambiguous);
        }
        let profile = candidates.pop().ok_or(RouteError::Missing)?;
        if !profile.enabled {
            return Err(RouteError::Disabled(profile.profile.id));
        }
        if !profile.authorized_callers.contains(&request.caller) {
            return Err(RouteError::Unauthorized(profile.profile.id));
        }
        if !profile.profile.channels.contains(&request.reply.channel) {
            return Err(RouteError::ChannelDisabled {
                profile: profile.profile.id,
                channel: request.reply.channel.clone(),
            });
        }
        let model_route = self
            .models
            .resolve(&profile.profile.id, ModelPurpose::Primary)?;
        ensure_model_route_matches(&profile, &model_route)?;
        let primary = model_route
            .candidates
            .first()
            .ok_or(RouteError::ModelRouteMismatch)?;
        let snapshot = ResolvedProfileSnapshot {
            profile,
            model: ResolvedModelConfiguration {
                provider: primary.selection.provider.clone(),
                model: primary.selection.model.clone(),
                credential_ref: primary.selection.credential_ref.clone(),
            },
            reply: request.reply.clone(),
            session_policy: request.session_policy.clone(),
            resolved_at: now,
        };
        Ok(ResolvedSessionRoute {
            snapshot,
            model_route,
        })
    }

    /// Resolves first and only then creates the durable root session with a pinned snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error without creating a session when any route decision fails.
    pub fn create_root(
        &self,
        request: &RouteRequest,
        new_session: NewRootSession,
    ) -> Result<(SessionManifest, ResolvedSessionRoute), RouteError> {
        let resolved = self.resolve(request, new_session.created_at)?;
        let metadata = snapshot_metadata(&resolved.snapshot)?;
        let manifest = self.sessions.create(NewSession {
            kind: SessionKind::Root,
            session_id: new_session.session_id,
            root_tree_id: new_session.root_tree_id,
            parent_session_id: None,
            profile_id: resolved.snapshot.profile.profile.id.clone(),
            workspace_id: resolved.snapshot.profile.profile.workspace_id.clone(),
            created_at: new_session.created_at,
            label: new_session.label,
            profile_snapshot: Some(metadata),
        })?;
        Ok((manifest, resolved))
    }

    /// Resolves the current authorized route before resume and keeps or deliberately refreshes the snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for route/session mismatch, missing snapshots, or stale updates.
    pub fn prepare_resume(
        &self,
        session_id: &SessionId,
        request: &RouteRequest,
        writer_identity: Option<WriterIdentity>,
        now: UtcTimestamp,
    ) -> Result<ResolvedProfileSnapshot, RouteError> {
        let resolved = self.resolve(request, now)?;
        let manifest = self.sessions.manifest(session_id)?;
        if manifest.profile_id != resolved.snapshot.profile.profile.id
            || manifest.workspace_id != resolved.snapshot.profile.profile.workspace_id
        {
            return Err(RouteError::SessionIdentityMismatch);
        }
        let pinned = manifest
            .profile_snapshot
            .ok_or(RouteError::MissingSnapshot)?;
        let pinned_snapshot: ResolvedProfileSnapshot =
            serde_json::from_value(pinned.snapshot.clone())?;
        match request.session_policy.profile_refresh {
            ProfileRefreshPolicy::KeepPinned => Ok(pinned_snapshot),
            ProfileRefreshPolicy::ApplyLatest => {
                let identity = writer_identity.ok_or_else(|| {
                    RouteError::Invalid(
                        "applying a profile update requires an explicit writer identity".into(),
                    )
                })?;
                let replacement = snapshot_metadata(&resolved.snapshot)?;
                let mut writer = self.sessions.acquire_writer(session_id, identity)?;
                writer.update_profile_snapshot(Some(&pinned.digest), replacement)?;
                Ok(resolved.snapshot)
            }
        }
    }
}

fn validate_request(request: &RouteRequest) -> Result<(), RouteError> {
    if request.profile_id.is_none() && request.workspace_id.is_none() {
        return Err(RouteError::Invalid(
            "a profile or workspace selector is required".into(),
        ));
    }
    if request.caller.trim().is_empty()
        || request.reply.channel.trim().is_empty()
        || request.reply.destination.trim().is_empty()
    {
        return Err(RouteError::Invalid(
            "caller, channel, and destination must be non-empty".into(),
        ));
    }
    Ok(())
}

fn ensure_model_route_matches(
    profile: &RegisteredProfile,
    resolved: &ResolvedRoute,
) -> Result<(), RouteError> {
    let configured = &profile.profile.model_route;
    let expected = std::iter::once((
        configured.provider.as_str(),
        configured.model.as_str(),
        configured.credential_ref.as_deref(),
    ))
    .chain(
        configured
            .fallbacks
            .iter()
            .map(|selection| (selection.provider.as_str(), selection.model.as_str(), None)),
    )
    .collect::<Vec<_>>();
    let actual = resolved
        .candidates
        .iter()
        .map(|candidate| {
            (
                candidate.selection.provider.as_str(),
                candidate.selection.model.as_str(),
                candidate.selection.credential_ref.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    if expected == actual {
        Ok(())
    } else {
        Err(RouteError::ModelRouteMismatch)
    }
}

fn snapshot_metadata(
    snapshot: &ResolvedProfileSnapshot,
) -> Result<ProfileSnapshotMetadata, RouteError> {
    Ok(ProfileSnapshotMetadata::new(
        snapshot.profile.profile.id.clone(),
        snapshot.profile.profile.workspace_id.clone(),
        snapshot.profile.revision,
        snapshot.resolved_at,
        serde_json::to_value(snapshot)?,
    )?)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::thread;

    use keith_agent_types::{
        CURRENT_SCHEMA_VERSION, EntityId, Generation, ProfileId, Revision, TimeZoneName, WorkerId,
    };
    use keith_profile::{
        AgentProfile, AutonomyMode, ModelRoute, NotificationSettings, ProfileAutonomy,
        ProfileResources, RefinementSettings, RegisteredProfile, ThinkingLevel, ToolPermission,
    };
    use keith_provider_adapters::{OpenAiProvider, ProviderHttpConfig};
    use keith_provider_core::{ModelProvider, ProviderCredential};
    use keith_state_store::EmbeddedStore;
    use tempfile::TempDir;

    use super::*;

    fn serve_models() -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buffer = [0_u8; 1_024];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut buffer).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
            }
            assert!(String::from_utf8_lossy(&request).starts_with("GET /v1/models "));
            let body = r#"{"data":[{"id":"model-a"},{"id":"model-b"}]}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            stream.flush().unwrap();
        });
        (format!("http://{address}"), handle)
    }

    fn registered(
        root: &TempDir,
        name: &str,
        model: &str,
        channel: &str,
        tool: &str,
        caller: &str,
    ) -> RegisteredProfile {
        fs::write(root.path().join("PERSONA.md"), format!("persona {name}")).unwrap();
        fs::write(root.path().join("USER.md"), format!("user {name}")).unwrap();
        fs::write(root.path().join("RULES.md"), format!("rules {name}")).unwrap();
        fs::create_dir(root.path().join("memory")).unwrap();
        fs::create_dir(root.path().join("schedules")).unwrap();
        RegisteredProfile {
            profile: AgentProfile {
                version: CURRENT_SCHEMA_VERSION,
                id: ProfileId::new(),
                display_name: name.into(),
                workspace_id: WorkspaceId::new(),
                persona_file: "PERSONA.md".into(),
                user_file: "USER.md".into(),
                rule_files: vec!["RULES.md".into()],
                model_route: ModelRoute {
                    provider: "openai".into(),
                    model: model.into(),
                    fallbacks: vec![],
                    credential_ref: Some(format!("credential-{name}")),
                },
                thinking: ThinkingLevel::High,
                tool_rules: BTreeMap::from([(tool.into(), ToolPermission::Allow)]),
                enabled_skills: vec![format!("skill-{name}")],
                enabled_mcp_servers: vec![format!("mcp-{name}")],
                enabled_plugins: vec![format!("plugin-{name}")],
                channels: vec![channel.into()],
                autonomy: ProfileAutonomy {
                    mode: AutonomyMode::Bounded,
                    max_children: 2,
                    max_depth: 2,
                    daily_token_budget: 10_000,
                },
                notifications: NotificationSettings {
                    quiet_hours_start: "22:00".into(),
                    quiet_hours_end: "08:00".into(),
                    time_zone: TimeZoneName::parse("Europe/Berlin").unwrap(),
                    daily_limit: 4,
                },
                refinement: RefinementSettings {
                    enabled: true,
                    require_confirmation: true,
                    editable_targets: BTreeSet::from([format!("persona-{name}")]),
                },
            },
            resources: ProfileResources {
                workspace_root: root.path().into(),
                memory_root: root.path().join("memory"),
                schedule_root: root.path().join("schedules"),
            },
            enabled: true,
            authorized_callers: BTreeSet::from([caller.into()]),
            revision: Revision::ZERO,
            updated_at: UtcTimestamp::UNIX_EPOCH,
        }
    }

    fn request(profile: &RegisteredProfile, caller: &str, channel: &str) -> RouteRequest {
        RouteRequest {
            profile_id: Some(profile.profile.id.clone()),
            workspace_id: Some(profile.profile.workspace_id.clone()),
            caller: caller.into(),
            reply: ReplyRoute {
                channel: channel.into(),
                destination: format!("destination-{caller}"),
            },
            session_policy: SessionPolicy {
                profile_refresh: ProfileRefreshPolicy::KeepPinned,
                memory_enabled: true,
                schedules_enabled: true,
            },
        }
    }

    fn writer_identity() -> WriterIdentity {
        WriterIdentity {
            worker_id: WorkerId::new(),
            owner_instance: EntityId::new(),
            generation: Generation::ZERO,
            acquired_at: UtcTimestamp::from_unix_millis(3),
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn cross_profile_routes_are_real_isolated_durable_and_fail_closed() {
        let (base_url, server) = serve_models();
        let models = ModelRegistry::new();
        let provider: Arc<dyn ModelProvider> =
            Arc::new(OpenAiProvider::new(ProviderHttpConfig::new(base_url).unwrap()).unwrap());
        models.register_provider(provider).unwrap();
        models
            .refresh_models("openai", &ProviderCredential::new("secret").unwrap())
            .unwrap();
        server.join().unwrap();

        let profiles = ProfileRegistry::new(EmbeddedStore::open_in_memory().unwrap());
        let workspace_a = TempDir::new().unwrap();
        let workspace_b = TempDir::new().unwrap();
        let profile_a = profiles
            .register(registered(
                &workspace_a,
                "a",
                "model-a",
                "terminal",
                "read",
                "caller-a",
            ))
            .unwrap();
        let profile_b = profiles
            .register(registered(
                &workspace_b,
                "b",
                "model-b",
                "webhook",
                "shell",
                "caller-b",
            ))
            .unwrap();
        let session_root = TempDir::new().unwrap();
        let sessions = SessionStore::open(session_root.path()).unwrap();
        let resolver = RouteResolver::new(&profiles, &models, &sessions);
        resolver.synchronize_model_route(&profile_a).unwrap();
        resolver.synchronize_model_route(&profile_b).unwrap();

        let resolved_a = resolver
            .resolve(
                &request(&profile_a, "caller-a", "terminal"),
                UtcTimestamp::UNIX_EPOCH,
            )
            .unwrap();
        let resolved_b = resolver
            .resolve(
                &request(&profile_b, "caller-b", "webhook"),
                UtcTimestamp::UNIX_EPOCH,
            )
            .unwrap();
        assert_eq!(resolved_a.snapshot.model.model, "model-a");
        assert_eq!(resolved_b.snapshot.model.model, "model-b");
        assert_ne!(
            resolved_a.snapshot.profile.resources.workspace_root,
            resolved_b.snapshot.profile.resources.workspace_root
        );
        assert_ne!(
            resolved_a.snapshot.profile.resources.memory_root,
            resolved_b.snapshot.profile.resources.memory_root
        );
        assert_ne!(
            resolved_a.snapshot.profile.resources.schedule_root,
            resolved_b.snapshot.profile.resources.schedule_root
        );
        assert!(
            resolved_a
                .snapshot
                .profile
                .profile
                .tool_rules
                .contains_key("read")
        );
        assert!(
            resolved_b
                .snapshot
                .profile
                .profile
                .tool_rules
                .contains_key("shell")
        );
        assert_eq!(resolved_a.snapshot.reply.channel, "terminal");
        assert_eq!(resolved_b.snapshot.reply.channel, "webhook");

        fs::write(
            resolved_a
                .snapshot
                .profile
                .resources
                .memory_root
                .join("entry"),
            "memory-a",
        )
        .unwrap();
        fs::write(
            resolved_b
                .snapshot
                .profile
                .resources
                .schedule_root
                .join("job"),
            "schedule-b",
        )
        .unwrap();
        assert!(
            !resolved_b
                .snapshot
                .profile
                .resources
                .memory_root
                .join("entry")
                .exists()
        );
        assert!(
            !resolved_a
                .snapshot
                .profile
                .resources
                .schedule_root
                .join("job")
                .exists()
        );

        let session_a = SessionId::new();
        let (manifest_a, _) = resolver
            .create_root(
                &request(&profile_a, "caller-a", "terminal"),
                NewRootSession {
                    session_id: session_a.clone(),
                    root_tree_id: RootTreeId::new(),
                    created_at: UtcTimestamp::from_unix_millis(1),
                    label: Some("a".into()),
                },
            )
            .unwrap();
        let session_b = SessionId::new();
        let (manifest_b, _) = resolver
            .create_root(
                &request(&profile_b, "caller-b", "webhook"),
                NewRootSession {
                    session_id: session_b,
                    root_tree_id: RootTreeId::new(),
                    created_at: UtcTimestamp::from_unix_millis(1),
                    label: Some("b".into()),
                },
            )
            .unwrap();
        assert_ne!(manifest_a.profile_id, manifest_b.profile_id);
        assert_ne!(manifest_a.workspace_id, manifest_b.workspace_id);
        assert!(manifest_a.profile_snapshot.is_some());
        assert!(manifest_b.profile_snapshot.is_some());

        let failed_session = SessionId::new();
        let mut unauthorized = request(&profile_a, "intruder", "terminal");
        assert!(matches!(
            resolver.create_root(
                &unauthorized,
                NewRootSession {
                    session_id: failed_session.clone(),
                    root_tree_id: RootTreeId::new(),
                    created_at: UtcTimestamp::from_unix_millis(2),
                    label: None,
                }
            ),
            Err(RouteError::Unauthorized(_))
        ));
        assert!(matches!(
            sessions.manifest(&failed_session),
            Err(SessionStoreError::NotFound(_))
        ));
        unauthorized.caller = "caller-a".into();
        unauthorized.reply.channel = "webhook".into();
        assert!(matches!(
            resolver.resolve(&unauthorized, UtcTimestamp::from_unix_millis(2)),
            Err(RouteError::ChannelDisabled { .. })
        ));
        let mut missing = request(&profile_a, "caller-a", "terminal");
        missing.profile_id = Some(ProfileId::new());
        assert!(matches!(
            resolver.resolve(&missing, UtcTimestamp::from_unix_millis(2)),
            Err(RouteError::Missing)
        ));

        let mut updated_a = profiles.get(&profile_a.profile.id).unwrap().unwrap();
        updated_a.profile.thinking = ThinkingLevel::Low;
        updated_a.updated_at = UtcTimestamp::from_unix_millis(2);
        let updated_a = profiles.update(updated_a, Revision::ZERO).unwrap();
        let pinned = resolver
            .prepare_resume(
                &session_a,
                &request(&updated_a, "caller-a", "terminal"),
                None,
                UtcTimestamp::from_unix_millis(2),
            )
            .unwrap();
        assert_eq!(pinned.profile.profile.thinking, ThinkingLevel::High);
        let mut refresh = request(&updated_a, "caller-a", "terminal");
        refresh.session_policy.profile_refresh = ProfileRefreshPolicy::ApplyLatest;
        let latest = resolver
            .prepare_resume(
                &session_a,
                &refresh,
                Some(writer_identity()),
                UtcTimestamp::from_unix_millis(3),
            )
            .unwrap();
        assert_eq!(latest.profile.profile.thinking, ThinkingLevel::Low);
        let stored: ResolvedProfileSnapshot = serde_json::from_value(
            sessions
                .manifest(&session_a)
                .unwrap()
                .profile_snapshot
                .unwrap()
                .snapshot,
        )
        .unwrap();
        assert_eq!(stored.profile.revision, Revision::new(1));

        let mut disabled_b = profiles.get(&profile_b.profile.id).unwrap().unwrap();
        disabled_b.enabled = false;
        disabled_b.updated_at = UtcTimestamp::from_unix_millis(4);
        let disabled_b = profiles.update(disabled_b, Revision::ZERO).unwrap();
        assert!(matches!(
            resolver.resolve(
                &request(&disabled_b, "caller-b", "webhook"),
                UtcTimestamp::from_unix_millis(4)
            ),
            Err(RouteError::Disabled(_))
        ));

        let mut ambiguous_profile = profile_a.clone();
        ambiguous_profile.profile.id = ProfileId::new();
        ambiguous_profile.profile.display_name = "ambiguous".into();
        ambiguous_profile.authorized_callers = BTreeSet::from(["caller-a".into()]);
        ambiguous_profile.revision = Revision::ZERO;
        let ambiguous_profile = profiles.register(ambiguous_profile).unwrap();
        resolver
            .synchronize_model_route(&ambiguous_profile)
            .unwrap();
        let mut ambiguous = request(&profile_a, "caller-a", "terminal");
        ambiguous.profile_id = None;
        assert!(matches!(
            resolver.resolve(&ambiguous, UtcTimestamp::from_unix_millis(5)),
            Err(RouteError::Ambiguous)
        ));
    }
}
