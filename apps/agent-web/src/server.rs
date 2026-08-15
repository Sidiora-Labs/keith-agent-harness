use std::collections::BTreeSet;
use std::ffi::OsString;
use std::net::{Shutdown, SocketAddr};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{DefaultBodyLimit, Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use keith_agent_types::{
    CURRENT_PROTOCOL_VERSION, ClientId, CommandId, ProfileId, Sequence, SessionId, UtcTimestamp,
};
use keith_connection::{AgentTransport, FramedTransport};
use keith_credentials::{
    BrowserWritePolicy, CredentialOwner, CredentialRef, CsrfToken, EncryptedCredentialStore,
    MasterKey, SecretValue,
};
use keith_framing::FrameError;
use keith_protocol::{
    AttachSession, ClientCommand, ClientHello, CommandEnvelope, CommandResult,
    CommandResultEnvelope, Feature, ProfileSummary, ResponsePayload, ResumeCursor, SessionFilter,
    SessionSummary, WireFormat, WireMessage,
};
use serde::Deserialize;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use url::Url;

use crate::security::{BrowserSecurity, SecurityError};
use crate::{APP_CSS, login_page, shell_page};

const MAX_BROWSER_BODY_BYTES: usize = 128 * 1024;
const EVENT_QUEUE_CAPACITY: usize = 256;
const BOOTSTRAP_JS: &str =
    "import init from '/assets/agent_web.js';init({module_or_path:'/assets/agent_web_bg.wasm'});";

pub struct WebServerConfig {
    pub bind: SocketAddr,
    pub exact_origin: String,
    pub daemon_socket: PathBuf,
    pub asset_root: PathBuf,
    pub credential_root: PathBuf,
    pub credential_key: MasterKey,
    pub login_secret: Vec<u8>,
    pub session_lifetime: Duration,
    pub mutation_limit_per_second: usize,
    pub daemon_timeout: Duration,
}

impl std::fmt::Debug for WebServerConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WebServerConfig")
            .field("bind", &self.bind)
            .field("exact_origin", &self.exact_origin)
            .field("daemon_socket", &self.daemon_socket)
            .field("asset_root", &self.asset_root)
            .field("credential_root", &self.credential_root)
            .field("credential_key", &"[REDACTED]")
            .field("login_secret", &"[REDACTED]")
            .field("session_lifetime", &self.session_lifetime)
            .field("mutation_limit_per_second", &self.mutation_limit_per_second)
            .field("daemon_timeout", &self.daemon_timeout)
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerArguments {
    pub bind: SocketAddr,
    pub exact_origin: String,
    pub daemon_socket: PathBuf,
    pub asset_root: PathBuf,
    pub credential_root: PathBuf,
    pub login_secret_env: String,
    pub credential_key_env: String,
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("server configuration is invalid: {0}")]
    Configuration(String),
    #[error("authentication setup failed")]
    Security(#[from] SecurityError),
    #[error("credential storage setup failed")]
    Credentials(#[from] keith_credentials::CredentialError),
    #[error("server I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP server failed: {0}")]
    Http(#[from] axum::Error),
}

#[derive(Clone)]
struct AppState {
    security: Arc<BrowserSecurity>,
    bridge: DaemonBridge,
    credential_store: Arc<EncryptedCredentialStore>,
    exact_origin: String,
    asset_root: PathBuf,
}

pub struct WebServer {
    state: AppState,
    bind: SocketAddr,
}

impl WebServer {
    /// Opens the authenticated boundary and encrypted credential store.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid configuration, authentication setup, or credential storage.
    pub fn new(mut config: WebServerConfig) -> Result<Self, ServerError> {
        validate_config(&config)?;
        let security = BrowserSecurity::new(
            config.exact_origin.clone(),
            &config.login_secret,
            config.session_lifetime,
            config.mutation_limit_per_second,
        )?;
        config.login_secret.fill(0);
        let credential_store =
            EncryptedCredentialStore::open(&config.credential_root, config.credential_key)?;
        Ok(Self {
            state: AppState {
                security: Arc::new(security),
                bridge: DaemonBridge {
                    socket_path: config.daemon_socket,
                    timeout: config.daemon_timeout,
                },
                credential_store: Arc::new(credential_store),
                exact_origin: config.exact_origin,
                asset_root: config.asset_root,
            },
            bind: config.bind,
        })
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/", get(app))
            .route("/login", get(login))
            .route("/auth/session", post(create_session))
            .route("/assets/app.css", get(stylesheet))
            .route("/assets/bootstrap.js", get(bootstrap_script))
            .route("/assets/agent_web.js", get(wasm_javascript))
            .route("/assets/agent_web_bg.wasm", get(wasm_binary))
            .route("/api/profiles/{profile}/commands", post(command))
            .route(
                "/api/profiles/{profile}/credentials",
                post(write_credential),
            )
            .route("/api/events/{profile}/{session}", get(events))
            .layer(DefaultBodyLimit::max(MAX_BROWSER_BODY_BYTES))
            .with_state(self.state.clone())
    }

    /// Runs the HTTP server until an operating-system shutdown signal arrives.
    ///
    /// # Errors
    ///
    /// Returns an error when the listener or HTTP server fails.
    pub async fn run(self) -> Result<(), ServerError> {
        let listener = TcpListener::bind(self.bind).await?;
        axum::serve(listener, self.router())
            .with_graceful_shutdown(shutdown_signal())
            .await?;
        Ok(())
    }

    /// Runs the HTTP server on an already-bound listener.
    ///
    /// # Errors
    ///
    /// Returns an error when the HTTP server fails.
    pub async fn serve_listener(self, listener: TcpListener) -> Result<(), ServerError> {
        axum::serve(listener, self.router()).await?;
        Ok(())
    }
}

impl ServerArguments {
    /// Parses server arguments without accepting secret values on the command line.
    ///
    /// # Errors
    ///
    /// Returns a safe error for missing, malformed, or unknown arguments.
    pub fn parse<I, S>(arguments: I) -> Result<Option<Self>, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut arguments = arguments.into_iter().map(Into::into);
        let _program = arguments.next();
        let mut bind = SocketAddr::from(([127, 0, 0, 1], 7341));
        let mut exact_origin = "http://127.0.0.1:7341".to_owned();
        let mut daemon_socket = None;
        let mut asset_root = PathBuf::from("apps/agent-web/static");
        let mut credential_root = None;
        let mut login_secret_env = "KEITH_WEB_LOGIN_SECRET".to_owned();
        let mut credential_key_env = "KEITH_CREDENTIAL_KEY".to_owned();
        while let Some(argument) = arguments.next() {
            let argument = argument
                .into_string()
                .map_err(|_| "arguments must be UTF-8".to_owned())?;
            if matches!(argument.as_str(), "--version" | "-V") {
                println!("agent-web {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--bind" => {
                    bind = value
                        .to_string_lossy()
                        .parse()
                        .map_err(|_| "invalid bind address".to_owned())?;
                }
                "--origin" => {
                    exact_origin = value
                        .into_string()
                        .map_err(|_| "origin must be UTF-8".to_owned())?;
                }
                "--socket" => daemon_socket = Some(PathBuf::from(value)),
                "--asset-root" => asset_root = PathBuf::from(value),
                "--credential-root" => credential_root = Some(PathBuf::from(value)),
                "--login-secret-env" => {
                    login_secret_env = value
                        .into_string()
                        .map_err(|_| "environment name must be UTF-8".to_owned())?;
                }
                "--credential-key-env" => {
                    credential_key_env = value
                        .into_string()
                        .map_err(|_| "environment name must be UTF-8".to_owned())?;
                }
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Some(Self {
            bind,
            exact_origin,
            daemon_socket: daemon_socket.ok_or_else(|| "--socket is required".to_owned())?,
            asset_root,
            credential_root: credential_root
                .ok_or_else(|| "--credential-root is required".to_owned())?,
            login_secret_env,
            credential_key_env,
        }))
    }

    /// Resolves the named secret environment variables into a redacted server configuration.
    ///
    /// # Errors
    ///
    /// Returns a safe error when an environment variable is missing or the key is malformed.
    pub fn load_config(self) -> Result<WebServerConfig, String> {
        let login_secret = std::env::var_os(&self.login_secret_env)
            .ok_or_else(|| format!("{} is unavailable", self.login_secret_env))?
            .into_encoded_bytes();
        let mut encoded_key = std::env::var_os(&self.credential_key_env)
            .ok_or_else(|| format!("{} is unavailable", self.credential_key_env))?
            .into_encoded_bytes();
        let decoded_key = decode_key(&encoded_key)?;
        encoded_key.fill(0);
        Ok(WebServerConfig {
            bind: self.bind,
            exact_origin: self.exact_origin,
            daemon_socket: self.daemon_socket,
            asset_root: self.asset_root,
            credential_root: self.credential_root,
            credential_key: MasterKey::from_bytes(decoded_key),
            login_secret,
            session_lifetime: Duration::from_secs(8 * 60 * 60),
            mutation_limit_per_second: 24,
            daemon_timeout: Duration::from_secs(5),
        })
    }
}

async fn login() -> Response {
    html_response(login_page().to_owned())
}

#[derive(Deserialize)]
struct LoginForm {
    password: String,
}

async fn create_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> Response {
    let mut secret = form.password.into_bytes();
    let issued = state.security.issue_for_request(&headers, &secret);
    secret.fill(0);
    match issued {
        Ok(issued) => {
            let mut response = Redirect::to("/").into_response();
            if let Ok(cookie) = HeaderValue::from_str(&issued.cookie) {
                response.headers_mut().insert(header::SET_COOKIE, cookie);
            }
            response
        }
        Err(error) => security_response(error),
    }
}

async fn app(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Ok(authenticated) = state.security.authenticate(&headers) else {
        return Redirect::to("/login").into_response();
    };
    let csrf = match state.security.csrf(authenticated) {
        Ok(csrf) => csrf,
        Err(error) => return security_response(error),
    };
    let bridge = state.bridge.clone();
    let catalog = tokio::task::spawn_blocking(move || bridge.catalog()).await;
    match catalog {
        Ok(Ok((profiles, sessions))) => html_response(shell_page(&csrf, &profiles, &sessions)),
        Ok(Err(error)) => safe_error(StatusCode::SERVICE_UNAVAILABLE, &error.to_string()),
        Err(_) => safe_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "agent connection unavailable",
        ),
    }
}

async fn stylesheet() -> Response {
    asset_response("text/css; charset=utf-8", APP_CSS.as_bytes().to_vec(), true)
}

async fn bootstrap_script() -> Response {
    asset_response(
        "text/javascript; charset=utf-8",
        BOOTSTRAP_JS.as_bytes().to_vec(),
        true,
    )
}

async fn wasm_javascript(State(state): State<AppState>) -> Response {
    file_asset(
        &state.asset_root,
        "agent_web.js",
        "text/javascript; charset=utf-8",
    )
}

async fn wasm_binary(State(state): State<AppState>) -> Response {
    file_asset(&state.asset_root, "agent_web_bg.wasm", "application/wasm")
}

async fn command(
    State(state): State<AppState>,
    Path(profile): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let Some(csrf) = headers
        .get("x-keith-csrf")
        .and_then(|value| value.to_str().ok())
    else {
        return security_response(SecurityError::Csrf);
    };
    if let Err(error) = state.security.authorize_mutation(&headers, csrf) {
        return security_response(error);
    }
    let profile: ProfileId = match profile.parse() {
        Ok(profile) => profile,
        Err(_) => return safe_error(StatusCode::BAD_REQUEST, "invalid profile scope"),
    };
    let envelope: CommandEnvelope = match serde_json::from_slice(&body) {
        Ok(envelope) => envelope,
        Err(_) => return safe_error(StatusCode::BAD_REQUEST, "invalid command envelope"),
    };
    let bridge = state.bridge.clone();
    let result =
        tokio::task::spawn_blocking(move || bridge.execute_scoped(&profile, envelope)).await;
    match result {
        Ok(Ok(result)) => Json(WireMessage::CommandResult(result)).into_response(),
        Ok(Err(BridgeError::Scope)) => safe_error(StatusCode::FORBIDDEN, "command scope denied"),
        Ok(Err(error)) => safe_error(StatusCode::SERVICE_UNAVAILABLE, &error.to_string()),
        Err(_) => safe_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "agent connection unavailable",
        ),
    }
}

#[derive(Deserialize)]
struct CredentialForm {
    csrf: String,
    provider: String,
    name: String,
    secret: String,
}

async fn write_credential(
    State(state): State<AppState>,
    Path(profile): Path<String>,
    headers: HeaderMap,
    Form(form): Form<CredentialForm>,
) -> Response {
    if let Err(error) = state.security.authorize_mutation(&headers, &form.csrf) {
        return security_response(error);
    }
    let profile: ProfileId = match profile.parse() {
        Ok(profile) => profile,
        Err(_) => return safe_error(StatusCode::BAD_REQUEST, "invalid profile scope"),
    };
    let bridge = state.bridge.clone();
    let scoped = tokio::task::spawn_blocking(move || bridge.profile_exists(&profile)).await;
    if !matches!(scoped, Ok(Ok(true))) {
        return safe_error(StatusCode::FORBIDDEN, "credential scope denied");
    }
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let policy = match CsrfToken::new(form.csrf.as_bytes()) {
        Ok(csrf) => BrowserWritePolicy {
            exact_origin: state.exact_origin.clone(),
            csrf,
            max_payload_bytes: MAX_BROWSER_BODY_BYTES,
        },
        Err(_) => return safe_error(StatusCode::BAD_REQUEST, "credential request denied"),
    };
    let Ok(reference) = CredentialRef::new(form.name, CredentialOwner::Provider(form.provider))
    else {
        return safe_error(StatusCode::BAD_REQUEST, "credential reference is invalid");
    };
    let content_length = form
        .secret
        .len()
        .saturating_add(reference.name.len())
        .saturating_add(form.csrf.len());
    let Ok(secret) = SecretValue::new(form.secret.into_bytes()) else {
        return safe_error(StatusCode::BAD_REQUEST, "credential value is invalid");
    };
    match state.credential_store.configure_from_browser(
        &policy,
        true,
        origin,
        form.csrf.as_bytes(),
        reference,
        secret,
        content_length,
        UtcTimestamp::now().unwrap_or(UtcTimestamp::UNIX_EPOCH),
    ) {
        Ok(_) => Redirect::to("/?credential=configured").into_response(),
        Err(_) => safe_error(StatusCode::BAD_REQUEST, "credential request denied"),
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
struct ResumeQuery {
    generation: Option<u64>,
    sequence: Option<u64>,
}

async fn events(
    websocket: WebSocketUpgrade,
    State(state): State<AppState>,
    Path((profile, session)): Path<(String, String)>,
    Query(resume): Query<ResumeQuery>,
    headers: HeaderMap,
) -> Response {
    if let Err(error) = state.security.authorize_read_socket(&headers) {
        return security_response(error);
    }
    let profile: ProfileId = match profile.parse() {
        Ok(profile) => profile,
        Err(_) => return safe_error(StatusCode::BAD_REQUEST, "invalid profile scope"),
    };
    let session: SessionId = match session.parse() {
        Ok(session) => session,
        Err(_) => return safe_error(StatusCode::BAD_REQUEST, "invalid session scope"),
    };
    let bridge = state.bridge.clone();
    let checked_bridge = bridge.clone();
    let checked_profile = profile.clone();
    let checked_session = session.clone();
    let scoped = tokio::task::spawn_blocking(move || {
        checked_bridge.session_in_profile(&checked_profile, &checked_session)
    })
    .await;
    if !matches!(scoped, Ok(Ok(true))) {
        return safe_error(StatusCode::FORBIDDEN, "subscription scope denied");
    }
    websocket
        .max_message_size(MAX_BROWSER_BODY_BYTES)
        .on_upgrade(move |socket| subscription_socket(socket, bridge, profile, session, resume))
}

async fn subscription_socket(
    socket: WebSocket,
    bridge: DaemonBridge,
    profile: ProfileId,
    session: SessionId,
    resume: ResumeQuery,
) {
    let (sender, mut receiver) = mpsc::channel(EVENT_QUEUE_CAPACITY);
    let (shutdown_sender, shutdown_receiver) = std::sync::mpsc::sync_channel(1);
    tokio::task::spawn_blocking(move || {
        let _ = bridge.subscribe(&profile, &session, resume, &sender, &shutdown_sender);
    });
    let (mut websocket_sender, mut websocket_receiver) = socket.split();
    loop {
        tokio::select! {
            outbound = receiver.recv() => {
                let Some(outbound) = outbound else { break; };
                if websocket_sender.send(Message::Text(outbound.into())).await.is_err() {
                    break;
                }
            }
            inbound = websocket_receiver.next() => {
                match inbound {
                    Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                    Some(Ok(Message::Ping(value))) => {
                        if websocket_sender.send(Message::Pong(value)).await.is_err() { break; }
                    }
                    Some(Ok(_)) => {
                        let _ = websocket_sender.send(Message::Close(None)).await;
                        break;
                    }
                }
            }
        }
    }
    if let Ok(stream) = shutdown_receiver.try_recv() {
        let _ = stream.shutdown(Shutdown::Both);
    }
}

#[derive(Clone)]
struct DaemonBridge {
    socket_path: PathBuf,
    timeout: Duration,
}

#[derive(Debug, Error)]
enum BridgeError {
    #[error("agent connection failed")]
    Connection(#[from] keith_connection::ConnectionError),
    #[error("agent protocol handshake failed")]
    Handshake,
    #[error("agent response was invalid")]
    Response,
    #[error("command scope was denied")]
    Scope,
    #[error("agent response serialization failed")]
    Serialize(#[from] serde_json::Error),
    #[cfg(not(unix))]
    #[error("agent connection is unsupported on this platform")]
    Unsupported,
}

#[cfg(unix)]
struct NativeClient {
    transport: FramedTransport<UnixStream>,
    shutdown: UnixStream,
    client_id: ClientId,
    protocol: keith_agent_types::ProtocolVersion,
    server_hello: keith_protocol::ServerHello,
}

impl DaemonBridge {
    fn catalog(&self) -> Result<(Vec<ProfileSummary>, Vec<SessionSummary>), BridgeError> {
        let mut client = self.connect()?;
        let sessions = client.sessions(None)?;
        let mut profiles = sessions
            .iter()
            .map(|session| session.profile_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|id| ProfileSummary {
                display_name: id.to_string(),
                id,
                enabled: true,
            })
            .collect::<Vec<_>>();
        if let Ok(authoritative) = client.profiles() {
            profiles = authoritative;
        }
        Ok((profiles, sessions))
    }

    fn profile_exists(&self, profile: &ProfileId) -> Result<bool, BridgeError> {
        Ok(self
            .connect()?
            .sessions(None)?
            .iter()
            .any(|candidate| &candidate.profile_id == profile))
    }

    fn session_in_profile(
        &self,
        profile: &ProfileId,
        session: &SessionId,
    ) -> Result<bool, BridgeError> {
        Ok(self
            .connect()?
            .sessions(Some(profile.clone()))?
            .iter()
            .any(|candidate| &candidate.session_id == session))
    }

    fn execute_scoped(
        &self,
        profile: &ProfileId,
        envelope: CommandEnvelope,
    ) -> Result<CommandResultEnvelope, BridgeError> {
        let mut client = self.connect()?;
        validate_command_scope(&mut client, profile, &envelope)?;
        client.execute(envelope)
    }

    fn subscribe(
        &self,
        profile: &ProfileId,
        session: &SessionId,
        resume: ResumeQuery,
        output: &mpsc::Sender<String>,
        shutdown_sender: &std::sync::mpsc::SyncSender<UnixStream>,
    ) -> Result<(), BridgeError> {
        let mut client = self.connect()?;
        if !client
            .sessions(Some(profile.clone()))?
            .iter()
            .any(|candidate| &candidate.session_id == session)
        {
            return Err(BridgeError::Scope);
        }
        send_bounded(
            output,
            &WireMessage::ServerHello(client.server_hello.clone()),
        )?;
        let root_tree_id = client
            .sessions(Some(profile.clone()))?
            .into_iter()
            .find(|candidate| &candidate.session_id == session)
            .ok_or(BridgeError::Scope)?
            .root_tree_id;
        let cursor = match (resume.generation, resume.sequence) {
            (Some(generation), Some(sequence)) => Some(ResumeCursor {
                root_tree_id,
                generation: keith_agent_types::Generation::new(generation),
                last_sequence: Sequence::new(sequence),
            }),
            _ => None,
        };
        let envelope = client.envelope(
            Some(session.clone()),
            ClientCommand::AttachSession(AttachSession {
                session_id: session.clone(),
                resume: cursor,
            }),
        );
        let command_id = envelope.command_id.clone();
        client.transport.send(&WireMessage::Command(envelope))?;
        loop {
            match client.transport.receive() {
                Ok(message @ WireMessage::Event(_)) => send_bounded(output, &message)?,
                Ok(WireMessage::CommandResult(result)) if result.command_id == command_id => {
                    let message = WireMessage::CommandResult(result);
                    send_bounded(output, &message)?;
                    break;
                }
                Ok(WireMessage::CommandResult(_)) => return Err(BridgeError::Response),
                Ok(_) => {}
                Err(error) if connection_timed_out(&error) => {
                    if output.is_closed() {
                        return Ok(());
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
        client
            .shutdown
            .set_read_timeout(None)
            .map_err(keith_connection::ConnectionError::from)?;
        shutdown_sender
            .send(
                client
                    .shutdown
                    .try_clone()
                    .map_err(keith_connection::ConnectionError::from)?,
            )
            .map_err(|_| BridgeError::Response)?;
        while !output.is_closed() {
            match client.transport.receive() {
                Ok(message @ WireMessage::Event(_)) => send_bounded(output, &message)?,
                Ok(_) => {}
                Err(keith_connection::ConnectionError::Closed) => return Ok(()),
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    #[cfg(unix)]
    fn connect(&self) -> Result<NativeClient, BridgeError> {
        let stream = keith_connection::connect_local(&self.socket_path)?;
        let shutdown = stream
            .try_clone()
            .map_err(keith_connection::ConnectionError::from)?;
        stream
            .set_read_timeout(Some(self.timeout))
            .map_err(keith_connection::ConnectionError::from)?;
        stream
            .set_write_timeout(Some(self.timeout))
            .map_err(keith_connection::ConnectionError::from)?;
        let mut transport = FramedTransport::new(stream, WireFormat::Json);
        let client_id = ClientId::new();
        transport.send(&WireMessage::ClientHello(ClientHello {
            protocol: CURRENT_PROTOCOL_VERSION,
            client_id: client_id.clone(),
            client_name: "agent-web".into(),
            client_version: env!("CARGO_PKG_VERSION").into(),
            supported_features: BTreeSet::from([
                Feature::SessionLifecycle,
                Feature::Branching,
                Feature::Steering,
                Feature::Goals,
                Feature::Children,
                Feature::Schedules,
                Feature::MemoryQueries,
                Feature::Confirmations,
                Feature::BackgroundControls,
                Feature::Replay,
                Feature::Snapshots,
                Feature::FramedJson,
            ]),
            resume: None,
        }))?;
        let WireMessage::ServerHello(server_hello) = transport.receive()? else {
            return Err(BridgeError::Handshake);
        };
        Ok(NativeClient {
            transport,
            shutdown,
            client_id,
            protocol: server_hello.protocol,
            server_hello,
        })
    }

    #[cfg(not(unix))]
    fn connect(&self) -> Result<NativeClient, BridgeError> {
        Err(BridgeError::Unsupported)
    }
}

#[cfg(unix)]
impl NativeClient {
    fn envelope(&self, session_id: Option<SessionId>, command: ClientCommand) -> CommandEnvelope {
        CommandEnvelope {
            protocol: self.protocol,
            command_id: CommandId::new(),
            client_id: self.client_id.clone(),
            sent_at: UtcTimestamp::now().unwrap_or(UtcTimestamp::UNIX_EPOCH),
            session_id,
            command,
        }
    }

    fn execute(
        &mut self,
        mut envelope: CommandEnvelope,
    ) -> Result<CommandResultEnvelope, BridgeError> {
        envelope.protocol = self.protocol;
        envelope.client_id.clone_from(&self.client_id);
        let command_id = envelope.command_id.clone();
        self.transport.send(&WireMessage::Command(envelope))?;
        loop {
            match self.transport.receive()? {
                WireMessage::CommandResult(result) if result.command_id == command_id => {
                    return Ok(result);
                }
                WireMessage::Event(_) => {}
                _ => return Err(BridgeError::Response),
            }
        }
    }

    fn profiles(&mut self) -> Result<Vec<ProfileSummary>, BridgeError> {
        let envelope = self.envelope(None, ClientCommand::ListProfiles);
        let result = self.execute(envelope)?;
        match result.result {
            CommandResult::Data(payload) => match *payload {
                ResponsePayload::Profiles(profiles) => Ok(profiles),
                _ => Err(BridgeError::Response),
            },
            _ => Err(BridgeError::Response),
        }
    }

    fn sessions(
        &mut self,
        profile_id: Option<ProfileId>,
    ) -> Result<Vec<SessionSummary>, BridgeError> {
        let envelope = self.envelope(
            None,
            ClientCommand::ListSessions(SessionFilter {
                profile_id,
                include_archived: true,
            }),
        );
        let result = self.execute(envelope)?;
        match result.result {
            CommandResult::Data(payload) => match *payload {
                ResponsePayload::Sessions(sessions) => Ok(sessions),
                _ => Err(BridgeError::Response),
            },
            _ => Err(BridgeError::Response),
        }
    }
}

#[cfg(not(unix))]
struct NativeClient;

#[cfg(unix)]
fn validate_command_scope(
    client: &mut NativeClient,
    profile: &ProfileId,
    envelope: &CommandEnvelope,
) -> Result<(), BridgeError> {
    if matches!(envelope.command, ClientCommand::ListProfiles) {
        return Err(BridgeError::Scope);
    }
    if let Some(command_profile) = command_profile(&envelope.command)
        && command_profile != profile
    {
        return Err(BridgeError::Scope);
    }
    let embedded_session = command_session(&envelope.command);
    if let (Some(outer), Some(inner)) = (&envelope.session_id, embedded_session)
        && outer != inner
    {
        return Err(BridgeError::Scope);
    }
    let session = embedded_session.or(envelope.session_id.as_ref());
    if command_requires_session(&envelope.command) && session.is_none() {
        return Err(BridgeError::Scope);
    }
    if let Some(session) = session
        && !client
            .sessions(Some(profile.clone()))?
            .iter()
            .any(|candidate| &candidate.session_id == session)
    {
        return Err(BridgeError::Scope);
    }
    Ok(())
}

fn command_profile(command: &ClientCommand) -> Option<&ProfileId> {
    match command {
        ClientCommand::ListSessions(filter) => filter.profile_id.as_ref(),
        ClientCommand::CreateSession(request) => Some(&request.profile_id),
        ClientCommand::CreateSchedule(request) => Some(&request.profile_id),
        ClientCommand::QueryMemory(request) => Some(&request.profile_id),
        ClientCommand::SetBackgroundControl(request) => Some(&request.profile_id),
        _ => None,
    }
}

fn command_session(command: &ClientCommand) -> Option<&SessionId> {
    match command {
        ClientCommand::AttachSession(request) => Some(&request.session_id),
        ClientCommand::DetachSession { session_id }
        | ClientCommand::ResumeSession { session_id }
        | ClientCommand::ListGoals { session_id }
        | ClientCommand::ListChildren { session_id } => Some(session_id),
        ClientCommand::BranchSession(request) => Some(&request.session_id),
        ClientCommand::SelectBranch(request) => Some(&request.session_id),
        ClientCommand::SubmitPrompt(request) => Some(&request.session_id),
        ClientCommand::Steer(request) => Some(&request.session_id),
        ClientCommand::SelectModel(request) => Some(&request.session_id),
        ClientCommand::CreateGoal(request) => Some(&request.session_id),
        ClientCommand::CreateChild(request) => Some(&request.parent_session_id),
        ClientCommand::CreateSchedule(request) => request.session_id.as_ref(),
        ClientCommand::Export(request) => Some(&request.session_id),
        ClientCommand::Cancel(keith_protocol::CancelTarget::Session(session_id)) => {
            Some(session_id)
        }
        _ => None,
    }
}

fn command_requires_session(command: &ClientCommand) -> bool {
    !matches!(
        command,
        ClientCommand::ListProfiles
            | ClientCommand::ListSessions(_)
            | ClientCommand::CreateSession(_)
            | ClientCommand::CreateSchedule(_)
            | ClientCommand::QueryMemory(_)
            | ClientCommand::SetBackgroundControl(_)
    )
}

fn send_bounded(output: &mpsc::Sender<String>, message: &WireMessage) -> Result<(), BridgeError> {
    let encoded = serde_json::to_string(message)?;
    output.try_send(encoded).map_err(|_| BridgeError::Response)
}

fn connection_timed_out(error: &keith_connection::ConnectionError) -> bool {
    match error {
        keith_connection::ConnectionError::Io(error)
        | keith_connection::ConnectionError::Frame(FrameError::Io(error)) => matches!(
            error.kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ),
        _ => false,
    }
}

fn validate_config(config: &WebServerConfig) -> Result<(), ServerError> {
    let origin = Url::parse(&config.exact_origin)
        .map_err(|_| ServerError::Configuration("exact origin is invalid".into()))?;
    if !matches!(origin.scheme(), "http" | "https")
        || origin.host_str().is_none()
        || origin.username() != ""
        || origin.password().is_some()
        || origin.query().is_some()
        || origin.fragment().is_some()
        || origin.path() != "/"
        || config.login_secret.is_empty()
        || config.session_lifetime.is_zero()
        || config.daemon_timeout.is_zero()
    {
        return Err(ServerError::Configuration(
            "origin, secrets, or limits are invalid".into(),
        ));
    }
    Ok(())
}

fn decode_key(encoded: &[u8]) -> Result<[u8; 32], String> {
    if encoded.len() != 64 {
        return Err("credential key must be 64 hexadecimal characters".into());
    }
    let mut key = [0_u8; 32];
    for (index, pair) in encoded.chunks_exact(2).enumerate() {
        let high = hex_digit(pair[0])?;
        let low = hex_digit(pair[1])?;
        key[index] = (high << 4) | low;
    }
    Ok(key)
}

fn hex_digit(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err("credential key must be hexadecimal".into()),
    }
}

fn html_response(html: String) -> Response {
    let mut response = Html(html).into_response();
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self' ws: wss:; style-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
        ),
    );
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn file_asset(root: &FsPath, filename: &str, media_type: &'static str) -> Response {
    match std::fs::read(root.join(filename)) {
        Ok(bytes) => asset_response(media_type, bytes, false),
        Err(_) => safe_error(StatusCode::NOT_FOUND, "asset unavailable"),
    }
}

fn asset_response(media_type: &'static str, bytes: Vec<u8>, no_store: bool) -> Response {
    let mut response = bytes.into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(media_type));
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(if no_store {
            "no-store"
        } else {
            "public, max-age=31536000, immutable"
        }),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn security_response(error: SecurityError) -> Response {
    let status = match error {
        SecurityError::Authentication => StatusCode::UNAUTHORIZED,
        SecurityError::Origin | SecurityError::Csrf => StatusCode::FORBIDDEN,
        SecurityError::RateLimit => StatusCode::TOO_MANY_REQUESTS,
        SecurityError::Random | SecurityError::Lock => StatusCode::SERVICE_UNAVAILABLE,
    };
    safe_error(status, &error.to_string())
}

fn safe_error(status: StatusCode, message: &str) -> Response {
    (status, message.to_owned()).into_response()
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use ring::digest::{SHA256, digest};

    use super::*;

    #[test]
    fn arguments_require_secret_environment_names_not_secret_values() {
        let parsed = ServerArguments::parse([
            "agent-web",
            "--socket",
            "/tmp/agent.sock",
            "--credential-root",
            "/tmp/credentials",
            "--login-secret-env",
            "TEST_LOGIN",
            "--credential-key-env",
            "TEST_KEY",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(parsed.login_secret_env, "TEST_LOGIN");
        assert_eq!(parsed.credential_key_env, "TEST_KEY");
        assert!(!format!("{parsed:?}").contains("secret-value"));
        assert!(ServerArguments::parse(["agent-web", "--socket", "/tmp/agent.sock"]).is_err());
    }

    #[test]
    fn command_scope_requires_matching_profile_and_session() {
        let profile = ProfileId::new();
        let other = ProfileId::new();
        let session = SessionId::new();
        let command = ClientCommand::SubmitPrompt(keith_protocol::SubmitPrompt {
            session_id: session.clone(),
            text: "hello".into(),
            delivery: keith_protocol::DeliveryPolicy::Immediate,
            reply_route: None,
        });
        assert_eq!(command_session(&command), Some(&session));
        assert!(command_profile(&command).is_none());
        let query = ClientCommand::QueryMemory(keith_protocol::MemoryQuery {
            profile_id: other.clone(),
            query: "term".into(),
            limit: 10,
        });
        assert_eq!(command_profile(&query), Some(&other));
        assert_ne!(command_profile(&query), Some(&profile));
    }

    #[test]
    fn configuration_debug_and_key_errors_do_not_expose_secrets() {
        let config = WebServerConfig {
            bind: "127.0.0.1:7341".parse().unwrap(),
            exact_origin: "http://127.0.0.1:7341".into(),
            daemon_socket: "/tmp/daemon.sock".into(),
            asset_root: "/tmp/assets".into(),
            credential_root: "/tmp/credentials".into(),
            credential_key: MasterKey::from_bytes([3; 32]),
            login_secret: b"diagnostic-secret".to_vec(),
            session_lifetime: Duration::from_secs(30),
            mutation_limit_per_second: 4,
            daemon_timeout: Duration::from_secs(1),
        };
        assert!(!format!("{config:?}").contains("diagnostic-secret"));
        assert!(decode_key(b"not-a-key").unwrap_err().contains("64"));
    }

    #[test]
    fn asset_names_are_fixed_and_credentials_never_enter_bootstrap() {
        assert!(!BOOTSTRAP_JS.contains("localStorage"));
        assert!(!BOOTSTRAP_JS.contains("sessionStorage"));
        assert!(!BOOTSTRAP_JS.contains("token"));
        let digest = digest(&SHA256, BOOTSTRAP_JS.as_bytes());
        assert_eq!(digest.as_ref().len(), 32);
    }
}
