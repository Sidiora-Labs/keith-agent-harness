#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use keith_agent_types::{
    CURRENT_PROTOCOL_VERSION, CURRENT_SCHEMA_VERSION, ClientId, CommandId, EntityId, SchemaVersion,
    UtcTimestamp,
};
use keith_connection::{
    AgentTransport, FramedTransport, connect_local, set_local_read_timeout, set_local_write_timeout,
};
use keith_platform::PlatformPaths;
use keith_protocol::{
    ClientCommand, ClientHello, CommandEnvelope, CommandResult, ResponsePayload, SessionFilter,
    SessionSummary, WireFormat, WireMessage,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

const MAX_CRASH_BYTES: usize = 64 * 1_024;
const MAX_NOTIFICATION_BYTES: usize = 8 * 1_024;

#[derive(Debug, Error)]
pub enum DesktopError {
    #[error("desktop I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("desktop state encoding failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("desktop configuration is invalid")]
    InvalidConfiguration,
    #[error("desktop path is unsafe")]
    UnsafePath,
    #[error("desktop process did not become ready")]
    StartupTimeout,
    #[error("desktop process is not owned by this shell")]
    NotOwned,
    #[error("agent connection failed: {0}")]
    AgentConnection(String),
    #[error("update digest did not match")]
    DigestMismatch,
    #[error("update version already exists")]
    VersionExists,
    #[error("update rollback target is unavailable")]
    RollbackUnavailable,
    #[error("uninstall confirmation did not match the exact scope")]
    ConfirmationRequired,
    #[error("browser handoff is not a local authenticated application route")]
    InvalidBrowserHandoff,
    #[error("required child secret environment is unavailable")]
    MissingSecretEnvironment,
    #[error("random generation failed")]
    Random,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopSettings {
    pub version: SchemaVersion,
    pub installation_id: EntityId,
    pub state_root: PathBuf,
    pub data_root: PathBuf,
    pub daemon_socket: PathBuf,
    pub web_origin: String,
    pub created_at: UtcTimestamp,
}

pub struct DesktopBootstrap;

impl DesktopBootstrap {
    /// Creates or reopens desktop state in the native platform locations.
    ///
    /// # Errors
    ///
    /// Returns an error when platform paths cannot be discovered or initialized.
    pub fn initialize_default(web_origin: &str) -> Result<DesktopSettings, DesktopError> {
        let paths = PlatformPaths::discover().map_err(|_| DesktopError::InvalidConfiguration)?;
        let mut settings = Self::initialize(&paths.state_root, &paths.data_root, web_origin)?;
        if settings.daemon_socket != paths.daemon_endpoint {
            settings.daemon_socket = paths.daemon_endpoint;
            atomic_json(&settings.state_root.join("desktop.json"), &settings)?;
        }
        Ok(settings)
    }

    /// Creates or reopens the non-secret first-run desktop state.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe paths, invalid loopback origin, or persistence failure.
    pub fn initialize(
        state_root: &Path,
        data_root: &Path,
        web_origin: &str,
    ) -> Result<DesktopSettings, DesktopError> {
        validate_absolute_root(state_root)?;
        validate_absolute_root(data_root)?;
        validate_loopback_origin(web_origin)?;
        fs::create_dir_all(state_root)?;
        fs::create_dir_all(data_root)?;
        for directory in ["crashes", "notifications", "updates", "backups"] {
            fs::create_dir_all(state_root.join(directory))?;
        }
        let path = state_root.join("desktop.json");
        if path.exists() {
            let existing = serde_json::from_slice::<DesktopSettings>(&fs::read(path)?)?;
            if existing.version.major != CURRENT_SCHEMA_VERSION.major
                || existing.state_root != state_root
                || existing.data_root != data_root
                || existing.web_origin != web_origin
            {
                return Err(DesktopError::InvalidConfiguration);
            }
            return Ok(existing);
        }
        let settings = DesktopSettings {
            version: CURRENT_SCHEMA_VERSION,
            installation_id: EntityId::new(),
            state_root: state_root.to_path_buf(),
            data_root: data_root.to_path_buf(),
            daemon_socket: data_root.join("agentd.sock"),
            web_origin: web_origin.into(),
            created_at: UtcTimestamp::now().map_err(|_| DesktopError::InvalidConfiguration)?,
        };
        atomic_json(&path, &settings)?;
        Ok(settings)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesktopProcessConfig {
    pub settings: DesktopSettings,
    pub daemon_executable: PathBuf,
    pub worker_executable: PathBuf,
    pub web_executable: PathBuf,
    pub web_bind: String,
    pub asset_root: PathBuf,
    pub credential_root: PathBuf,
    pub login_secret_env: String,
    pub credential_key_env: String,
    pub startup_timeout: Duration,
    pub shutdown_grace: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProcessOwnership {
    Existing,
    Owned,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManagedProcessKind {
    Daemon,
    Web,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrashReport {
    pub version: SchemaVersion,
    pub id: EntityId,
    pub process: ManagedProcessKind,
    pub exit_code: Option<i32>,
    pub safe_detail: String,
    pub observed_at: UtcTimestamp,
}

struct OwnedProcess {
    child: Child,
    stderr_path: PathBuf,
}

pub struct DesktopLifecycle {
    config: DesktopProcessConfig,
    daemon: Option<OwnedProcess>,
    web: Option<OwnedProcess>,
}

impl DesktopLifecycle {
    /// Validates packaged executables and web assets before process ownership begins.
    ///
    /// # Errors
    ///
    /// Returns an error when paths, timeouts, executables, assets, or origins are invalid.
    pub fn new(config: DesktopProcessConfig) -> Result<Self, DesktopError> {
        if config.startup_timeout.is_zero()
            || config.shutdown_grace.is_zero()
            || !config.daemon_executable.is_file()
            || !config.worker_executable.is_file()
            || !config.web_executable.is_file()
            || !config.asset_root.join("agent_web.js").is_file()
            || !config.asset_root.join("agent_web_bg.wasm").is_file()
            || config.login_secret_env.is_empty()
            || config.credential_key_env.is_empty()
        {
            return Err(DesktopError::InvalidConfiguration);
        }
        validate_loopback_origin(&config.settings.web_origin)?;
        Ok(Self {
            config,
            daemon: None,
            web: None,
        })
    }

    /// Reuses a healthy daemon or starts and probes the packaged daemon process.
    ///
    /// # Errors
    ///
    /// Returns an error for unsafe stale endpoints, spawn failure, or readiness timeout.
    pub fn ensure_daemon(&mut self) -> Result<ProcessOwnership, DesktopError> {
        if DesktopConnection::probe(&self.config.settings.daemon_socket).is_ok() {
            return Ok(ProcessOwnership::Existing);
        }
        remove_stale_socket(&self.config.settings.daemon_socket)?;
        let stderr_path = self.process_log_path(ManagedProcessKind::Daemon);
        let stderr = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&stderr_path)?;
        let child = Command::new(&self.config.daemon_executable)
            .arg("--data-root")
            .arg(&self.config.settings.data_root)
            .arg("--socket")
            .arg(&self.config.settings.daemon_socket)
            .arg("--worker-executable")
            .arg(&self.config.worker_executable)
            .arg("--idle-seconds")
            .arg("900")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr))
            .spawn()?;
        self.daemon = Some(OwnedProcess { child, stderr_path });
        self.wait_for_daemon()?;
        Ok(ProcessOwnership::Owned)
    }

    /// Starts the authenticated packaged web application or reuses its listener.
    ///
    /// # Errors
    ///
    /// Returns an error for missing narrow secret references, spawn failure, or timeout.
    pub fn ensure_web(&mut self) -> Result<ProcessOwnership, DesktopError> {
        if web_listener_ready(&self.config.web_bind) {
            return Ok(ProcessOwnership::Existing);
        }
        let login_secret = std::env::var_os(&self.config.login_secret_env)
            .ok_or(DesktopError::MissingSecretEnvironment)?;
        let credential_key = std::env::var_os(&self.config.credential_key_env)
            .ok_or(DesktopError::MissingSecretEnvironment)?;
        let stderr_path = self.process_log_path(ManagedProcessKind::Web);
        let stderr = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&stderr_path)?;
        let mut command = Command::new(&self.config.web_executable);
        command
            .arg("--bind")
            .arg(&self.config.web_bind)
            .arg("--origin")
            .arg(&self.config.settings.web_origin)
            .arg("--socket")
            .arg(&self.config.settings.daemon_socket)
            .arg("--asset-root")
            .arg(&self.config.asset_root)
            .arg("--credential-root")
            .arg(&self.config.credential_root)
            .arg("--login-secret-env")
            .arg(&self.config.login_secret_env)
            .arg("--credential-key-env")
            .arg(&self.config.credential_key_env)
            .env_clear()
            .env(&self.config.login_secret_env, login_secret)
            .env(&self.config.credential_key_env, credential_key)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr));
        let child = command.spawn()?;
        self.web = Some(OwnedProcess { child, stderr_path });
        self.wait_for_web()?;
        Ok(ProcessOwnership::Owned)
    }

    pub fn connection(&self) -> DesktopConnection {
        DesktopConnection {
            socket: self.config.settings.daemon_socket.clone(),
        }
    }

    /// Observes real child termination and persists a bounded redacted crash report.
    ///
    /// # Errors
    ///
    /// Returns an error when process status or crash persistence cannot be inspected.
    pub fn poll_crashes(&mut self) -> Result<Vec<CrashReport>, DesktopError> {
        let mut reports = Vec::new();
        if let Some(report) = poll_process(
            &self.config.settings.state_root,
            &mut self.daemon,
            ManagedProcessKind::Daemon,
        )? {
            reports.push(report);
        }
        if let Some(report) = poll_process(
            &self.config.settings.state_root,
            &mut self.web,
            ManagedProcessKind::Web,
        )? {
            reports.push(report);
        }
        Ok(reports)
    }

    /// Gracefully stops only processes started by this desktop instance.
    ///
    /// # Errors
    ///
    /// Returns an error if a child cannot be signalled, reaped, or force-stopped.
    pub fn stop_owned(&mut self) -> Result<(), DesktopError> {
        stop_process(&mut self.web, self.config.shutdown_grace)?;
        stop_process(&mut self.daemon, self.config.shutdown_grace)
    }

    fn wait_for_daemon(&mut self) -> Result<(), DesktopError> {
        let deadline = Instant::now() + self.config.startup_timeout;
        loop {
            if DesktopConnection::probe(&self.config.settings.daemon_socket).is_ok() {
                return Ok(());
            }
            if self
                .daemon
                .as_mut()
                .and_then(|process| process.child.try_wait().ok().flatten())
                .is_some()
            {
                return Err(DesktopError::StartupTimeout);
            }
            if Instant::now() >= deadline {
                stop_process(&mut self.daemon, self.config.shutdown_grace)?;
                return Err(DesktopError::StartupTimeout);
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_for_web(&mut self) -> Result<(), DesktopError> {
        let deadline = Instant::now() + self.config.startup_timeout;
        loop {
            if web_listener_ready(&self.config.web_bind) {
                return Ok(());
            }
            if self
                .web
                .as_mut()
                .and_then(|process| process.child.try_wait().ok().flatten())
                .is_some()
            {
                return Err(DesktopError::StartupTimeout);
            }
            if Instant::now() >= deadline {
                stop_process(&mut self.web, self.config.shutdown_grace)?;
                return Err(DesktopError::StartupTimeout);
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn process_log_path(&self, kind: ManagedProcessKind) -> PathBuf {
        let label = match kind {
            ManagedProcessKind::Daemon => "daemon",
            ManagedProcessKind::Web => "web",
        };
        self.config
            .settings
            .state_root
            .join("crashes")
            .join(format!("{label}-{}.stderr", EntityId::new()))
    }
}

impl Drop for DesktopLifecycle {
    fn drop(&mut self) {
        let _ = self.stop_owned();
    }
}

#[derive(Clone, Debug)]
pub struct DesktopConnection {
    socket: PathBuf,
}

impl DesktopConnection {
    /// Lists sessions exclusively through `AgentConnection`.
    ///
    /// # Errors
    ///
    /// Returns an error for transport, negotiation, or response-type failure.
    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>, DesktopError> {
        let stream = connect_local(&self.socket)
            .map_err(|error| DesktopError::AgentConnection(error.to_string()))?;
        set_local_read_timeout(&stream, Some(Duration::from_secs(2)))
            .map_err(|error| DesktopError::AgentConnection(error.to_string()))?;
        set_local_write_timeout(&stream, Some(Duration::from_secs(2)))
            .map_err(|error| DesktopError::AgentConnection(error.to_string()))?;
        let mut transport = FramedTransport::new(stream, WireFormat::Json);
        let client_id = ClientId::new();
        transport
            .send(&WireMessage::ClientHello(ClientHello {
                protocol: CURRENT_PROTOCOL_VERSION,
                client_id: client_id.clone(),
                client_name: "keith-agent-desktop".into(),
                client_version: env!("CARGO_PKG_VERSION").into(),
                supported_features: BTreeSet::new(),
                resume: None,
            }))
            .map_err(|error| DesktopError::AgentConnection(error.to_string()))?;
        if !matches!(
            transport
                .receive()
                .map_err(|error| DesktopError::AgentConnection(error.to_string()))?,
            WireMessage::ServerHello(_)
        ) {
            return Err(DesktopError::AgentConnection(
                "daemon did not negotiate AgentConnection".into(),
            ));
        }
        transport
            .send(&WireMessage::Command(CommandEnvelope {
                protocol: CURRENT_PROTOCOL_VERSION,
                command_id: CommandId::new(),
                client_id,
                sent_at: UtcTimestamp::now().map_err(|_| DesktopError::InvalidConfiguration)?,
                session_id: None,
                command: ClientCommand::ListSessions(SessionFilter::default()),
            }))
            .map_err(|error| DesktopError::AgentConnection(error.to_string()))?;
        let WireMessage::CommandResult(result) = transport
            .receive()
            .map_err(|error| DesktopError::AgentConnection(error.to_string()))?
        else {
            return Err(DesktopError::AgentConnection(
                "daemon returned an unexpected message".into(),
            ));
        };
        let CommandResult::Data(payload) = result.result else {
            return Err(DesktopError::AgentConnection(
                "daemon rejected session listing".into(),
            ));
        };
        let ResponsePayload::Sessions(sessions) = *payload else {
            return Err(DesktopError::AgentConnection(
                "daemon returned an unexpected response".into(),
            ));
        };
        Ok(sessions)
    }

    fn probe(socket: &Path) -> Result<(), DesktopError> {
        Self {
            socket: socket.to_path_buf(),
        }
        .list_sessions()
        .map(|_| ())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopNotification {
    pub version: SchemaVersion,
    pub id: EntityId,
    pub title: String,
    pub body: String,
    pub route: String,
    pub created_at: UtcTimestamp,
    pub acknowledged_at: Option<UtcTimestamp>,
}

pub struct DesktopNotificationCenter {
    root: PathBuf,
}

impl DesktopNotificationCenter {
    /// Opens the in-shell local notification queue.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsafe or unavailable notification root.
    pub fn open(state_root: &Path) -> Result<Self, DesktopError> {
        let root = state_root.join("notifications");
        fs::create_dir_all(&root)?;
        reject_symlink(&root)?;
        Ok(Self { root })
    }

    /// Persists a bounded local notification linked to an application route.
    ///
    /// # Errors
    ///
    /// Returns an error for oversized/private content, invalid route, or persistence failure.
    pub fn notify(
        &self,
        title: impl Into<String>,
        body: impl Into<String>,
        route: impl Into<String>,
        now: UtcTimestamp,
    ) -> Result<DesktopNotification, DesktopError> {
        let notification = DesktopNotification {
            version: CURRENT_SCHEMA_VERSION,
            id: EntityId::new(),
            title: title.into(),
            body: body.into(),
            route: route.into(),
            created_at: now,
            acknowledged_at: None,
        };
        if notification.title.trim().is_empty()
            || notification.title.len() + notification.body.len() > MAX_NOTIFICATION_BYTES
            || !notification.route.starts_with('/')
            || notification.route.starts_with("//")
            || contains_secret(&format!("{} {}", notification.title, notification.body))
        {
            return Err(DesktopError::InvalidConfiguration);
        }
        atomic_json(&self.path(&notification.id), &notification)?;
        Ok(notification)
    }

    /// Marks a persisted notification as observed by the local shell.
    ///
    /// # Errors
    ///
    /// Returns an error when the notification is missing or cannot be persisted.
    pub fn acknowledge(
        &self,
        id: &EntityId,
        now: UtcTimestamp,
    ) -> Result<DesktopNotification, DesktopError> {
        let path = self.path(id);
        let mut notification = serde_json::from_slice::<DesktopNotification>(&fs::read(&path)?)?;
        notification.acknowledged_at = Some(now);
        atomic_json(&path, &notification)?;
        Ok(notification)
    }

    fn path(&self, id: &EntityId) -> PathBuf {
        self.root.join(format!("{id}.json"))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedFile {
    pub path: PathBuf,
    pub bytes: u64,
}

/// Resolves a file selection beneath an explicitly granted root without reading its content.
///
/// # Errors
///
/// Returns an error for symlinks, directories, missing files, or paths outside every root.
pub fn select_file(path: &Path, allowed_roots: &[PathBuf]) -> Result<SelectedFile, DesktopError> {
    reject_symlink(path)?;
    let canonical = fs::canonicalize(path)?;
    let allowed = allowed_roots.iter().any(|root| {
        fs::canonicalize(root).is_ok_and(|canonical_root| canonical.starts_with(canonical_root))
    });
    let metadata = fs::metadata(&canonical)?;
    if !allowed || !metadata.is_file() {
        return Err(DesktopError::UnsafePath);
    }
    Ok(SelectedFile {
        path: canonical,
        bytes: metadata.len(),
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BrowserHandoff {
    url: Url,
}

impl BrowserHandoff {
    /// Constructs a loopback browser route without credentials or fragments.
    ///
    /// # Errors
    ///
    /// Returns an error for non-loopback origins, absolute route URLs, queries, or fragments.
    pub fn new(origin: &str, route: &str) -> Result<Self, DesktopError> {
        validate_loopback_origin(origin)?;
        if !route.starts_with('/')
            || route.starts_with("//")
            || route.contains(['?', '#'])
            || route.contains("..")
        {
            return Err(DesktopError::InvalidBrowserHandoff);
        }
        let url = Url::parse(origin)
            .and_then(|base| base.join(route))
            .map_err(|_| DesktopError::InvalidBrowserHandoff)?;
        if url.username() != "" || url.password().is_some() || url.query().is_some() {
            return Err(DesktopError::InvalidBrowserHandoff);
        }
        Ok(Self { url })
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Hands the safe route to the operating-system browser service.
    ///
    /// # Errors
    ///
    /// Returns an error when the platform browser service cannot be started.
    pub fn open(&self) -> Result<(), DesktopError> {
        #[cfg(target_os = "linux")]
        let status = Command::new("xdg-open").arg(self.url.as_str()).status()?;
        #[cfg(target_os = "macos")]
        let status = Command::new("open").arg(self.url.as_str()).status()?;
        #[cfg(target_os = "windows")]
        let status = Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", self.url.as_str()])
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(DesktopError::InvalidBrowserHandoff)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveRelease {
    pub version: SchemaVersion,
    pub current: String,
    pub previous: Option<String>,
    pub digest: String,
    pub activated_at: UtcTimestamp,
}

pub struct DesktopUpdateManager {
    root: PathBuf,
}

impl DesktopUpdateManager {
    /// Opens the versioned local installation area.
    ///
    /// # Errors
    ///
    /// Returns an error for an unsafe update root.
    pub fn open(state_root: &Path) -> Result<Self, DesktopError> {
        let root = state_root.join("updates");
        fs::create_dir_all(root.join("versions"))?;
        reject_symlink(&root)?;
        Ok(Self { root })
    }

    /// Returns the canonical digest for a release directory.
    ///
    /// # Errors
    ///
    /// Returns an error for symlinks, unsafe relative paths, or unreadable content.
    pub fn digest_release(source: &Path) -> Result<String, DesktopError> {
        let mut files = Vec::new();
        collect_files(source, source, &mut files)?;
        files.sort();
        let mut hash = Sha256::new();
        for relative in files {
            hash.update(relative.to_string_lossy().as_bytes());
            hash.update([0]);
            hash.update(fs::read(source.join(&relative))?);
        }
        Ok(hex_digest(hash.finalize()))
    }

    /// Stages, verifies, and atomically activates a complete version directory.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid version, duplicate target, digest mismatch, or unsafe content.
    pub fn activate(
        &self,
        version: &str,
        source: &Path,
        expected_digest: &str,
        now: UtcTimestamp,
    ) -> Result<ActiveRelease, DesktopError> {
        if !valid_version(version) {
            return Err(DesktopError::InvalidConfiguration);
        }
        let digest = Self::digest_release(source)?;
        if digest != expected_digest {
            return Err(DesktopError::DigestMismatch);
        }
        let target = self.root.join("versions").join(version);
        if target.exists() {
            return Err(DesktopError::VersionExists);
        }
        let temporary = self
            .root
            .join("versions")
            .join(format!(".{version}-{}.tmp", EntityId::new()));
        copy_tree(source, &temporary)?;
        fs::rename(&temporary, &target)?;
        File::open(target.parent().ok_or(DesktopError::UnsafePath)?)?.sync_all()?;
        let previous = self.active().ok().map(|active| active.current);
        let active = ActiveRelease {
            version: CURRENT_SCHEMA_VERSION,
            current: version.into(),
            previous,
            digest,
            activated_at: now,
        };
        atomic_json(&self.root.join("active.json"), &active)?;
        Ok(active)
    }

    /// Atomically returns to the recorded previous complete version.
    ///
    /// # Errors
    ///
    /// Returns an error when the previous release is missing, corrupt, or not recorded.
    pub fn rollback(&self, now: UtcTimestamp) -> Result<ActiveRelease, DesktopError> {
        let active = self.active()?;
        let previous = active.previous.ok_or(DesktopError::RollbackUnavailable)?;
        let target = self.root.join("versions").join(&previous);
        if !target.is_dir() {
            return Err(DesktopError::RollbackUnavailable);
        }
        let digest = Self::digest_release(&target)?;
        let rolled = ActiveRelease {
            version: CURRENT_SCHEMA_VERSION,
            current: previous,
            previous: Some(active.current),
            digest,
            activated_at: now,
        };
        atomic_json(&self.root.join("active.json"), &rolled)?;
        Ok(rolled)
    }

    /// Loads the active release pointer.
    ///
    /// # Errors
    ///
    /// Returns an error for missing or malformed active state.
    pub fn active(&self) -> Result<ActiveRelease, DesktopError> {
        Ok(serde_json::from_slice(&fs::read(
            self.root.join("active.json"),
        )?)?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UninstallChoice {
    KeepUserData,
    RemoveRuntime,
    RemoveEverything,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UninstallPlan {
    pub choice: UninstallChoice,
    pub exact_paths: Vec<PathBuf>,
    pub confirmation: String,
}

pub fn plan_uninstall(settings: &DesktopSettings, choice: UninstallChoice) -> UninstallPlan {
    let exact_paths = match choice {
        UninstallChoice::KeepUserData => vec![settings.state_root.join("updates")],
        UninstallChoice::RemoveRuntime => vec![
            settings.state_root.join("updates"),
            settings.data_root.join("runtime"),
        ],
        UninstallChoice::RemoveEverything => {
            vec![settings.state_root.clone(), settings.data_root.clone()]
        }
    };
    UninstallPlan {
        choice,
        exact_paths,
        confirmation: format!("REMOVE {}", settings.installation_id),
    }
}

/// Executes only the exact confirmed uninstall plan.
///
/// # Errors
///
/// Returns an error for mismatched confirmation, paths outside configured roots, or deletion failure.
pub fn execute_uninstall(
    settings: &DesktopSettings,
    plan: &UninstallPlan,
    confirmation: &str,
) -> Result<(), DesktopError> {
    if confirmation != plan.confirmation
        || plan.confirmation != format!("REMOVE {}", settings.installation_id)
    {
        return Err(DesktopError::ConfirmationRequired);
    }
    for path in &plan.exact_paths {
        if !(path.starts_with(&settings.state_root) || path.starts_with(&settings.data_root))
            || path == Path::new("/")
        {
            return Err(DesktopError::UnsafePath);
        }
    }
    for path in &plan.exact_paths {
        match fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(DesktopError::UnsafePath);
            }
            Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path)?,
            Ok(_) => fs::remove_file(path)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

/// Creates a verified filesystem backup of desktop-owned state.
///
/// # Errors
///
/// Returns an error for unsafe source content or backup persistence failure.
pub fn backup_state(settings: &DesktopSettings) -> Result<PathBuf, DesktopError> {
    let backup = settings
        .state_root
        .join("backups")
        .join(EntityId::new().to_string());
    fs::create_dir(&backup)?;
    copy_tree(&settings.data_root, &backup.join("data"))?;
    copy_tree(
        &settings.state_root.join("notifications"),
        &backup.join("notifications"),
    )?;
    Ok(backup)
}

/// Restores a verified backup into an empty target data directory.
///
/// # Errors
///
/// Returns an error for non-empty targets, symlinks, or invalid backup layout.
pub fn restore_state(backup: &Path, target_data_root: &Path) -> Result<(), DesktopError> {
    if target_data_root.exists() && fs::read_dir(target_data_root)?.next().is_some() {
        return Err(DesktopError::InvalidConfiguration);
    }
    copy_tree(&backup.join("data"), target_data_root)
}

fn validate_absolute_root(path: &Path) -> Result<(), DesktopError> {
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::CurDir | Component::Prefix(_)
            )
        })
        || path == Path::new("/")
    {
        Err(DesktopError::UnsafePath)
    } else {
        Ok(())
    }
}

fn validate_loopback_origin(origin: &str) -> Result<(), DesktopError> {
    let url = Url::parse(origin).map_err(|_| DesktopError::InvalidConfiguration)?;
    let valid_host = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]"));
    if url.scheme() != "http"
        || !valid_host
        || url.username() != ""
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        Err(DesktopError::InvalidConfiguration)
    } else {
        Ok(())
    }
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), DesktopError> {
    let parent = path.parent().ok_or(DesktopError::UnsafePath)?;
    fs::create_dir_all(parent)?;
    reject_symlink(parent)?;
    let temporary = path.with_extension(format!("{}.tmp", EntityId::new()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(&keith_agent_types::canonical_json_bytes(value)?)?;
    file.sync_all()?;
    keith_platform::replace_file(&temporary, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn reject_symlink(path: &Path) -> Result<(), DesktopError> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        Err(DesktopError::UnsafePath)
    } else {
        Ok(())
    }
}

fn remove_stale_socket(path: &Path) -> Result<(), DesktopError> {
    #[cfg(windows)]
    {
        let _ = path;
        return Ok(());
    }
    #[cfg(unix)]
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || metadata.is_dir() => {
            Err(DesktopError::UnsafePath)
        }
        Ok(_) => {
            fs::remove_file(path)?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn web_listener_ready(bind: &str) -> bool {
    bind.parse().ok().is_some_and(|address| {
        std::net::TcpStream::connect_timeout(&address, Duration::from_millis(50)).is_ok()
    })
}

fn poll_process(
    state_root: &Path,
    process: &mut Option<OwnedProcess>,
    kind: ManagedProcessKind,
) -> Result<Option<CrashReport>, DesktopError> {
    let Some(status) = process
        .as_mut()
        .map(|owned| owned.child.try_wait())
        .transpose()?
        .flatten()
    else {
        return Ok(None);
    };
    let owned = process.take().ok_or(DesktopError::NotOwned)?;
    let detail = read_safe_crash_detail(&owned.stderr_path)?;
    let report = CrashReport {
        version: CURRENT_SCHEMA_VERSION,
        id: EntityId::new(),
        process: kind,
        exit_code: status.code(),
        safe_detail: detail,
        observed_at: UtcTimestamp::now().map_err(|_| DesktopError::InvalidConfiguration)?,
    };
    atomic_json(
        &state_root
            .join("crashes")
            .join(format!("{}.json", report.id)),
        &report,
    )?;
    Ok(Some(report))
}

fn read_safe_crash_detail(path: &Path) -> Result<String, DesktopError> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(u64::try_from(MAX_CRASH_BYTES).unwrap_or(u64::MAX))
        .read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut safe = String::new();
    for line in text.lines().take(128) {
        if contains_secret(line) {
            safe.push_str("[REDACTED]\n");
        } else {
            safe.push_str(line);
            safe.push('\n');
        }
    }
    Ok(safe)
}

fn contains_secret(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    [
        "authorization",
        "bearer ",
        "password",
        "private_key",
        "secret",
    ]
    .iter()
    .any(|marker| value.contains(marker))
}

fn stop_process(process: &mut Option<OwnedProcess>, grace: Duration) -> Result<(), DesktopError> {
    let Some(mut owned) = process.take() else {
        return Ok(());
    };
    if owned.child.try_wait()?.is_some() {
        return Ok(());
    }
    signal_terminate(&mut owned.child)?;
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        if owned.child.try_wait()?.is_some() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    owned.child.kill()?;
    owned.child.wait()?;
    Ok(())
}

#[cfg(unix)]
fn signal_terminate(child: &mut Child) -> Result<(), DesktopError> {
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    let pid = i32::try_from(child.id()).map_err(|_| DesktopError::InvalidConfiguration)?;
    kill(Pid::from_raw(pid), Signal::SIGTERM)
        .map_err(|error| DesktopError::Io(std::io::Error::other(error)))
}

#[cfg(not(unix))]
fn signal_terminate(child: &mut Child) -> Result<(), DesktopError> {
    child.kill().map_err(DesktopError::from)
}

fn collect_files(
    root: &Path,
    current: &Path,
    output: &mut Vec<PathBuf>,
) -> Result<(), DesktopError> {
    reject_symlink(current)?;
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let metadata = entry.file_type()?;
        if metadata.is_symlink() {
            return Err(DesktopError::UnsafePath);
        }
        if metadata.is_dir() {
            collect_files(root, &entry.path(), output)?;
        } else if metadata.is_file() {
            output.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .map_err(|_| DesktopError::UnsafePath)?
                    .to_path_buf(),
            );
        } else {
            return Err(DesktopError::UnsafePath);
        }
    }
    Ok(())
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), DesktopError> {
    reject_symlink(source)?;
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(DesktopError::UnsafePath);
        }
        let destination = target.join(entry.file_name());
        if file_type.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else if file_type.is_file() {
            let mut input = File::open(entry.path())?;
            let mut output = OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(destination)?;
            std::io::copy(&mut input, &mut output)?;
            output.sync_all()?;
        } else {
            return Err(DesktopError::UnsafePath);
        }
    }
    File::open(target)?.sync_all()?;
    Ok(())
}

fn valid_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 64
        && version
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a String cannot fail");
            output
        })
}

/// Generates bounded random hexadecimal material for first-run secret stores.
///
/// # Errors
///
/// Returns an error when the operating-system random source is unavailable.
pub fn random_hex(bytes: usize) -> Result<String, DesktopError> {
    let mut value = vec![0_u8; bytes];
    getrandom::fill(&mut value).map_err(|_| DesktopError::Random)?;
    Ok(hex_digest(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_run_notification_file_browser_backup_and_uninstall_are_scoped() {
        let directory = tempfile::tempdir().unwrap();
        let state = directory.path().join("state");
        let data = directory.path().join("data");
        let settings =
            DesktopBootstrap::initialize(&state, &data, "http://127.0.0.1:7341").unwrap();
        assert_eq!(
            DesktopBootstrap::initialize(&state, &data, "http://127.0.0.1:7341").unwrap(),
            settings
        );

        let notifications = DesktopNotificationCenter::open(&state).unwrap();
        let notification = notifications
            .notify(
                "Goal complete",
                "Verified result",
                "/goals/1",
                UtcTimestamp::UNIX_EPOCH,
            )
            .unwrap();
        assert!(
            notifications
                .acknowledge(&notification.id, UtcTimestamp::from_unix_millis(1))
                .unwrap()
                .acknowledged_at
                .is_some()
        );
        assert!(
            notifications
                .notify("unsafe", "password is abc", "/", UtcTimestamp::UNIX_EPOCH)
                .is_err()
        );

        let workspace = data.join("workspace");
        fs::create_dir(&workspace).unwrap();
        fs::write(workspace.join("result.txt"), b"result").unwrap();
        assert_eq!(
            select_file(
                &workspace.join("result.txt"),
                std::slice::from_ref(&workspace)
            )
            .unwrap()
            .bytes,
            6
        );
        assert_eq!(
            BrowserHandoff::new("http://127.0.0.1:7341", "/sessions")
                .unwrap()
                .url()
                .as_str(),
            "http://127.0.0.1:7341/sessions"
        );
        assert!(BrowserHandoff::new("https://example.com", "/sessions").is_err());

        let backup = backup_state(&settings).unwrap();
        let restored = directory.path().join("restored");
        restore_state(&backup, &restored).unwrap();
        assert_eq!(
            fs::read(restored.join("workspace/result.txt")).unwrap(),
            b"result"
        );

        let plan = plan_uninstall(&settings, UninstallChoice::RemoveEverything);
        assert!(matches!(
            execute_uninstall(&settings, &plan, "no"),
            Err(DesktopError::ConfirmationRequired)
        ));
        execute_uninstall(&settings, &plan, &plan.confirmation).unwrap();
        assert!(!state.exists());
        assert!(!data.exists());
    }

    #[test]
    fn update_activation_digest_and_rollback_use_complete_version_directories() {
        let directory = tempfile::tempdir().unwrap();
        let state = directory.path().join("state");
        fs::create_dir(&state).unwrap();
        let manager = DesktopUpdateManager::open(&state).unwrap();
        let first = directory.path().join("release-1");
        let second = directory.path().join("release-2");
        fs::create_dir(&first).unwrap();
        fs::create_dir(&second).unwrap();
        fs::write(first.join("agentd"), b"version one").unwrap();
        fs::write(second.join("agentd"), b"version two").unwrap();
        let first_digest = DesktopUpdateManager::digest_release(&first).unwrap();
        manager
            .activate("1.0.0", &first, &first_digest, UtcTimestamp::UNIX_EPOCH)
            .unwrap();
        assert!(matches!(
            manager.activate("2.0.0", &second, "wrong", UtcTimestamp::UNIX_EPOCH),
            Err(DesktopError::DigestMismatch)
        ));
        let second_digest = DesktopUpdateManager::digest_release(&second).unwrap();
        manager
            .activate(
                "2.0.0",
                &second,
                &second_digest,
                UtcTimestamp::from_unix_millis(1),
            )
            .unwrap();
        let rolled = manager.rollback(UtcTimestamp::from_unix_millis(2)).unwrap();
        assert_eq!(rolled.current, "1.0.0");
        assert_eq!(manager.active().unwrap(), rolled);
    }
}
