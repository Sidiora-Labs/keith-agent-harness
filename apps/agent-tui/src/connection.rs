use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use keith_agent_types::{CURRENT_PROTOCOL_VERSION, ClientId, ProtocolVersion};
use keith_connection::{AgentTransport, FramedTransport, WebSocketTransport};
use keith_protocol::{
    ClientHello, CommandEnvelope, CommandResultEnvelope, Feature, ResumeCursor, ServerHello,
    WireFormat, WireMessage,
};
use thiserror::Error;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use url::Url;

const MAX_REMOTE_MESSAGE_BYTES: usize = 8 * 1_024 * 1_024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SupervisedLocalConfig {
    pub daemon_executable: PathBuf,
    pub worker_executable: PathBuf,
    pub data_root: PathBuf,
    pub socket_path: PathBuf,
    pub idle_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ConnectionMode {
    Attach { socket_path: PathBuf },
    SupervisedLocal(SupervisedLocalConfig),
    Remote { url: Url, bearer_token: String },
}

#[derive(Debug, Error)]
pub enum TuiConnectionError {
    #[error("connection setup is invalid: {0}")]
    Invalid(String),
    #[error("agent transport failed: {0}")]
    Transport(#[from] keith_connection::ConnectionError),
    #[error("supervised daemon failed to start: {0}")]
    DaemonStart(String),
    #[error("remote WebSocket failed: {0}")]
    WebSocket(#[from] tungstenite::Error),
    #[error("remote authorization header is invalid")]
    InvalidAuthorization,
    #[error("server did not return a protocol hello")]
    MissingServerHello,
    #[error("server returned a message for another command")]
    UnexpectedCommandResult,
}

type BoxedTransport = Box<dyn AgentTransport + Send>;

pub struct AgentConnectionClient {
    mode: ConnectionMode,
    transport: BoxedTransport,
    supervised_daemon: Option<Child>,
    pub client_id: ClientId,
    pub server: ServerHello,
}

impl AgentConnectionClient {
    /// Opens and negotiates a bounded local, supervised-local, or authenticated remote connection.
    ///
    /// # Errors
    ///
    /// Returns an error when startup, authentication, transport, or negotiation fails.
    pub fn connect(
        mode: ConnectionMode,
        client_id: ClientId,
        resume: Option<ResumeCursor>,
        startup_timeout: Duration,
    ) -> Result<Self, TuiConnectionError> {
        let (mut transport, supervised_daemon) = open_transport(&mode, startup_timeout)?;
        transport.send(&WireMessage::ClientHello(client_hello(
            client_id.clone(),
            resume,
        )))?;
        let WireMessage::ServerHello(server) = transport.receive()? else {
            return Err(TuiConnectionError::MissingServerHello);
        };
        Ok(Self {
            mode,
            transport,
            supervised_daemon,
            client_id,
            server,
        })
    }

    /// Reopens the same endpoint with the same client identity and a durable resume cursor.
    ///
    /// # Errors
    ///
    /// Returns an error when the replacement connection cannot negotiate.
    pub fn reconnect(
        &mut self,
        resume: Option<ResumeCursor>,
        startup_timeout: Duration,
    ) -> Result<(), TuiConnectionError> {
        let mut replacement = Self::connect(
            self.mode.clone(),
            self.client_id.clone(),
            resume,
            startup_timeout,
        )?;
        std::mem::swap(self, &mut replacement);
        Ok(())
    }

    /// Sends one command and consumes ordered events until its matching result arrives.
    ///
    /// # Errors
    ///
    /// Returns an error for transport closure or a mismatched result.
    pub fn execute(
        &mut self,
        mut command: CommandEnvelope,
        mut on_message: impl FnMut(WireMessage),
    ) -> Result<CommandResultEnvelope, TuiConnectionError> {
        command.protocol = self.server.protocol;
        let command_id = command.command_id.clone();
        self.transport.send(&WireMessage::Command(command))?;
        loop {
            match self.transport.receive()? {
                WireMessage::CommandResult(result) if result.command_id == command_id => {
                    return Ok(result);
                }
                WireMessage::CommandResult(_) => {
                    return Err(TuiConnectionError::UnexpectedCommandResult);
                }
                message @ WireMessage::Event(_) => on_message(message),
                WireMessage::ServerHello(_)
                | WireMessage::ClientHello(_)
                | WireMessage::Command(_) => {}
            }
        }
    }

    pub const fn protocol(&self) -> ProtocolVersion {
        self.server.protocol
    }
}

impl Drop for AgentConnectionClient {
    fn drop(&mut self) {
        if let Some(child) = &mut self.supervised_daemon {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn open_transport(
    mode: &ConnectionMode,
    startup_timeout: Duration,
) -> Result<(BoxedTransport, Option<Child>), TuiConnectionError> {
    match mode {
        ConnectionMode::Attach { socket_path } => Ok((open_local(socket_path)?, None)),
        ConnectionMode::SupervisedLocal(config) => {
            if startup_timeout.is_zero() {
                return Err(TuiConnectionError::Invalid(
                    "startup timeout must be non-zero".into(),
                ));
            }
            std::fs::create_dir_all(&config.data_root)
                .map_err(|error| TuiConnectionError::DaemonStart(error.to_string()))?;
            let mut child = Command::new(&config.daemon_executable)
                .arg("--data-root")
                .arg(&config.data_root)
                .arg("--socket")
                .arg(&config.socket_path)
                .arg("--worker-executable")
                .arg(&config.worker_executable)
                .arg("--idle-seconds")
                .arg(config.idle_seconds.to_string())
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|error| TuiConnectionError::DaemonStart(error.to_string()))?;
            let deadline = Instant::now() + startup_timeout;
            loop {
                if config.socket_path.exists() {
                    match open_local(&config.socket_path) {
                        Ok(transport) => return Ok((transport, Some(child))),
                        Err(error) if Instant::now() < deadline => {
                            let _ = error;
                        }
                        Err(error) => {
                            let _ = child.kill();
                            let _ = child.wait();
                            return Err(error);
                        }
                    }
                }
                if let Some(status) = child
                    .try_wait()
                    .map_err(|error| TuiConnectionError::DaemonStart(error.to_string()))?
                {
                    return Err(TuiConnectionError::DaemonStart(format!(
                        "daemon exited with {status}"
                    )));
                }
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(TuiConnectionError::DaemonStart(
                        "daemon startup timed out".into(),
                    ));
                }
                thread::sleep(Duration::from_millis(20));
            }
        }
        ConnectionMode::Remote { url, bearer_token } => {
            if !matches!(url.scheme(), "ws" | "wss")
                || bearer_token.is_empty()
                || bearer_token.chars().any(char::is_control)
            {
                return Err(TuiConnectionError::Invalid(
                    "remote mode requires ws/wss and a non-empty bearer token".into(),
                ));
            }
            let mut request = url
                .as_str()
                .into_client_request()
                .map_err(TuiConnectionError::WebSocket)?;
            let authorization = HeaderValue::from_str(&format!("Bearer {bearer_token}"))
                .map_err(|_| TuiConnectionError::InvalidAuthorization)?;
            request.headers_mut().insert("authorization", authorization);
            let (socket, _) = tungstenite::connect(request)?;
            Ok((
                Box::new(WebSocketTransport::new(
                    socket,
                    WireFormat::Json,
                    MAX_REMOTE_MESSAGE_BYTES,
                )),
                None,
            ))
        }
    }
}

#[cfg(unix)]
fn open_local(path: &Path) -> Result<BoxedTransport, TuiConnectionError> {
    let stream = keith_connection::connect_local(path)?;
    Ok(Box::new(FramedTransport::new(stream, WireFormat::Json)))
}

#[cfg(not(unix))]
fn open_local(_path: &Path) -> Result<BoxedTransport, TuiConnectionError> {
    Err(TuiConnectionError::Invalid(
        "this build has no platform local transport".into(),
    ))
}

fn client_hello(client_id: ClientId, resume: Option<ResumeCursor>) -> ClientHello {
    ClientHello {
        protocol: CURRENT_PROTOCOL_VERSION,
        client_id,
        client_name: "agent-tui".into(),
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
            Feature::Export,
            Feature::BackgroundControls,
            Feature::Replay,
            Feature::Snapshots,
            Feature::FramedJson,
            Feature::WebSocket,
        ]),
        resume,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TuiArguments {
    pub mode: ConnectionMode,
    pub session_id: Option<keith_agent_types::SessionId>,
    pub color_mode: crate::ColorMode,
    pub reduced_motion: bool,
    pub startup_timeout: Duration,
}

impl TuiArguments {
    /// # Errors
    ///
    /// Returns a safe message for unknown, missing, or incompatible arguments.
    #[allow(clippy::too_many_lines)]
    pub fn parse<I, S>(arguments: I) -> Result<Option<Self>, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut arguments = arguments.into_iter().map(Into::into);
        let program = arguments
            .next()
            .unwrap_or_else(|| OsString::from("agent-tui"));
        let mut socket = None;
        let mut remote = None;
        let mut token_env = None;
        let mut data_root = None;
        let mut daemon = None;
        let mut worker = None;
        let mut session_id = None;
        let mut color_mode = crate::ColorMode::TrueColor;
        let mut reduced_motion = false;
        let mut startup_timeout = Duration::from_secs(10);
        while let Some(argument) = arguments.next() {
            let argument = argument
                .into_string()
                .map_err(|_| "arguments must be UTF-8".to_owned())?;
            if matches!(argument.as_str(), "--version" | "-V") {
                println!("agent-tui {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            if argument == "--reduced-motion" {
                reduced_motion = true;
                continue;
            }
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--socket" => socket = Some(PathBuf::from(value)),
                "--remote" => {
                    remote = Some(
                        value
                            .into_string()
                            .map_err(|_| "remote URL must be UTF-8".to_owned())?,
                    );
                }
                "--token-env" => {
                    token_env = Some(
                        value
                            .into_string()
                            .map_err(|_| "token environment name must be UTF-8".to_owned())?,
                    );
                }
                "--data-root" => data_root = Some(PathBuf::from(value)),
                "--daemon-executable" => daemon = Some(PathBuf::from(value)),
                "--worker-executable" => worker = Some(PathBuf::from(value)),
                "--session" => {
                    let value = value
                        .into_string()
                        .map_err(|_| "session ID must be UTF-8".to_owned())?;
                    session_id = Some(value.parse().map_err(|_| "invalid session ID".to_owned())?);
                }
                "--color" => {
                    color_mode = match value.to_string_lossy().as_ref() {
                        "truecolor" => crate::ColorMode::TrueColor,
                        "256" => crate::ColorMode::Ansi256,
                        "none" => crate::ColorMode::NoColor,
                        "contrast" => crate::ColorMode::HighContrast,
                        _ => return Err("color must be truecolor, 256, none, or contrast".into()),
                    };
                }
                "--startup-timeout-ms" => {
                    let millis = value
                        .to_string_lossy()
                        .parse::<u64>()
                        .map_err(|_| "startup timeout must be an integer".to_owned())?;
                    startup_timeout = Duration::from_millis(millis);
                }
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        if startup_timeout.is_zero() {
            return Err("startup timeout must be non-zero".into());
        }
        let mode = if let Some(remote) = remote {
            if socket.is_some() || data_root.is_some() {
                return Err("remote mode cannot be combined with local modes".into());
            }
            let token_env =
                token_env.ok_or_else(|| "--token-env is required remotely".to_owned())?;
            let bearer_token = std::env::var(&token_env)
                .map_err(|_| format!("token environment variable {token_env} is unavailable"))?;
            ConnectionMode::Remote {
                url: Url::parse(&remote).map_err(|_| "invalid remote URL".to_owned())?,
                bearer_token,
            }
        } else if let Some(data_root) = data_root {
            let socket_path = socket.unwrap_or_else(|| data_root.join("agentd.sock"));
            let daemon_executable = daemon.unwrap_or_else(|| sibling_binary(&program, "agentd"));
            let worker_executable =
                worker.unwrap_or_else(|| sibling_binary(&program, "agent-worker"));
            ConnectionMode::SupervisedLocal(SupervisedLocalConfig {
                daemon_executable,
                worker_executable,
                data_root,
                socket_path,
                idle_seconds: 15 * 60,
            })
        } else {
            ConnectionMode::Attach {
                socket_path: socket.ok_or_else(|| {
                    "--socket, --data-root, or --remote must select a connection mode".to_owned()
                })?,
            }
        };
        Ok(Some(Self {
            mode,
            session_id,
            color_mode,
            reduced_motion,
            startup_timeout,
        }))
    }
}

fn sibling_binary(program: &OsString, name: &str) -> PathBuf {
    let mut path = PathBuf::from(program);
    path.set_file_name(name);
    path
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    use keith_agent_types::{
        CommandId, CommonError, EntityId, ErrorCode, Generation, RootTreeId, Sequence, UtcTimestamp,
    };
    use keith_protocol::{
        ClientCommand, CommandResult, DaemonEvent, EventEnvelope, ResponsePayload, SessionFilter,
        negotiate,
    };

    use super::*;

    fn server_journey(transport: &mut impl AgentTransport) {
        let WireMessage::ClientHello(hello) = transport.receive().unwrap() else {
            panic!("client hello required");
        };
        let server = negotiate(
            &hello,
            CURRENT_PROTOCOL_VERSION,
            EntityId::new(),
            &hello.supported_features,
        )
        .unwrap();
        transport.send(&WireMessage::ServerHello(server)).unwrap();
        let WireMessage::Command(command) = transport.receive().unwrap() else {
            panic!("command required");
        };
        transport
            .send(&WireMessage::Event(EventEnvelope {
                protocol: CURRENT_PROTOCOL_VERSION,
                root_tree_id: RootTreeId::new(),
                generation: Generation::new(1),
                first_sequence: Sequence::new(1),
                sequence: Sequence::new(1),
                occurred_at: UtcTimestamp::UNIX_EPOCH,
                event: DaemonEvent::Warning(CommonError::new(
                    ErrorCode::Unavailable,
                    "recovered event",
                    true,
                )),
            }))
            .unwrap();
        transport
            .send(&WireMessage::CommandResult(CommandResultEnvelope {
                protocol: CURRENT_PROTOCOL_VERSION,
                command_id: command.command_id,
                completed_at: UtcTimestamp::UNIX_EPOCH,
                result: CommandResult::Data(Box::new(ResponsePayload::Sessions(Vec::new()))),
            }))
            .unwrap();
    }

    fn list_command(client_id: ClientId) -> CommandEnvelope {
        CommandEnvelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            command_id: CommandId::new(),
            client_id,
            sent_at: UtcTimestamp::UNIX_EPOCH,
            session_id: None,
            command: ClientCommand::ListSessions(SessionFilter::default()),
        }
    }

    #[test]
    #[cfg(unix)]
    fn local_attach_and_reconnect_run_the_real_protocol_twice() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("agent.sock");
        let listener = keith_connection::bind_permissioned_local(&path).unwrap();
        let server = thread::spawn(move || {
            for _ in 0..2 {
                let (stream, _) = listener.accept().unwrap();
                server_journey(&mut FramedTransport::new(stream, WireFormat::Json));
            }
        });
        let client_id = ClientId::new();
        let mut client = AgentConnectionClient::connect(
            ConnectionMode::Attach { socket_path: path },
            client_id.clone(),
            None,
            Duration::from_secs(1),
        )
        .unwrap();
        let mut recovered = Vec::new();
        let result = client
            .execute(list_command(client_id.clone()), |message| {
                recovered.push(message);
            })
            .unwrap();
        assert!(matches!(
            recovered.as_slice(),
            [WireMessage::Event(EventEnvelope {
                event: DaemonEvent::Warning(error),
                ..
            })] if error.message == "recovered event"
        ));
        assert!(matches!(
            result.result,
            CommandResult::Data(payload)
                if matches!(*payload, ResponsePayload::Sessions(ref sessions) if sessions.is_empty())
        ));
        client.reconnect(None, Duration::from_secs(1)).unwrap();
        client.execute(list_command(client_id), |_| {}).unwrap();
        drop(client);
        server.join().unwrap();
    }

    #[test]
    fn remote_connection_sends_bearer_header_without_url_credentials() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let observed = Arc::new(Mutex::new(None));
        let server_observed = Arc::clone(&observed);
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let callback =
                |request: &tungstenite::handshake::server::Request,
                 response: tungstenite::handshake::server::Response| {
                    let header = request
                        .headers()
                        .get("authorization")
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_owned();
                    *server_observed.lock().unwrap() = Some((request.uri().to_string(), header));
                    Ok(response)
                };
            let socket = tungstenite::accept_hdr(stream, callback).unwrap();
            let mut transport =
                WebSocketTransport::new(socket, WireFormat::Json, MAX_REMOTE_MESSAGE_BYTES);
            let WireMessage::ClientHello(hello) = transport.receive().unwrap() else {
                panic!("client hello required");
            };
            let response = negotiate(
                &hello,
                CURRENT_PROTOCOL_VERSION,
                EntityId::new(),
                &hello.supported_features,
            )
            .unwrap();
            transport.send(&WireMessage::ServerHello(response)).unwrap();
        });
        let url = Url::parse(&format!("ws://{address}/agent")).unwrap();
        let client = AgentConnectionClient::connect(
            ConnectionMode::Remote {
                url,
                bearer_token: "remote-secret".into(),
            },
            ClientId::new(),
            None,
            Duration::from_secs(1),
        )
        .unwrap();
        drop(client);
        server.join().unwrap();
        let (uri, authorization) = observed.lock().unwrap().clone().unwrap();
        assert_eq!(uri, "/agent");
        assert_eq!(authorization, "Bearer remote-secret");
        assert!(!uri.contains("remote-secret"));
    }

    #[test]
    fn argument_modes_and_accessibility_are_explicit() {
        let attached = TuiArguments::parse([
            "agent-tui",
            "--socket",
            "/tmp/keith.sock",
            "--session",
            &keith_agent_types::SessionId::new().to_string(),
            "--color",
            "none",
            "--reduced-motion",
        ])
        .unwrap()
        .unwrap();
        assert!(matches!(attached.mode, ConnectionMode::Attach { .. }));
        assert_eq!(attached.color_mode, crate::ColorMode::NoColor);
        assert!(attached.reduced_motion);

        let supervised = TuiArguments::parse([
            "agent-tui",
            "--data-root",
            "/tmp/keith-state",
            "--daemon-executable",
            "/bin/agentd",
            "--worker-executable",
            "/bin/agent-worker",
        ])
        .unwrap()
        .unwrap();
        assert!(matches!(
            supervised.mode,
            ConnectionMode::SupervisedLocal(_)
        ));
        assert!(TuiArguments::parse(["agent-tui", "--remote", "ws://localhost"]).is_err());
    }
}
