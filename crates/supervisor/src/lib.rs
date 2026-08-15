#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use keith_agent_types::{Generation, RootTreeId, UtcTimestamp, WorkerId};
use keith_worker_runtime::{
    WorkerRegistration, WorkerRunState, read_registration, registration_path,
};
use nix::errno::Errno;
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerHealth {
    Starting,
    Healthy,
    Unresponsive,
    Draining,
    Exited,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkerResourceState {
    pub resident_bytes: Option<u64>,
    pub virtual_bytes: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerStatus {
    pub worker_id: WorkerId,
    pub root_tree_id: RootTreeId,
    pub generation: Generation,
    pub pid: u32,
    pub health: WorkerHealth,
    pub heartbeat_at: UtcTimestamp,
    pub idle_for: Duration,
    pub resources: WorkerResourceState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkerEvent {
    Exited {
        root_tree_id: RootTreeId,
        generation: Generation,
        success: Option<bool>,
    },
}

#[derive(Clone, Debug)]
pub struct SupervisorOptions {
    pub startup_timeout: Duration,
    pub drain_timeout: Duration,
    pub stale_heartbeat: Duration,
    pub heartbeat_interval: Duration,
}

impl Default for SupervisorOptions {
    fn default() -> Self {
        Self {
            startup_timeout: Duration::from_secs(5),
            drain_timeout: Duration::from_secs(2),
            stale_heartbeat: Duration::from_secs(2),
            heartbeat_interval: Duration::from_millis(100),
        }
    }
}

struct ManagedWorker {
    registration: WorkerRegistration,
    child: Option<Child>,
    last_activity: Instant,
    draining: bool,
}

pub struct WorkerSupervisor {
    state_dir: PathBuf,
    executable: PathBuf,
    options: SupervisorOptions,
    workers: BTreeMap<RootTreeId, ManagedWorker>,
}

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("worker process I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("worker registration failed: {0}")]
    Registration(#[from] keith_worker_runtime::WorkerRuntimeError),
    #[error("worker {0} is already active")]
    AlreadyActive(RootTreeId),
    #[error("worker {0} is not active")]
    NotActive(RootTreeId),
    #[error("worker {root_tree_id} failed to become ready before the deadline")]
    StartupTimeout { root_tree_id: RootTreeId },
    #[error("worker {root_tree_id} exited during startup with {status}")]
    StartupExit {
        root_tree_id: RootTreeId,
        status: std::process::ExitStatus,
    },
    #[error("worker generation overflow for {0}")]
    GenerationOverflow(RootTreeId),
    #[error("worker signal failed: {0}")]
    Signal(Errno),
}

impl WorkerSupervisor {
    pub fn new(
        state_dir: impl Into<PathBuf>,
        executable: impl Into<PathBuf>,
        options: SupervisorOptions,
    ) -> Self {
        Self {
            state_dir: state_dir.into(),
            executable: executable.into(),
            options,
            workers: BTreeMap::new(),
        }
    }

    /// Adopts live workers from their metadata registrations without starting new processes.
    ///
    /// # Errors
    ///
    /// Returns an error when the registration directory cannot be read or contains invalid data.
    pub fn adopt_existing(&mut self) -> Result<Vec<WorkerStatus>, SupervisorError> {
        let directory = self.state_dir.join("workers");
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        for entry in entries {
            let path = entry?.path();
            if path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let registration = read_registration(&path)?;
            if !matches!(
                registration.state,
                WorkerRunState::Starting | WorkerRunState::Ready | WorkerRunState::Draining
            ) || !process_is_alive(registration.pid)
            {
                continue;
            }
            self.workers
                .entry(registration.root_tree_id.clone())
                .or_insert(ManagedWorker {
                    registration,
                    child: None,
                    last_activity: Instant::now(),
                    draining: false,
                });
        }
        Ok(self.statuses())
    }

    /// Starts a worker and waits for its production runtime to publish readiness.
    ///
    /// # Errors
    ///
    /// Returns an error when a live worker already exists or the child cannot become ready.
    pub fn start(
        &mut self,
        root_tree_id: RootTreeId,
        generation: Generation,
    ) -> Result<WorkerStatus, SupervisorError> {
        if self
            .workers
            .get(&root_tree_id)
            .is_some_and(|worker| process_is_alive(worker.registration.pid))
        {
            return Err(SupervisorError::AlreadyActive(root_tree_id));
        }
        self.workers.remove(&root_tree_id);
        let heartbeat_ms = self.options.heartbeat_interval.as_millis().max(1);
        let mut child = Command::new(&self.executable)
            .arg("--state-dir")
            .arg(&self.state_dir)
            .arg("--root-tree")
            .arg(root_tree_id.to_string())
            .arg("--generation")
            .arg(generation.get().to_string())
            .arg("--heartbeat-ms")
            .arg(heartbeat_ms.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
        let deadline = Instant::now() + self.options.startup_timeout;
        let path = registration_path(&self.state_dir, &root_tree_id);
        loop {
            if let Some(status) = child.try_wait()? {
                return Err(SupervisorError::StartupExit {
                    root_tree_id,
                    status,
                });
            }
            if let Ok(registration) = read_registration(&path)
                && registration.pid == child.id()
                && registration.root_tree_id == root_tree_id
                && registration.generation == generation
                && registration.state == WorkerRunState::Ready
            {
                let worker = ManagedWorker {
                    registration,
                    child: Some(child),
                    last_activity: Instant::now(),
                    draining: false,
                };
                let status = status_for(&worker, self.options.stale_heartbeat);
                self.workers.insert(root_tree_id, worker);
                return Ok(status);
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(SupervisorError::StartupTimeout { root_tree_id });
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn statuses(&self) -> Vec<WorkerStatus> {
        self.workers
            .values()
            .map(|worker| status_for(worker, self.options.stale_heartbeat))
            .collect()
    }

    pub fn status(&self, root_tree_id: &RootTreeId) -> Option<WorkerStatus> {
        self.workers
            .get(root_tree_id)
            .map(|worker| status_for(worker, self.options.stale_heartbeat))
    }

    pub fn mark_activity(&mut self, root_tree_id: &RootTreeId) -> bool {
        self.workers.get_mut(root_tree_id).is_some_and(|worker| {
            worker.last_activity = Instant::now();
            true
        })
    }

    /// Refreshes registrations and reports exits without disturbing unrelated workers.
    ///
    /// # Errors
    ///
    /// Returns an error when an owned child cannot be queried.
    pub fn monitor(&mut self) -> Result<Vec<WorkerEvent>, SupervisorError> {
        let roots: Vec<_> = self.workers.keys().cloned().collect();
        let mut events = Vec::new();
        for root in roots {
            let exited = {
                let Some(worker) = self.workers.get_mut(&root) else {
                    continue;
                };
                let status = if let Some(child) = worker.child.as_mut() {
                    child.try_wait()?.map(|status| status.success())
                } else if process_is_alive(worker.registration.pid) {
                    None
                } else {
                    Some(false)
                };
                if status.is_none() {
                    let path = registration_path(&self.state_dir, &root);
                    if let Ok(registration) = read_registration(&path)
                        && registration.pid == worker.registration.pid
                        && registration.generation == worker.registration.generation
                    {
                        worker.registration = registration;
                    }
                }
                status
            };
            if let Some(success) = exited
                && let Some(worker) = self.workers.remove(&root)
            {
                events.push(WorkerEvent::Exited {
                    root_tree_id: root,
                    generation: worker.registration.generation,
                    success: Some(success),
                });
            }
        }
        Ok(events)
    }

    /// Gracefully stops a worker, forcing termination after the configured deadline.
    ///
    /// # Errors
    ///
    /// Returns an error when no worker exists or signaling fails.
    pub fn drain(&mut self, root_tree_id: &RootTreeId) -> Result<(), SupervisorError> {
        let worker = self
            .workers
            .get_mut(root_tree_id)
            .ok_or_else(|| SupervisorError::NotActive(root_tree_id.clone()))?;
        worker.draining = true;
        signal(worker.registration.pid, Signal::SIGTERM)?;
        let pid = worker.registration.pid;
        let deadline = Instant::now() + self.options.drain_timeout;
        while process_is_alive(pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        if process_is_alive(pid) {
            signal(pid, Signal::SIGKILL)?;
        }
        if let Some(child) = worker.child.as_mut() {
            let _ = child.wait();
        } else {
            while process_is_alive(pid) {
                thread::sleep(Duration::from_millis(5));
            }
        }
        self.workers.remove(root_tree_id);
        Ok(())
    }

    /// Stops all workers as part of structured daemon shutdown.
    ///
    /// # Errors
    ///
    /// Returns the first worker shutdown error after attempting every worker.
    pub fn drain_all(&mut self) -> Result<(), SupervisorError> {
        let roots: Vec<_> = self.workers.keys().cloned().collect();
        let mut first_error = None;
        for root in roots {
            if let Err(error) = self.drain(&root)
                && first_error.is_none()
            {
                first_error = Some(error);
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Restarts a worker with the next generation.
    ///
    /// # Errors
    ///
    /// Returns an error when shutdown, generation advancement, or startup fails.
    pub fn restart(&mut self, root_tree_id: &RootTreeId) -> Result<WorkerStatus, SupervisorError> {
        let generation = self
            .workers
            .get(root_tree_id)
            .map(|worker| worker.registration.generation)
            .or_else(|| {
                read_registration(&registration_path(&self.state_dir, root_tree_id))
                    .ok()
                    .map(|registration| registration.generation)
            })
            .unwrap_or(Generation::ZERO)
            .checked_next()
            .ok_or_else(|| SupervisorError::GenerationOverflow(root_tree_id.clone()))?;
        if self.workers.contains_key(root_tree_id) {
            self.drain(root_tree_id)?;
        }
        self.start(root_tree_id.clone(), generation)
    }

    /// Drains workers whose supervisor-observed activity exceeds `idle_limit`.
    ///
    /// # Errors
    ///
    /// Returns an error when an idle worker cannot be stopped.
    pub fn evict_idle(&mut self, idle_limit: Duration) -> Result<Vec<RootTreeId>, SupervisorError> {
        let roots: Vec<_> = self
            .workers
            .iter()
            .filter(|(_, worker)| worker.last_activity.elapsed() >= idle_limit)
            .map(|(root, _)| root.clone())
            .collect();
        for root in &roots {
            self.drain(root)?;
        }
        Ok(roots)
    }
}

fn signal(pid: u32, signal: Signal) -> Result<(), SupervisorError> {
    let raw_pid = i32::try_from(pid).map_err(|_| SupervisorError::Signal(Errno::EINVAL))?;
    kill(Pid::from_raw(raw_pid), signal).map_err(SupervisorError::Signal)
}

fn process_is_alive(pid: u32) -> bool {
    let Ok(raw_pid) = i32::try_from(pid) else {
        return false;
    };
    if process_is_zombie(pid) {
        return false;
    }
    match kill(Pid::from_raw(raw_pid), None) {
        Ok(()) | Err(Errno::EPERM) => true,
        Err(_) => false,
    }
}

fn process_is_zombie(pid: u32) -> bool {
    fs::read_to_string(Path::new("/proc").join(pid.to_string()).join("stat"))
        .ok()
        .and_then(|stat| {
            stat.rsplit_once(") ")
                .map(|(_, fields)| fields.starts_with('Z'))
        })
        .unwrap_or(false)
}

fn status_for(worker: &ManagedWorker, stale_heartbeat: Duration) -> WorkerStatus {
    let heartbeat_age = UtcTimestamp::now()
        .ok()
        .and_then(|now| {
            now.unix_millis()
                .checked_sub(worker.registration.heartbeat_at.unix_millis())
        })
        .and_then(|millis| u64::try_from(millis).ok())
        .map_or(Duration::ZERO, Duration::from_millis);
    let health = if !process_is_alive(worker.registration.pid) {
        WorkerHealth::Exited
    } else if worker.draining || worker.registration.state == WorkerRunState::Draining {
        WorkerHealth::Draining
    } else if worker.registration.state == WorkerRunState::Starting {
        WorkerHealth::Starting
    } else if heartbeat_age > stale_heartbeat {
        WorkerHealth::Unresponsive
    } else {
        WorkerHealth::Healthy
    };
    WorkerStatus {
        worker_id: worker.registration.worker_id.clone(),
        root_tree_id: worker.registration.root_tree_id.clone(),
        generation: worker.registration.generation,
        pid: worker.registration.pid,
        health,
        heartbeat_at: worker.registration.heartbeat_at,
        idle_for: worker.last_activity.elapsed(),
        resources: read_resources(worker.registration.pid),
    }
}

fn read_resources(pid: u32) -> WorkerResourceState {
    let Ok(status) = fs::read_to_string(Path::new("/proc").join(pid.to_string()).join("status"))
    else {
        return WorkerResourceState::default();
    };
    let kibibytes = |prefix: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(prefix))
            .and_then(|value| value.split_whitespace().next())
            .and_then(|value| value.parse::<u64>().ok())
            .and_then(|value| value.checked_mul(1024))
    };
    WorkerResourceState {
        resident_bytes: kibibytes("VmRSS:"),
        virtual_bytes: kibibytes("VmSize:"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_registration_directory_is_an_empty_adoption_set() {
        let directory = tempfile::tempdir().unwrap();
        let mut supervisor = WorkerSupervisor::new(
            directory.path(),
            "/not/started/by-this-test",
            SupervisorOptions::default(),
        );
        assert!(supervisor.adopt_existing().unwrap().is_empty());
    }
}
