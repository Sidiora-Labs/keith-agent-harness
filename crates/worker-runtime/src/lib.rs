#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use fs2::FileExt;
use keith_agent_types::{
    CURRENT_SCHEMA_VERSION, EntityId, Generation, Revision, RootTreeId, SchemaVersion,
    UtcTimestamp, WorkerId,
};
use keith_connection::bind_permissioned_local;
use keith_framing::{FrameError, LengthDelimitedCodec};
use keith_state_store::{EmbeddedStore, FileBackupHook, StoreError};
use keith_state_store_core::{
    AtomicStateRepository, Collection, RecordMutation, VersionedRecord, WritePrecondition,
};
use serde::{Deserialize, Serialize};
use signal_hook::consts::{SIGINT, SIGTERM};
use thiserror::Error;

pub const PRIVATE_MAX_FRAME_BYTES: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerRunState {
    Starting,
    Ready,
    Draining,
    Stopped,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkerRegistration {
    pub version: SchemaVersion,
    pub worker_id: WorkerId,
    pub root_tree_id: RootTreeId,
    pub generation: Generation,
    pub pid: u32,
    pub control_socket: PathBuf,
    pub started_at: UtcTimestamp,
    pub heartbeat_at: UtcTimestamp,
    pub state: WorkerRunState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LeaseGrant {
    pub root_tree_id: RootTreeId,
    pub worker_id: WorkerId,
    pub generation: Generation,
    pub authentication: EntityId,
    pub expires_at: UtcTimestamp,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "message", content = "payload")]
pub enum PrivateMessage {
    SupervisorHello,
    Ready { pid: u32 },
    Heartbeat { at: UtcTimestamp },
    Idle { since: UtcTimestamp },
    Fatal { reason: String },
    Shutdown { deadline: UtcTimestamp },
    ShutdownAck,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrivateEnvelope {
    version: SchemaVersion,
    root_tree_id: RootTreeId,
    worker_id: WorkerId,
    generation: Generation,
    authentication: EntityId,
    body: PrivateMessage,
}

pub struct PrivateTransport<S> {
    stream: S,
    grant: LeaseGrant,
    framing: LengthDelimitedCodec,
}

impl<S> PrivateTransport<S> {
    /// # Errors
    ///
    /// Returns an error if the private framing bound is invalid.
    pub fn new(stream: S, grant: LeaseGrant) -> Result<Self, PrivateProtocolError> {
        Ok(Self {
            stream,
            grant,
            framing: LengthDelimitedCodec::new(PRIVATE_MAX_FRAME_BYTES)?,
        })
    }

    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S: Write> PrivateTransport<S> {
    /// # Errors
    ///
    /// Returns an error when the authenticated frame cannot be serialized or written.
    pub fn send(&mut self, body: PrivateMessage) -> Result<(), PrivateProtocolError> {
        let envelope = PrivateEnvelope {
            version: CURRENT_SCHEMA_VERSION,
            root_tree_id: self.grant.root_tree_id.clone(),
            worker_id: self.grant.worker_id.clone(),
            generation: self.grant.generation,
            authentication: self.grant.authentication.clone(),
            body,
        };
        let bytes = keith_agent_types::canonical_json_bytes(&envelope)?;
        self.framing.write_frame(&mut self.stream, &bytes)?;
        Ok(())
    }
}

impl<S: Read> PrivateTransport<S> {
    /// # Errors
    ///
    /// Returns an error for closed, malformed, unauthenticated, or stale-generation frames.
    pub fn receive(&mut self) -> Result<PrivateMessage, PrivateProtocolError> {
        let bytes = self
            .framing
            .read_frame(&mut self.stream)?
            .ok_or(PrivateProtocolError::Closed)?;
        let envelope: PrivateEnvelope = serde_json::from_slice(&bytes)?;
        if envelope.version != CURRENT_SCHEMA_VERSION
            || envelope.root_tree_id != self.grant.root_tree_id
            || envelope.worker_id != self.grant.worker_id
            || envelope.generation != self.grant.generation
        {
            return Err(PrivateProtocolError::StaleRoute);
        }
        if envelope.authentication != self.grant.authentication {
            return Err(PrivateProtocolError::Unauthenticated);
        }
        Ok(envelope.body)
    }
}

#[derive(Debug, Error)]
pub enum PrivateProtocolError {
    #[error(transparent)]
    Frame(#[from] FrameError),
    #[error("private message serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("private connection closed")]
    Closed,
    #[error("private message authentication failed")]
    Unauthenticated,
    #[error("private message targets a stale worker generation")]
    StaleRoute,
}

impl PrivateProtocolError {
    pub fn is_retryable_io(&self) -> bool {
        matches!(
            self,
            Self::Frame(FrameError::Io(error))
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock
                        | std::io::ErrorKind::TimedOut
                        | std::io::ErrorKind::Interrupted
                )
        )
    }

    pub fn is_connection_loss(&self) -> bool {
        matches!(self, Self::Closed)
            || matches!(
                self,
                Self::Frame(FrameError::Io(error))
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::BrokenPipe
                            | std::io::ErrorKind::ConnectionAborted
                            | std::io::ErrorKind::ConnectionReset
                            | std::io::ErrorKind::NotConnected
                            | std::io::ErrorKind::UnexpectedEof
                    )
            )
    }
}

pub struct LeaseManager {
    store: EmbeddedStore,
}

#[derive(Debug, Error)]
pub enum LeaseError {
    #[error("lease state failed: {0}")]
    Store(#[from] StoreError),
    #[error("lease payload failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("root {0} already has an unexpired lease")]
    Active(RootTreeId),
    #[error("lease claim for root {0} lost a transaction race")]
    Contended(RootTreeId),
    #[error("worker no longer owns the lease for root {0}")]
    OwnershipLost(RootTreeId),
    #[error("generation overflow for root {0}")]
    GenerationOverflow(RootTreeId),
    #[error("lease record revision overflow")]
    RevisionOverflow,
    #[error("clock failed: {0}")]
    Clock(#[from] keith_agent_types::TimestampError),
}

impl LeaseManager {
    /// # Errors
    ///
    /// Returns an error when the durable lease database cannot be opened or migrated.
    pub fn open(path: &Path) -> Result<Self, LeaseError> {
        Ok(Self {
            store: EmbeddedStore::open(path, Some(&FileBackupHook))?,
        })
    }

    /// Transactionally claims an expired or absent root and advances its generation.
    ///
    /// # Errors
    ///
    /// Returns an error for a live owner, contention, generation overflow, or store failure.
    pub fn claim(
        &self,
        root_tree_id: &RootTreeId,
        worker_id: WorkerId,
        lease_duration: Duration,
    ) -> Result<LeaseGrant, LeaseError> {
        self.claim_at(
            root_tree_id,
            worker_id,
            UtcTimestamp::now()?,
            lease_duration,
        )
    }

    /// Equivalent to [`Self::claim`] with an explicit time for deterministic recovery tests.
    ///
    /// # Errors
    ///
    /// Returns an error for a live owner, contention, generation overflow, or store failure.
    pub fn claim_at(
        &self,
        root_tree_id: &RootTreeId,
        worker_id: WorkerId,
        now: UtcTimestamp,
        lease_duration: Duration,
    ) -> Result<LeaseGrant, LeaseError> {
        let id = root_tree_id.as_entity_id();
        let lease_record = self.store.get_record(Collection::WorkerLeases, id)?;
        let current_lease = lease_record
            .as_ref()
            .map(|record| serde_json::from_value::<LeaseGrant>(record.payload.clone()))
            .transpose()?;
        if current_lease
            .as_ref()
            .is_some_and(|lease| lease.expires_at > now)
        {
            return Err(LeaseError::Active(root_tree_id.clone()));
        }
        let generation_record = self.store.get_record(Collection::WorkerGenerations, id)?;
        let persisted_generation = generation_record
            .as_ref()
            .map(|record| serde_json::from_value::<Generation>(record.payload.clone()))
            .transpose()?
            .unwrap_or(Generation::ZERO);
        let current_generation = current_lease
            .as_ref()
            .map_or(persisted_generation, |lease| {
                persisted_generation.max(lease.generation)
            });
        let generation = current_generation
            .checked_next()
            .ok_or_else(|| LeaseError::GenerationOverflow(root_tree_id.clone()))?;
        let expires_at = add_duration(now, lease_duration)
            .ok_or_else(|| LeaseError::GenerationOverflow(root_tree_id.clone()))?;
        let grant = LeaseGrant {
            root_tree_id: root_tree_id.clone(),
            worker_id,
            generation,
            authentication: EntityId::new(),
            expires_at,
        };
        let mutations = [
            RecordMutation::Put {
                collection: Collection::WorkerGenerations,
                record: versioned_record(
                    id.clone(),
                    next_revision(generation_record.as_ref())?,
                    now,
                    serde_json::to_value(generation)?,
                ),
                precondition: precondition(generation_record.as_ref()),
            },
            RecordMutation::Put {
                collection: Collection::WorkerLeases,
                record: versioned_record(
                    id.clone(),
                    next_revision(lease_record.as_ref())?,
                    now,
                    serde_json::to_value(&grant)?,
                ),
                precondition: precondition(lease_record.as_ref()),
            },
        ];
        match self.store.transact(&mutations) {
            Ok(_) => Ok(grant),
            Err(StoreError::Conflict { .. }) => Err(LeaseError::Contended(root_tree_id.clone())),
            Err(error) => Err(error.into()),
        }
    }

    /// Renews the caller's lease only if its complete ownership tuple still matches.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::OwnershipLost`] for stale workers.
    pub fn renew(
        &self,
        grant: &LeaseGrant,
        lease_duration: Duration,
    ) -> Result<LeaseGrant, LeaseError> {
        self.renew_at(grant, UtcTimestamp::now()?, lease_duration)
    }

    /// Equivalent to [`Self::renew`] with an explicit time.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::OwnershipLost`] for expired or stale workers.
    pub fn renew_at(
        &self,
        grant: &LeaseGrant,
        now: UtcTimestamp,
        lease_duration: Duration,
    ) -> Result<LeaseGrant, LeaseError> {
        let id = grant.root_tree_id.as_entity_id();
        let Some(record) = self.store.get_record(Collection::WorkerLeases, id)? else {
            return Err(LeaseError::OwnershipLost(grant.root_tree_id.clone()));
        };
        let current: LeaseGrant = serde_json::from_value(record.payload.clone())?;
        if !same_owner(&current, grant) || current.expires_at <= now {
            return Err(LeaseError::OwnershipLost(grant.root_tree_id.clone()));
        }
        let mut renewed = current;
        renewed.expires_at = add_duration(now, lease_duration)
            .ok_or_else(|| LeaseError::GenerationOverflow(grant.root_tree_id.clone()))?;
        let revision = record
            .revision
            .checked_next()
            .ok_or_else(|| LeaseError::GenerationOverflow(grant.root_tree_id.clone()))?;
        match self.store.transact(&[RecordMutation::Put {
            collection: Collection::WorkerLeases,
            record: versioned_record(id.clone(), revision, now, serde_json::to_value(&renewed)?),
            precondition: WritePrecondition::Exact(record.revision),
        }]) {
            Ok(_) => Ok(renewed),
            Err(StoreError::Conflict { .. }) => {
                Err(LeaseError::OwnershipLost(grant.root_tree_id.clone()))
            }
            Err(error) => Err(error.into()),
        }
    }

    /// Reads an unexpired lease for supervisor adoption.
    ///
    /// # Errors
    ///
    /// Returns an error when lease state cannot be read or decoded.
    pub fn current(&self, root_tree_id: &RootTreeId) -> Result<Option<LeaseGrant>, LeaseError> {
        let now = UtcTimestamp::now()?;
        let lease = self
            .store
            .get_record(Collection::WorkerLeases, root_tree_id.as_entity_id())?
            .map(|record| serde_json::from_value::<LeaseGrant>(record.payload))
            .transpose()?;
        Ok(lease.filter(|grant| grant.expires_at > now))
    }

    /// Confirms the complete ownership tuple and expiry.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::OwnershipLost`] for expired or stale grants.
    pub fn validate(&self, grant: &LeaseGrant) -> Result<(), LeaseError> {
        let now = UtcTimestamp::now()?;
        let current = self.current(&grant.root_tree_id)?;
        if current
            .as_ref()
            .is_some_and(|current| same_owner(current, grant) && current.expires_at > now)
        {
            Ok(())
        } else {
            Err(LeaseError::OwnershipLost(grant.root_tree_id.clone()))
        }
    }

    /// Releases a lease only if the caller remains its owner.
    ///
    /// # Errors
    ///
    /// Returns [`LeaseError::OwnershipLost`] for stale grants.
    pub fn release(&self, grant: &LeaseGrant) -> Result<(), LeaseError> {
        let id = grant.root_tree_id.as_entity_id();
        let Some(record) = self.store.get_record(Collection::WorkerLeases, id)? else {
            return Err(LeaseError::OwnershipLost(grant.root_tree_id.clone()));
        };
        let current: LeaseGrant = serde_json::from_value(record.payload)?;
        if !same_owner(&current, grant) {
            return Err(LeaseError::OwnershipLost(grant.root_tree_id.clone()));
        }
        match self.store.transact(&[RecordMutation::Delete {
            collection: Collection::WorkerLeases,
            id: id.clone(),
            precondition: WritePrecondition::Exact(record.revision),
        }]) {
            Ok(_) => Ok(()),
            Err(StoreError::Conflict { .. }) => {
                Err(LeaseError::OwnershipLost(grant.root_tree_id.clone()))
            }
            Err(error) => Err(error.into()),
        }
    }
}

fn same_owner(left: &LeaseGrant, right: &LeaseGrant) -> bool {
    left.root_tree_id == right.root_tree_id
        && left.worker_id == right.worker_id
        && left.generation == right.generation
        && left.authentication == right.authentication
}

fn precondition(record: Option<&VersionedRecord>) -> WritePrecondition {
    record.map_or(WritePrecondition::Missing, |record| {
        WritePrecondition::Exact(record.revision)
    })
}

fn next_revision(record: Option<&VersionedRecord>) -> Result<Revision, LeaseError> {
    record
        .map_or(Some(Revision::new(1)), |record| {
            record.revision.checked_next()
        })
        .ok_or(LeaseError::RevisionOverflow)
}

fn versioned_record(
    id: EntityId,
    revision: Revision,
    updated_at: UtcTimestamp,
    payload: serde_json::Value,
) -> VersionedRecord {
    VersionedRecord {
        version: CURRENT_SCHEMA_VERSION,
        id,
        revision,
        updated_at,
        payload,
    }
}

fn add_duration(timestamp: UtcTimestamp, duration: Duration) -> Option<UtcTimestamp> {
    i64::try_from(duration.as_millis())
        .ok()
        .and_then(|millis| timestamp.unix_millis().checked_add(millis))
        .map(UtcTimestamp::from_unix_millis)
}

pub struct WriterGuard {
    _file: File,
}

impl WriterGuard {
    /// Acquires a process-scoped advisory writer lock for the root tree.
    ///
    /// # Errors
    ///
    /// Returns an error when the lock path is inaccessible or another writer owns the guard.
    pub fn acquire(state_dir: &Path, grant: &LeaseGrant) -> Result<Self, WorkerRuntimeError> {
        let directory = state_dir.join("locks");
        fs::create_dir_all(&directory)?;
        let path = directory.join(format!("{}.lock", grant.root_tree_id));
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        file.try_lock_exclusive()
            .map_err(|_| WorkerRuntimeError::WriterLocked(grant.root_tree_id.clone()))?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(grant.authentication.as_str().as_bytes())?;
        file.sync_all()?;
        Ok(Self { _file: file })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerArguments {
    pub state_dir: PathBuf,
    pub lease_database: PathBuf,
    pub control_socket: PathBuf,
    pub grant: LeaseGrant,
    pub heartbeat_interval: Duration,
    pub lease_duration: Duration,
}

impl WorkerArguments {
    /// # Errors
    ///
    /// Returns an error when a required argument is missing or invalid.
    pub fn parse<I, S>(arguments: I) -> Result<Self, WorkerRuntimeError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut arguments = arguments.into_iter().map(Into::into);
        let _program = arguments.next();
        let mut values = std::collections::BTreeMap::new();
        while let Some(argument) = arguments.next() {
            let argument = argument
                .into_string()
                .map_err(|_| WorkerRuntimeError::InvalidArgument("non-UTF-8 argument".into()))?;
            let value = arguments.next().ok_or_else(|| {
                WorkerRuntimeError::InvalidArgument(format!("missing value for {argument}"))
            })?;
            let value = value
                .into_string()
                .map_err(|_| WorkerRuntimeError::InvalidArgument("non-UTF-8 value".into()))?;
            if !matches!(
                argument.as_str(),
                "--state-dir"
                    | "--lease-db"
                    | "--control-socket"
                    | "--root-tree"
                    | "--worker-id"
                    | "--generation"
                    | "--authentication"
                    | "--expires-at"
                    | "--heartbeat-ms"
                    | "--lease-ms"
            ) {
                return Err(WorkerRuntimeError::InvalidArgument(format!(
                    "unknown argument {argument}"
                )));
            }
            if values.insert(argument.clone(), value).is_some() {
                return Err(WorkerRuntimeError::InvalidArgument(format!(
                    "duplicate argument {argument}"
                )));
            }
        }
        let required = |name: &str| {
            values
                .get(name)
                .cloned()
                .ok_or_else(|| WorkerRuntimeError::InvalidArgument(format!("{name} is required")))
        };
        let parse_u64 = |name: &str| {
            required(name)?
                .parse::<u64>()
                .map_err(|_| WorkerRuntimeError::InvalidArgument(format!("invalid {name}")))
        };
        let heartbeat_ms = parse_u64("--heartbeat-ms")?;
        let lease_ms = parse_u64("--lease-ms")?;
        if heartbeat_ms == 0 || lease_ms <= heartbeat_ms.saturating_mul(2) {
            return Err(WorkerRuntimeError::InvalidArgument(
                "lease duration must exceed two non-zero heartbeat intervals".into(),
            ));
        }
        Ok(Self {
            state_dir: PathBuf::from(required("--state-dir")?),
            lease_database: PathBuf::from(required("--lease-db")?),
            control_socket: PathBuf::from(required("--control-socket")?),
            grant: LeaseGrant {
                root_tree_id: required("--root-tree")?.parse().map_err(|_| {
                    WorkerRuntimeError::InvalidArgument("invalid --root-tree".into())
                })?,
                worker_id: required("--worker-id")?.parse().map_err(|_| {
                    WorkerRuntimeError::InvalidArgument("invalid --worker-id".into())
                })?,
                generation: Generation::new(parse_u64("--generation")?),
                authentication: required("--authentication")?.parse().map_err(|_| {
                    WorkerRuntimeError::InvalidArgument("invalid --authentication".into())
                })?,
                expires_at: UtcTimestamp::from_unix_millis(
                    required("--expires-at")?.parse().map_err(|_| {
                        WorkerRuntimeError::InvalidArgument("invalid --expires-at".into())
                    })?,
                ),
            },
            heartbeat_interval: Duration::from_millis(heartbeat_ms),
            lease_duration: Duration::from_millis(lease_ms),
        })
    }
}

#[derive(Debug, Error)]
pub enum WorkerRuntimeError {
    #[error("worker argument error: {0}")]
    InvalidArgument(String),
    #[error("worker state I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("worker state serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("worker clock failed: {0}")]
    Clock(#[from] keith_agent_types::TimestampError),
    #[error("worker signal registration failed: {0}")]
    Signal(std::io::Error),
    #[error(transparent)]
    Lease(#[from] LeaseError),
    #[error(transparent)]
    Private(#[from] PrivateProtocolError),
    #[error("root {0} already has a local writer")]
    WriterLocked(RootTreeId),
    #[error("worker lost its root-tree lease")]
    LeaseLost,
    #[error("worker control endpoint failed: {0}")]
    Connection(#[from] keith_connection::ConnectionError),
}

pub fn registration_path(state_dir: &Path, root_tree_id: &RootTreeId) -> PathBuf {
    state_dir
        .join("workers")
        .join(format!("{root_tree_id}.json"))
}

/// # Errors
///
/// Returns an error when arguments, signals, leases, private control, or state writes fail.
pub fn run_from_environment() -> Result<(), WorkerRuntimeError> {
    let arguments = WorkerArguments::parse(std::env::args_os())?;
    let shutdown = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&shutdown))
        .map_err(WorkerRuntimeError::Signal)?;
    signal_hook::flag::register(SIGINT, Arc::clone(&shutdown))
        .map_err(WorkerRuntimeError::Signal)?;
    run_worker(&arguments, &shutdown)
}

/// # Errors
///
/// Returns an error when ownership is lost or worker state cannot be durably published.
pub fn run_worker(
    arguments: &WorkerArguments,
    shutdown: &AtomicBool,
) -> Result<(), WorkerRuntimeError> {
    let manager = LeaseManager::open(&arguments.lease_database)?;
    manager.validate(&arguments.grant)?;
    let _writer_guard = WriterGuard::acquire(&arguments.state_dir, &arguments.grant)?;
    if let Some(parent) = arguments.control_socket.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::remove_file(&arguments.control_socket) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let listener = bind_permissioned_local(&arguments.control_socket)?;
    listener.set_nonblocking(true)?;
    let started_at = UtcTimestamp::now()?;
    let mut registration = WorkerRegistration {
        version: CURRENT_SCHEMA_VERSION,
        worker_id: arguments.grant.worker_id.clone(),
        root_tree_id: arguments.grant.root_tree_id.clone(),
        generation: arguments.grant.generation,
        pid: std::process::id(),
        control_socket: arguments.control_socket.clone(),
        started_at,
        heartbeat_at: started_at,
        state: WorkerRunState::Starting,
    };
    write_registration(&arguments.state_dir, &registration)?;
    registration.state = WorkerRunState::Ready;
    write_registration(&arguments.state_dir, &registration)?;
    let mut grant = arguments.grant.clone();
    let mut control: Option<PrivateTransport<UnixStream>> = None;
    let mut next_heartbeat = Instant::now();
    let mut heartbeat_count = 0_u64;
    let mut requested_shutdown = false;
    while !shutdown.load(Ordering::Acquire) && !requested_shutdown {
        requested_shutdown = service_control(&listener, &mut control, &grant)?;
        if Instant::now() >= next_heartbeat {
            renew_and_publish(
                &manager,
                &mut grant,
                arguments,
                &mut registration,
                &mut control,
                heartbeat_count,
            )?;
            heartbeat_count = heartbeat_count.saturating_add(1);
            next_heartbeat = Instant::now() + arguments.heartbeat_interval;
        }
        thread::sleep(Duration::from_millis(5));
    }
    registration.state = WorkerRunState::Draining;
    registration.heartbeat_at = UtcTimestamp::now()?;
    write_registration(&arguments.state_dir, &registration)?;
    manager.release(&grant)?;
    registration.state = WorkerRunState::Stopped;
    registration.heartbeat_at = UtcTimestamp::now()?;
    write_registration(&arguments.state_dir, &registration)?;
    drop(listener);
    let _ = fs::remove_file(&arguments.control_socket);
    Ok(())
}

fn service_control(
    listener: &UnixListener,
    control: &mut Option<PrivateTransport<UnixStream>>,
    grant: &LeaseGrant,
) -> Result<bool, WorkerRuntimeError> {
    if control.is_none() {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_read_timeout(Some(Duration::from_millis(5)))?;
                stream.set_write_timeout(Some(Duration::from_secs(1)))?;
                *control = Some(PrivateTransport::new(stream, grant.clone())?);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error.into()),
        }
    }
    let Some(mut connection) = control.take() else {
        return Ok(false);
    };
    let mut keep = true;
    let mut requested_shutdown = false;
    match connection.receive() {
        Ok(PrivateMessage::SupervisorHello) => {
            if let Err(error) = connection.send(PrivateMessage::Ready {
                pid: std::process::id(),
            }) {
                if error.is_connection_loss() {
                    keep = false;
                } else {
                    return Err(error.into());
                }
            }
        }
        Ok(PrivateMessage::Shutdown { .. }) => {
            connection.send(PrivateMessage::ShutdownAck)?;
            requested_shutdown = true;
        }
        Ok(_) => {}
        Err(error) if error.is_retryable_io() => {}
        Err(error) if error.is_connection_loss() => keep = false,
        Err(PrivateProtocolError::Unauthenticated | PrivateProtocolError::StaleRoute) => {
            keep = false;
        }
        Err(error) => return Err(error.into()),
    }
    if keep {
        *control = Some(connection);
    }
    Ok(requested_shutdown)
}

fn renew_and_publish(
    manager: &LeaseManager,
    grant: &mut LeaseGrant,
    arguments: &WorkerArguments,
    registration: &mut WorkerRegistration,
    control: &mut Option<PrivateTransport<UnixStream>>,
    heartbeat_count: u64,
) -> Result<(), WorkerRuntimeError> {
    let Ok(renewed) = manager.renew(grant, arguments.lease_duration) else {
        registration.state = WorkerRunState::Failed;
        registration.heartbeat_at = UtcTimestamp::now()?;
        write_registration(&arguments.state_dir, registration)?;
        if let Some(connection) = control.as_mut() {
            let _ = connection.send(PrivateMessage::Fatal {
                reason: "root-tree lease was lost".into(),
            });
        }
        return Err(WorkerRuntimeError::LeaseLost);
    };
    *grant = renewed;
    registration.heartbeat_at = UtcTimestamp::now()?;
    write_registration(&arguments.state_dir, registration)?;
    if let Some(connection) = control.as_mut() {
        let message = if heartbeat_count > 0 && heartbeat_count.is_multiple_of(50) {
            PrivateMessage::Idle {
                since: registration.started_at,
            }
        } else {
            PrivateMessage::Heartbeat {
                at: registration.heartbeat_at,
            }
        };
        if connection.send(message).is_err() {
            *control = None;
        }
    }
    Ok(())
}

/// # Errors
///
/// Returns an error when the registration does not exist, is invalid, or has an unsupported schema.
pub fn read_registration(path: &Path) -> Result<WorkerRegistration, WorkerRuntimeError> {
    let bytes = fs::read(path)?;
    let registration: WorkerRegistration = serde_json::from_slice(&bytes)?;
    if registration.version != CURRENT_SCHEMA_VERSION {
        return Err(WorkerRuntimeError::InvalidArgument(format!(
            "unsupported worker registration schema {}",
            registration.version
        )));
    }
    Ok(registration)
}

fn write_registration(
    state_dir: &Path,
    registration: &WorkerRegistration,
) -> Result<(), WorkerRuntimeError> {
    let parent = state_dir.join("workers");
    let path = parent.join(format!("{}.json", registration.root_tree_id));
    fs::create_dir_all(&parent)?;
    let temporary = path.with_extension(format!("{}.tmp", registration.pid));
    let bytes = keith_agent_types::canonical_json_bytes(registration)?;
    fs::write(&temporary, bytes)?;
    File::open(&temporary)?.sync_all()?;
    fs::rename(&temporary, &path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::sync::{Arc, Barrier};

    use super::*;

    fn grant(root: RootTreeId) -> LeaseGrant {
        LeaseGrant {
            root_tree_id: root,
            worker_id: WorkerId::new(),
            generation: Generation::new(3),
            authentication: EntityId::new(),
            expires_at: UtcTimestamp::from_unix_millis(1_000),
        }
    }

    #[test]
    fn private_frames_authenticate_every_route_field() {
        let grant = grant(RootTreeId::new());
        let mut writer = PrivateTransport::new(Vec::new(), grant.clone()).unwrap();
        writer.send(PrivateMessage::Ready { pid: 42 }).unwrap();
        let bytes = writer.into_inner();
        let mut reader = PrivateTransport::new(Cursor::new(bytes.clone()), grant.clone()).unwrap();
        assert_eq!(reader.receive().unwrap(), PrivateMessage::Ready { pid: 42 });

        let mut wrong = grant;
        wrong.authentication = EntityId::new();
        let mut reader = PrivateTransport::new(Cursor::new(bytes), wrong).unwrap();
        assert!(matches!(
            reader.receive(),
            Err(PrivateProtocolError::Unauthenticated)
        ));
    }

    #[test]
    fn simultaneous_claims_have_one_winner_and_expiry_advances_generation() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("leases.sqlite");
        let root = RootTreeId::new();
        let barrier = Arc::new(Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let database = database.clone();
                let root = root.clone();
                let barrier = Arc::clone(&barrier);
                thread::spawn(move || {
                    let manager = LeaseManager::open(&database).unwrap();
                    barrier.wait();
                    manager.claim_at(
                        &root,
                        WorkerId::new(),
                        UtcTimestamp::from_unix_millis(100),
                        Duration::from_millis(50),
                    )
                })
            })
            .collect();
        let results: Vec<_> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        let winner = results.into_iter().find_map(Result::ok).unwrap();

        let manager = LeaseManager::open(&database).unwrap();
        let replacement = manager
            .claim_at(
                &root,
                WorkerId::new(),
                UtcTimestamp::from_unix_millis(151),
                Duration::from_millis(50),
            )
            .unwrap();
        assert_eq!(replacement.generation, Generation::new(2));
        assert!(matches!(
            manager.renew_at(
                &winner,
                UtcTimestamp::from_unix_millis(152),
                Duration::from_millis(50)
            ),
            Err(LeaseError::OwnershipLost(_))
        ));
    }

    #[test]
    fn advisory_writer_guard_rejects_a_second_local_writer() {
        let directory = tempfile::tempdir().unwrap();
        let grant = grant(RootTreeId::new());
        let first = WriterGuard::acquire(directory.path(), &grant).unwrap();
        assert!(matches!(
            WriterGuard::acquire(directory.path(), &grant),
            Err(WorkerRuntimeError::WriterLocked(_))
        ));
        drop(first);
        WriterGuard::acquire(directory.path(), &grant).unwrap();
    }
}
