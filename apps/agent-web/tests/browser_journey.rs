#![cfg(unix)]

use std::fs::OpenOptions;
use std::io::Write as _;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use keith_agent_types::{
    CURRENT_PROTOCOL_VERSION, EntityId, Generation, MessageId, ProfileId, Revision, RootTreeId,
    Sequence, SessionId, UtcTimestamp, WorkspaceId,
};
use keith_agent_web::{WebServer, WebServerConfig};
use keith_connection::{AgentTransport, FramedTransport};
use keith_credentials::MasterKey;
use keith_framing::FrameError;
use keith_protocol::{
    ClientCommand, CommandResult, CommandResultEnvelope, DaemonEvent, EventEnvelope,
    MessageProjection, MessageRole, PresenceProjection, PresenceState, ProfileSummary,
    ResponsePayload, SessionSnapshot, SessionState, SessionSummary, WireFormat, WireMessage,
    negotiate,
};

struct ProtocolHost {
    stop: Arc<AtomicBool>,
    manager: Option<thread::JoinHandle<()>>,
}

impl Drop for ProtocolHost {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(manager) = self.manager.take() {
            manager.join().unwrap();
        }
    }
}

#[test]
#[ignore = "requires KEITH_PLAYWRIGHT_MODULE pointing to an installed Playwright package"]
fn chromium_navigation_refresh_reconnect_restart_and_mutations_are_real() {
    let playwright = std::env::var_os("KEITH_PLAYWRIGHT_MODULE")
        .expect("KEITH_PLAYWRIGHT_MODULE must point to the installed Playwright package");
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("agent.sock");
    let control = directory.path().join("upstream.control");
    let command_log = directory.path().join("commands.jsonl");
    std::fs::write(&control, b"up").unwrap();
    std::fs::write(&command_log, b"").unwrap();
    let profile = ProfileId::new();
    let session = SessionId::new();
    let root = RootTreeId::new();
    let _host = start_protocol_host(
        socket.clone(),
        control.clone(),
        command_log.clone(),
        profile.clone(),
        session.clone(),
        root,
    );
    wait_for_path(&socket);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    let listener = runtime
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("http://{address}");
    let credentials = directory.path().join("credentials");
    let server = WebServer::new(WebServerConfig {
        bind: address,
        exact_origin: origin.clone(),
        daemon_socket: socket,
        asset_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static"),
        credential_root: credentials.clone(),
        credential_key: MasterKey::from_bytes([0x42; 32]),
        login_secret: b"browser-login-secret".to_vec(),
        session_lifetime: Duration::from_secs(60),
        mutation_limit_per_second: 64,
        daemon_timeout: Duration::from_secs(2),
        openai_compatibility: None,
        platform_compatibility: None,
    })
    .unwrap();
    let task = runtime.spawn(server.serve_listener(listener));

    let status = Command::new("node")
        .arg(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests")
                .join("browser_journey.cjs"),
        )
        .env("KEITH_PLAYWRIGHT_MODULE", playwright)
        .env("KEITH_WEB_ORIGIN", &origin)
        .env("KEITH_UPSTREAM_CONTROL", &control)
        .env("KEITH_COMMAND_LOG", &command_log)
        .status()
        .unwrap();
    task.abort();
    let _ = runtime.block_on(task);
    assert!(status.success());

    let commands = std::fs::read_to_string(command_log).unwrap();
    for expected in [
        "Update durable memory",
        "create_schedule",
        "configured-channel",
        "guarded refinement",
    ] {
        assert!(
            commands.contains(expected),
            "missing browser command {expected}"
        );
    }
    let secret = b"browser-credential-must-stay-write-only";
    for entry in std::fs::read_dir(credentials).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_file() {
            let bytes = std::fs::read(entry.path()).unwrap();
            assert!(!bytes.windows(secret.len()).any(|window| window == secret));
        }
    }
}

