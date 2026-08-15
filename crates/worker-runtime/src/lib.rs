#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use keith_agent_types::{
    CURRENT_SCHEMA_VERSION, EntityId, Generation, RootTreeId, SchemaVersion, UtcTimestamp, WorkerId,
};
use serde::{Deserialize, Serialize};
use signal_hook::consts::{SIGINT, SIGTERM};
use thiserror::Error;

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
    pub started_at: UtcTimestamp,
    pub heartbeat_at: UtcTimestamp,
    pub state: WorkerRunState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerArguments {
    pub state_dir: PathBuf,
    pub root_tree_id: RootTreeId,
    pub generation: Generation,
    pub heartbeat_interval: Duration,
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
        let mut state_dir = None;
        let mut root_tree_id = None;
        let mut generation = None;
        let mut heartbeat_ms = 100_u64;
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
            match argument.as_str() {
                "--state-dir" => state_dir = Some(PathBuf::from(value)),
                "--root-tree" => {
                    root_tree_id = Some(value.parse().map_err(|_| {
                        WorkerRuntimeError::InvalidArgument("invalid root tree ID".into())
                    })?);
                }
                "--generation" => {
                    generation = Some(Generation::new(value.parse().map_err(|_| {
                        WorkerRuntimeError::InvalidArgument("invalid generation".into())
                    })?));
                }
                "--heartbeat-ms" => {
                    heartbeat_ms = value.parse().map_err(|_| {
                        WorkerRuntimeError::InvalidArgument("invalid heartbeat interval".into())
                    })?;
                }
                _ => {
                    return Err(WorkerRuntimeError::InvalidArgument(format!(
                        "unknown argument {argument}"
                    )));
                }
            }
        }
        if heartbeat_ms == 0 {
            return Err(WorkerRuntimeError::InvalidArgument(
                "heartbeat interval must be non-zero".into(),
            ));
        }
        Ok(Self {
            state_dir: state_dir.ok_or_else(|| {
                WorkerRuntimeError::InvalidArgument("--state-dir is required".into())
            })?,
            root_tree_id: root_tree_id.ok_or_else(|| {
                WorkerRuntimeError::InvalidArgument("--root-tree is required".into())
            })?,
            generation: generation.ok_or_else(|| {
                WorkerRuntimeError::InvalidArgument("--generation is required".into())
            })?,
            heartbeat_interval: Duration::from_millis(heartbeat_ms),
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
}

pub fn registration_path(state_dir: &Path, root_tree_id: &RootTreeId) -> PathBuf {
    state_dir
        .join("workers")
        .join(format!("{root_tree_id}.json"))
}

/// # Errors
///
/// Returns an error when arguments, signals, state serialization, or atomic state writes fail.
pub fn run_from_environment() -> Result<(), WorkerRuntimeError> {
    let arguments = WorkerArguments::parse(std::env::args_os())?;
    let shutdown = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&shutdown))
        .map_err(WorkerRuntimeError::Signal)?;
    signal_hook::flag::register(SIGINT, Arc::clone(&shutdown))
        .map_err(WorkerRuntimeError::Signal)?;
    run_worker(arguments, &shutdown)
}

/// # Errors
///
/// Returns an error when worker state cannot be durably published.
pub fn run_worker(
    arguments: WorkerArguments,
    shutdown: &AtomicBool,
) -> Result<(), WorkerRuntimeError> {
    let started_at = UtcTimestamp::now()?;
    let mut registration = WorkerRegistration {
        version: CURRENT_SCHEMA_VERSION,
        worker_id: WorkerId::from(EntityId::new()),
        root_tree_id: arguments.root_tree_id,
        generation: arguments.generation,
        pid: std::process::id(),
        started_at,
        heartbeat_at: started_at,
        state: WorkerRunState::Starting,
    };
    write_registration(&arguments.state_dir, &registration)?;
    registration.state = WorkerRunState::Ready;
    write_registration(&arguments.state_dir, &registration)?;
    while !shutdown.load(Ordering::Acquire) {
        thread::sleep(arguments.heartbeat_interval);
        registration.heartbeat_at = UtcTimestamp::now()?;
        write_registration(&arguments.state_dir, &registration)?;
    }
    registration.state = WorkerRunState::Draining;
    registration.heartbeat_at = UtcTimestamp::now()?;
    write_registration(&arguments.state_dir, &registration)?;
    registration.state = WorkerRunState::Stopped;
    registration.heartbeat_at = UtcTimestamp::now()?;
    write_registration(&arguments.state_dir, &registration)
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
    use super::*;

    #[test]
    fn arguments_are_strict_and_registration_paths_are_scoped() {
        let root = RootTreeId::new();
        let parsed = WorkerArguments::parse([
            "worker",
            "--state-dir",
            "/tmp/keith-state",
            "--root-tree",
            &root.to_string(),
            "--generation",
            "3",
            "--heartbeat-ms",
            "25",
        ])
        .unwrap();
        assert_eq!(parsed.root_tree_id, root);
        assert_eq!(parsed.generation, Generation::new(3));
        assert_eq!(parsed.heartbeat_interval, Duration::from_millis(25));
        assert!(WorkerArguments::parse(["worker", "--unknown", "value"]).is_err());
        assert!(
            registration_path(Path::new("state"), &root).starts_with(Path::new("state/workers"))
        );
    }
}
