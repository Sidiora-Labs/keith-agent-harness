#![forbid(unsafe_code)]

mod events;
mod recovery;

pub use events::*;
pub use recovery::*;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use keith_agent_types::{
    CURRENT_PROTOCOL_VERSION, CURRENT_SCHEMA_VERSION, CommonError, EntityId, ErrorCode, ProfileId,
    Revision, RootTreeId, SchemaVersion, Sequence, SessionId, UtcTimestamp,
};
use keith_connection::{
    AgentTransport, FramedTransport, LocalStream, accept_local, bind_permissioned_local,
    set_local_listener_nonblocking, set_local_read_timeout,
};
use keith_protocol::{
    ClientCommand, CommandError, CommandResult, CommandResultEnvelope, Feature, ResponsePayload,
    SessionFilter, SessionSnapshot, SessionState, SessionSummary, WireFormat, WireMessage,
    negotiate,
};
use keith_supervisor::{
    SupervisorError, SupervisorOptions, WorkerEvent, WorkerStatus, WorkerSupervisor,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RootManifest {
    pub version: SchemaVersion,
    pub root_tree_id: RootTreeId,
    pub root_session_id: SessionId,
    pub profile_id: ProfileId,
    pub title: Option<String>,
    pub state: SessionState,
    pub updated_at: UtcTimestamp,
}

impl RootManifest {
    pub fn summary(&self) -> SessionSummary {
        SessionSummary {
            session_id: self.root_session_id.clone(),
            root_tree_id: self.root_tree_id.clone(),
            profile_id: self.profile_id.clone(),
            title: self.title.clone(),
            state: self.state,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RootCatalog {
    roots: BTreeMap<RootTreeId, RootManifest>,
    sessions: BTreeMap<SessionId, RootTreeId>,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("catalog I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("manifest {path} exceeds the metadata limit of {MAX_MANIFEST_BYTES} bytes")]
    ManifestTooLarge { path: PathBuf },
    #[error("manifest {path} is invalid: {source}")]
    InvalidManifest {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("manifest {path} uses unsupported schema {version}")]
    UnsupportedSchema {
        path: PathBuf,
        version: SchemaVersion,
    },
    #[error("manifest root {manifest} does not match directory root {directory}")]
    RootMismatch {
        manifest: RootTreeId,
        directory: RootTreeId,
    },
    #[error("duplicate root session ID {0}")]
    DuplicateSession(SessionId),
}

impl RootCatalog {
    /// Discovers root trees by reading only their bounded manifest files.
    ///
    /// Session journals, snapshots, and other root contents are deliberately never opened.
    ///
    /// # Errors
    ///
    /// Returns an error when catalog directories or manifests are malformed or unreadable.
    pub fn discover(data_root: &Path) -> Result<Self, CatalogError> {
        let sessions_directory = data_root.join("sessions");
        let entries = match fs::read_dir(&sessions_directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error.into()),
        };
        let mut catalog = Self::default();
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let directory_root: RootTreeId = match entry.file_name().to_string_lossy().parse() {
                Ok(root) => root,
                Err(_) => continue,
            };
            let path = entry.path().join("manifest.json");
            let metadata = match fs::metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error.into()),
            };
            if metadata.len() > MAX_MANIFEST_BYTES {
                return Err(CatalogError::ManifestTooLarge { path });
            }
            let bytes = fs::read(&path)?;
            let manifest: RootManifest =
                serde_json::from_slice(&bytes).map_err(|source| CatalogError::InvalidManifest {
                    path: path.clone(),
                    source,
                })?;
            if manifest.version != CURRENT_SCHEMA_VERSION {
                return Err(CatalogError::UnsupportedSchema {
                    path,
                    version: manifest.version,
                });
            }
            if manifest.root_tree_id != directory_root {
                return Err(CatalogError::RootMismatch {
                    manifest: manifest.root_tree_id,
                    directory: directory_root,
                });
            }
            if catalog
                .sessions
                .insert(
                    manifest.root_session_id.clone(),
                    manifest.root_tree_id.clone(),
                )
                .is_some()
            {
                return Err(CatalogError::DuplicateSession(manifest.root_session_id));
            }
            catalog
                .roots
                .insert(manifest.root_tree_id.clone(), manifest);
        }
        Ok(catalog)
    }

    pub fn len(&self) -> usize {
        self.roots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    pub fn root(&self, root_tree_id: &RootTreeId) -> Option<&RootManifest> {
        self.roots.get(root_tree_id)
    }

    pub fn root_for_session(&self, session_id: &SessionId) -> Option<&RootTreeId> {
        self.sessions.get(session_id)
    }

    pub fn list(&self, filter: &SessionFilter) -> Vec<SessionSummary> {
        self.roots
            .values()
            .filter(|manifest| {
                filter
                    .profile_id
                    .as_ref()
                    .is_none_or(|profile| profile == &manifest.profile_id)
            })
            .filter(|manifest| filter.include_archived || manifest.state != SessionState::Archived)
            .map(RootManifest::summary)
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct DaemonOptions {
    pub supervisor: SupervisorOptions,
    pub idle_evict_after: Duration,
    pub maintenance_interval: Duration,
    pub replay_capacity: usize,
    pub client_queue_capacity: usize,
    pub command_dedup_capacity: usize,
}

impl Default for DaemonOptions {
    fn default() -> Self {
        Self {
            supervisor: SupervisorOptions::default(),
            idle_evict_after: Duration::from_secs(15 * 60),
            maintenance_interval: Duration::from_millis(100),
            replay_capacity: 4_096,
            client_queue_capacity: 256,
            command_dedup_capacity: 4_096,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DaemonHealth {
    pub instance_id: EntityId,
    pub discovered_roots: usize,
    pub workers: Vec<WorkerStatus>,
    pub last_worker_events: Vec<WorkerEvent>,
    pub shutting_down: bool,
}

pub struct DaemonCore {
    instance_id: EntityId,
    catalog: RootCatalog,
    supervisor: WorkerSupervisor,
    options: DaemonOptions,
    last_worker_events: Vec<WorkerEvent>,
    event_hubs: BTreeMap<RootTreeId, EventHub>,
    command_ledger: CommandLedger,
    shutting_down: bool,
    startup_recovery: StartupRecoveryReport,
}

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Supervisor(#[from] SupervisorError),
    #[error("daemon endpoint failed: {0}")]
    Connection(#[from] keith_connection::ConnectionError),
    #[error("daemon I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("daemon state lock was poisoned")]
    LockPoisoned,
    #[error("session {0} is not in the root catalog")]
    UnknownSession(SessionId),
    #[error("root {0} is not in the catalog")]
    UnknownRoot(RootTreeId),
    #[error(transparent)]
    EventStream(#[from] EventStreamError),
    #[error(transparent)]
    Recovery(#[from] RecoveryError),
}

impl DaemonCore {
    /// Opens the daemon catalog and adopts live workers without activating dormant roots.
    ///
    /// # Errors
    ///
    /// Returns an error when catalog discovery or worker adoption fails.
    pub fn open(
        data_root: impl Into<PathBuf>,
        worker_executable: impl Into<PathBuf>,
        options: DaemonOptions,
    ) -> Result<Self, DaemonError> {
        let data_root = data_root.into();
        fs::create_dir_all(&data_root)?;
        let (catalog, startup_recovery) = recover_daemon_startup(&data_root)?;
        let mut supervisor = WorkerSupervisor::open(
            data_root.join("runtime"),
            worker_executable,
            options.supervisor.clone(),
        )?;
        supervisor.adopt_existing()?;
        let command_ledger = CommandLedger::new(options.command_dedup_capacity)?;
        Ok(Self {
            instance_id: EntityId::new(),
            catalog,
            supervisor,
            options,
            last_worker_events: Vec::new(),
            event_hubs: BTreeMap::new(),
            command_ledger,
            shutting_down: false,
            startup_recovery,
        })
    }

    pub fn catalog(&self) -> &RootCatalog {
        &self.catalog
    }

    pub fn startup_recovery(&self) -> &StartupRecoveryReport {
        &self.startup_recovery
    }

    pub fn health(&self) -> DaemonHealth {
        DaemonHealth {
            instance_id: self.instance_id.clone(),
            discovered_roots: self.catalog.len(),
            workers: self.supervisor.statuses(),
            last_worker_events: self.last_worker_events.clone(),
            shutting_down: self.shutting_down,
        }
    }

    pub fn event_hub(&self, root_tree_id: &RootTreeId) -> Option<&EventHub> {
        self.event_hubs.get(root_tree_id)
    }

    pub fn event_hub_mut(&mut self, root_tree_id: &RootTreeId) -> Option<&mut EventHub> {
        self.event_hubs.get_mut(root_tree_id)
    }

    /// Lazily activates the worker that owns a cataloged root session.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is unknown or its worker cannot start.
    pub fn activate_session(
        &mut self,
        session_id: &SessionId,
    ) -> Result<WorkerStatus, DaemonError> {
        let root = self
            .catalog
            .root_for_session(session_id)
            .cloned()
            .ok_or_else(|| DaemonError::UnknownSession(session_id.clone()))?;
        let status = if self.supervisor.mark_activity(&root) {
            self.supervisor
                .status(&root)
                .ok_or_else(|| DaemonError::UnknownSession(session_id.clone()))?
        } else {
            self.supervisor.restart(&root)?
        };
        self.ensure_event_hub(&root, status.generation)?;
        Ok(status)
    }

    fn ensure_event_hub(
        &mut self,
        root_tree_id: &RootTreeId,
        generation: keith_agent_types::Generation,
    ) -> Result<(), DaemonError> {
        let manifest = self
            .catalog
            .root(root_tree_id)
            .ok_or_else(|| DaemonError::UnknownRoot(root_tree_id.clone()))?;
        let snapshot = SessionSnapshot {
            session: manifest.summary(),
            generation,
            through_sequence: Sequence::ZERO,
            active_action: None,
            actions: Vec::new(),
            messages: Vec::new(),
            goals: Vec::new(),
            plans: Vec::new(),
            children: Vec::new(),
            kernels: Vec::new(),
            commitments: Vec::new(),
            schedules: Vec::new(),
            tools: Vec::new(),
            confirmations: Vec::new(),
            waits: Vec::new(),
            deliveries: Vec::new(),
            memory_changes: Vec::new(),
            usage: keith_protocol::UsageProjection::default(),
            presence: keith_protocol::PresenceProjection {
                session_id: manifest.root_session_id.clone(),
                goal_id: None,
                state: keith_protocol::PresenceState::Available,
                updated_at: manifest.updated_at,
                next_wake: None,
                safe_error: None,
            },
            revision: Revision::ZERO,
        };
        match self.event_hubs.get_mut(root_tree_id) {
            Some(hub) if hub.generation() != generation => {
                hub.replace_generation(generation, snapshot)?;
            }
            Some(_) => {}
            None => {
                self.event_hubs.insert(
                    root_tree_id.clone(),
                    EventHub::new(
                        root_tree_id.clone(),
                        generation,
                        snapshot,
                        self.options.replay_capacity,
                        self.options.client_queue_capacity,
                    )?,
                );
            }
        }
        Ok(())
    }

    /// Runs worker monitoring and idle eviction once.
    ///
    /// # Errors
    ///
    /// Returns an error when worker inspection or eviction fails.
    pub fn maintain(&mut self) -> Result<(), DaemonError> {
        self.last_worker_events = self.supervisor.monitor()?;
        self.supervisor.evict_idle(self.options.idle_evict_after)?;
        Ok(())
    }

    /// Drains all active and adopted workers.
    ///
    /// # Errors
    ///
    /// Returns an error when a worker cannot be drained or forcibly terminated.
    pub fn shutdown(&mut self) -> Result<(), DaemonError> {
        self.shutting_down = true;
        self.supervisor.drain_all().map_err(DaemonError::from)
    }

    /// Serves the permission-restricted local `AgentConnection` endpoint until shutdown is signaled.
    ///
    /// # Errors
    ///
    /// Returns an error when the endpoint, protocol journey, or maintenance loop fails.
    pub fn serve_local(
        &mut self,
        socket_path: &Path,
        shutdown: &AtomicBool,
    ) -> Result<(), DaemonError> {
        if let Some(parent) = socket_path.parent() {
            fs::create_dir_all(parent)?;
        }
        match fs::remove_file(socket_path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let listener = bind_permissioned_local(socket_path)?;
        set_local_listener_nonblocking(&listener, true)?;
        {
            let shared = Mutex::new(&mut *self);
            thread::scope(|scope| -> Result<(), DaemonError> {
                while !shutdown.load(Ordering::Acquire) {
                    match accept_local(&listener) {
                        Ok(stream) => {
                            let shared = &shared;
                            scope.spawn(move || {
                                let _ = Self::serve_shared_connection(shared, stream, shutdown);
                            });
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            let interval = {
                                let mut daemon =
                                    shared.lock().map_err(|_| DaemonError::LockPoisoned)?;
                                daemon.maintain()?;
                                daemon.options.maintenance_interval
                            };
                            thread::sleep(interval);
                        }
                        Err(error) => return Err(error.into()),
                    }
                }
                Ok(())
            })?;
        }
        let result = self.shutdown();
        drop(listener);
        match fs::remove_file(socket_path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) if result.is_ok() => return Err(error.into()),
            Ok(()) | Err(_) => {}
        }
        result
    }

    fn serve_shared_connection(
        shared: &Mutex<&mut Self>,
        stream: LocalStream,
        shutdown: &AtomicBool,
    ) -> Result<(), DaemonError> {
        let maintenance_interval = shared
            .lock()
            .map_err(|_| DaemonError::LockPoisoned)?
            .options
            .maintenance_interval;
        set_local_read_timeout(&stream, Some(maintenance_interval))?;
        let mut transport = FramedTransport::new(stream, WireFormat::Json);
        let WireMessage::ClientHello(client) = transport.receive()? else {
            return Ok(());
        };
        let connected_client_id = client.client_id.clone();
        let features = BTreeSet::from([
            Feature::SessionLifecycle,
            Feature::FramedJson,
            Feature::Replay,
            Feature::Snapshots,
        ]);
        let hello = negotiate(
            &client,
            CURRENT_PROTOCOL_VERSION,
            shared
                .lock()
                .map_err(|_| DaemonError::LockPoisoned)?
                .instance_id
                .clone(),
            &features,
        )
        .map_err(keith_connection::ConnectionError::from)?;
        let negotiated = hello.protocol;
        transport.send(&WireMessage::ServerHello(hello))?;
        while !shutdown.load(Ordering::Acquire) {
            let message = match transport.receive() {
                Ok(message) => message,
                Err(error) if error.is_timed_out() => {
                    let events = {
                        let mut daemon = shared.lock().map_err(|_| DaemonError::LockPoisoned)?;
                        daemon.maintain()?;
                        daemon.drain_client_events(&connected_client_id)?
                    };
                    for event in events {
                        transport.send(&WireMessage::Event(event))?;
                    }
                    continue;
                }
                Err(error) if error.is_interrupted() => {
                    if shutdown.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    continue;
                }
                Err(keith_connection::ConnectionError::Closed) => return Ok(()),
                Err(error) => return Err(error.into()),
            };
            let WireMessage::Command(command) = message else {
                continue;
            };
            let (result, recovery_events) = {
                let mut daemon = shared.lock().map_err(|_| DaemonError::LockPoisoned)?;
                daemon.handle_command(&connected_client_id, negotiated, command)
            };
            for event in recovery_events {
                transport.send(&WireMessage::Event(event))?;
            }
            transport.send(&WireMessage::CommandResult(result))?;
        }
        Ok(())
    }

    fn handle_command(
        &mut self,
        connected_client_id: &keith_agent_types::ClientId,
        negotiated: keith_agent_types::ProtocolVersion,
        command: keith_protocol::CommandEnvelope,
    ) -> (CommandResultEnvelope, Vec<keith_protocol::EventEnvelope>) {
        let mut recovery_events = Vec::new();
        let result = if command.client_id != *connected_client_id {
            CommandResultEnvelope {
                protocol: negotiated,
                command_id: command.command_id,
                completed_at: UtcTimestamp::now().unwrap_or(UtcTimestamp::UNIX_EPOCH),
                result: CommandResult::Rejected(CommandError {
                    error: CommonError::new(
                        ErrorCode::Unauthorized,
                        "command client ID does not match the connection",
                        false,
                    ),
                    unsupported_feature: None,
                }),
            }
        } else if let Some(result) = self.command_ledger.result(&command.command_id) {
            result.clone()
        } else {
            let result = if command.protocol.major != negotiated.major
                || command.protocol.minor > negotiated.minor
            {
                CommandResult::Rejected(CommandError {
                    error: CommonError::new(
                        ErrorCode::UnsupportedVersion,
                        "command envelope exceeds the negotiated protocol",
                        false,
                    ),
                    unsupported_feature: None,
                })
            } else {
                let (result, events) = self.execute_command(connected_client_id, command.command);
                recovery_events = events;
                result
            };
            let envelope = CommandResultEnvelope {
                protocol: negotiated,
                command_id: command.command_id,
                completed_at: UtcTimestamp::now().unwrap_or(UtcTimestamp::UNIX_EPOCH),
                result,
            };
            self.command_ledger.record(envelope.clone());
            envelope
        };
        (result, recovery_events)
    }

    fn drain_client_events(
        &mut self,
        client_id: &keith_agent_types::ClientId,
    ) -> Result<Vec<keith_protocol::EventEnvelope>, DaemonError> {
        let mut events = Vec::new();
        let mut remaining = self.options.client_queue_capacity;
        for hub in self.event_hubs.values_mut() {
            if remaining == 0 {
                break;
            }
            match hub.poll(client_id, remaining) {
                Ok(mut pending) => {
                    remaining = remaining.saturating_sub(pending.len());
                    events.append(&mut pending);
                }
                Err(EventStreamError::UnknownClient(_)) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(events)
    }

    fn execute_command(
        &mut self,
        client_id: &keith_agent_types::ClientId,
        command: ClientCommand,
    ) -> (CommandResult, Vec<keith_protocol::EventEnvelope>) {
        match command {
            ClientCommand::ListSessions(filter) => (
                CommandResult::Data(Box::new(ResponsePayload::Sessions(
                    self.catalog.list(&filter),
                ))),
                Vec::new(),
            ),
            ClientCommand::AttachSession(attach) => {
                match self.activate_and_attach(client_id, &attach) {
                    Ok(recovery) => (
                        recovery.snapshot.map_or(
                            CommandResult::Accepted { action_id: None },
                            |snapshot| {
                                CommandResult::Data(Box::new(ResponsePayload::Snapshot(Box::new(
                                    snapshot,
                                ))))
                            },
                        ),
                        recovery.events,
                    ),
                    Err(error) => (
                        CommandResult::Rejected(CommandError {
                            error: CommonError::new(ErrorCode::NotFound, error.to_string(), false),
                            unsupported_feature: None,
                        }),
                        Vec::new(),
                    ),
                }
            }
            ClientCommand::DetachSession { session_id } => {
                if let Some(root) = self.catalog.root_for_session(&session_id)
                    && let Some(hub) = self.event_hubs.get_mut(root)
                {
                    hub.detach(client_id);
                }
                (CommandResult::Accepted { action_id: None }, Vec::new())
            }
            ClientCommand::AcknowledgeEvents(acknowledgement) => {
                let result = self
                    .event_hubs
                    .get_mut(&acknowledgement.root_tree_id)
                    .ok_or_else(|| EventStreamError::UnknownClient(client_id.clone()))
                    .and_then(|hub| {
                        hub.acknowledge(
                            client_id,
                            acknowledgement.generation,
                            acknowledgement.through_sequence,
                        )
                    });
                match result {
                    Ok(()) => (CommandResult::Accepted { action_id: None }, Vec::new()),
                    Err(error) => (
                        CommandResult::Rejected(CommandError {
                            error: CommonError::new(ErrorCode::Conflict, error.to_string(), false),
                            unsupported_feature: None,
                        }),
                        Vec::new(),
                    ),
                }
            }
            _ => (
                CommandResult::Rejected(CommandError {
                    error: CommonError::new(
                        ErrorCode::Unavailable,
                        "command requires a session worker service that is not active on the daemon",
                        true,
                    ),
                    unsupported_feature: None,
                }),
                Vec::new(),
            ),
        }
    }

    fn activate_and_attach(
        &mut self,
        client_id: &keith_agent_types::ClientId,
        attach: &keith_protocol::AttachSession,
    ) -> Result<RecoveryBatch, DaemonError> {
        self.activate_session(&attach.session_id)?;
        let root = self
            .catalog
            .root_for_session(&attach.session_id)
            .cloned()
            .ok_or_else(|| DaemonError::UnknownSession(attach.session_id.clone()))?;
        let hub = self
            .event_hubs
            .get_mut(&root)
            .ok_or(DaemonError::UnknownRoot(root))?;
        Ok(hub.attach(client_id.clone(), attach.resume.as_ref()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest(root: RootTreeId, session: SessionId) -> RootManifest {
        RootManifest {
            version: CURRENT_SCHEMA_VERSION,
            root_tree_id: root,
            root_session_id: session,
            profile_id: ProfileId::new(),
            title: Some("catalog entry".into()),
            state: SessionState::Dormant,
            updated_at: UtcTimestamp::UNIX_EPOCH,
        }
    }

    #[test]
    fn discovery_reads_bounded_metadata_and_ignores_session_contents() {
        let directory = tempfile::tempdir().unwrap();
        let root = RootTreeId::new();
        let session = SessionId::new();
        let root_directory = directory.path().join("sessions").join(root.to_string());
        fs::create_dir_all(&root_directory).unwrap();
        fs::write(
            root_directory.join("manifest.json"),
            keith_agent_types::canonical_json_bytes(&manifest(root.clone(), session.clone()))
                .unwrap(),
        )
        .unwrap();
        fs::write(
            root_directory.join("session.jsonl"),
            b"this is deliberately corrupt and must not be loaded",
        )
        .unwrap();

        let catalog = RootCatalog::discover(directory.path()).unwrap();
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog.root_for_session(&session), Some(&root));
        assert_eq!(
            catalog.list(&SessionFilter::default())[0].session_id,
            session
        );
    }

    #[test]
    fn oversized_manifest_is_rejected_before_reading_it() {
        let directory = tempfile::tempdir().unwrap();
        let root = RootTreeId::new();
        let root_directory = directory.path().join("sessions").join(root.to_string());
        fs::create_dir_all(&root_directory).unwrap();
        fs::write(
            root_directory.join("manifest.json"),
            vec![b' '; usize::try_from(MAX_MANIFEST_BYTES + 1).unwrap()],
        )
        .unwrap();
        assert!(matches!(
            RootCatalog::discover(directory.path()),
            Err(CatalogError::ManifestTooLarge { .. })
        ));
    }
}