fn start_protocol_host(
    socket: PathBuf,
    control: PathBuf,
    command_log: PathBuf,
    profile: ProfileId,
    session: SessionId,
    root: RootTreeId,
) -> ProtocolHost {
    let stop = Arc::new(AtomicBool::new(false));
    let manager_stop = Arc::clone(&stop);
    let manager = thread::spawn(move || {
        let mut listener = None;
        while !manager_stop.load(Ordering::Acquire) {
            let enabled = std::fs::read(&control).is_ok_and(|value| value == b"up");
            if enabled && listener.is_none() {
                let _ = std::fs::remove_file(&socket);
                let created = UnixListener::bind(&socket).unwrap();
                created.set_nonblocking(true).unwrap();
                listener = Some(created);
            } else if !enabled && listener.is_some() {
                listener = None;
                let _ = std::fs::remove_file(&socket);
            }
            if let Some(active) = &listener {
                match active.accept() {
                    Ok((stream, _)) => {
                        let control = control.clone();
                        let command_log = command_log.clone();
                        let profile = profile.clone();
                        let session = session.clone();
                        let root = root.clone();
                        thread::spawn(move || {
                            serve_protocol_connection(
                                stream,
                                &control,
                                &command_log,
                                &profile,
                                &session,
                                &root,
                            );
                        });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(error) => panic!("protocol listener failed: {error}"),
                }
            }
            thread::sleep(Duration::from_millis(10));
        }
        drop(listener);
        let _ = std::fs::remove_file(&socket);
    });
    ProtocolHost {
        stop,
        manager: Some(manager),
    }
}

#[allow(clippy::too_many_lines)]
fn serve_protocol_connection(
    stream: UnixStream,
    control: &Path,
    command_log: &Path,
    profile: &ProfileId,
    session: &SessionId,
    root: &RootTreeId,
) {
    stream
        .set_read_timeout(Some(Duration::from_millis(100)))
        .unwrap();
    let mut transport = FramedTransport::new(stream, WireFormat::Json);
    let hello = loop {
        match transport.receive() {
            Ok(WireMessage::ClientHello(hello)) => break hello,
            Err(error) if timed_out(&error) => {}
            Ok(_) | Err(_) => return,
        }
    };
    let server = negotiate(
        &hello,
        CURRENT_PROTOCOL_VERSION,
        EntityId::new(),
        &hello.supported_features,
    )
    .unwrap();
    transport.send(&WireMessage::ServerHello(server)).unwrap();
    loop {
        if std::fs::read(control).is_ok_and(|value| value == b"down") {
            return;
        }
        let envelope = match transport.receive() {
            Ok(WireMessage::Command(envelope)) => envelope,
            Ok(_) => continue,
            Err(error) if timed_out(&error) => continue,
            Err(_) => return,
        };
        let command_id = envelope.command_id.clone();
        match envelope.command {
            ClientCommand::ListProfiles => send_result(
                &mut transport,
                command_id,
                CommandResult::Data(Box::new(ResponsePayload::Profiles(vec![ProfileSummary {
                    id: profile.clone(),
                    workspace_id: WorkspaceId::new(),
                    display_name: "Browser profile".into(),
                    enabled: true,
                }]))),
            ),
            ClientCommand::ListSessions(filter) => {
                let sessions = if filter
                    .profile_id
                    .as_ref()
                    .is_none_or(|requested| requested == profile)
                {
                    vec![session_summary(profile, session, root)]
                } else {
                    Vec::new()
                };
                send_result(
                    &mut transport,
                    command_id,
                    CommandResult::Data(Box::new(ResponsePayload::Sessions(sessions))),
                );
            }
            ClientCommand::AttachSession(request) if request.session_id == *session => {
                send_result(
                    &mut transport,
                    command_id,
                    CommandResult::Data(Box::new(ResponsePayload::Snapshot(Box::new(snapshot(
                        profile, session, root,
                    ))))),
                );
                let message_id = MessageId::new();
                send_event(
                    &mut transport,
                    root,
                    Sequence::new(1),
                    DaemonEvent::AssistantDelta {
                        message_id: message_id.clone(),
                        text: "streamed ".into(),
                    },
                );
                send_event(
                    &mut transport,
                    root,
                    Sequence::new(2),
                    DaemonEvent::MessageCommitted(MessageProjection {
                        message_id,
                        final_id: None,
                        role: MessageRole::Assistant,
                        text: "streamed response".into(),
                        committed: true,
                    }),
                );
            }
            command => {
                let encoded = serde_json::to_string(&command).unwrap();
                let mut log = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(command_log)
                    .unwrap();
                writeln!(log, "{encoded}").unwrap();
                send_result(
                    &mut transport,
                    command_id,
                    CommandResult::Accepted { action_id: None },
                );
            }
        }
    }
}

fn send_result(
    transport: &mut impl AgentTransport,
    command_id: keith_agent_types::CommandId,
    result: CommandResult,
) {
    transport
        .send(&WireMessage::CommandResult(CommandResultEnvelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            command_id,
            completed_at: UtcTimestamp::UNIX_EPOCH,
            result,
        }))
        .unwrap();
}

fn send_event(
    transport: &mut impl AgentTransport,
    root: &RootTreeId,
    sequence: Sequence,
    event: DaemonEvent,
) {
    transport
        .send(&WireMessage::Event(EventEnvelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            root_tree_id: root.clone(),
            generation: Generation::new(1),
            first_sequence: sequence,
            sequence,
            occurred_at: UtcTimestamp::UNIX_EPOCH,
            event,
        }))
        .unwrap();
}

fn session_summary(profile: &ProfileId, session: &SessionId, root: &RootTreeId) -> SessionSummary {
    SessionSummary {
        session_id: session.clone(),
        root_tree_id: root.clone(),
        profile_id: profile.clone(),
        title: Some("Browser journey".into()),
        state: SessionState::Ready,
        updated_at: UtcTimestamp::UNIX_EPOCH,
    }
}

fn snapshot(profile: &ProfileId, session: &SessionId, root: &RootTreeId) -> SessionSnapshot {
    SessionSnapshot {
        session: session_summary(profile, session, root),
        generation: Generation::new(1),
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
        presence: PresenceProjection {
            session_id: session.clone(),
            goal_id: None,
            state: PresenceState::Available,
            updated_at: UtcTimestamp::UNIX_EPOCH,
            next_wake: None,
            safe_error: None,
        },
        terminal: None,
        revision: Revision::ZERO,
    }
}

fn timed_out(error: &keith_connection::ConnectionError) -> bool {
    match error {
        keith_connection::ConnectionError::Io(error)
        | keith_connection::ConnectionError::Frame(FrameError::Io(error)) => matches!(
            error.kind(),
            std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
        ),
        _ => false,
    }
}

fn wait_for_path(path: &Path) {
    for _ in 0..200 {
        if path.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("protocol socket did not become ready");
}
