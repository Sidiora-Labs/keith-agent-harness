use std::collections::BTreeSet;
use std::fs;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use keith_agent_types::{
    CURRENT_PROTOCOL_VERSION, CURRENT_SCHEMA_VERSION, ClientId, CommandId, ProfileId, RootTreeId,
    SessionId, UtcTimestamp,
};
use keith_connection::{AgentTransport, FramedTransport};
use keith_daemon_core::RootManifest;
use keith_protocol::{
    AttachSession, ClientCommand, ClientHello, CommandEnvelope, CommandResult, ResponsePayload,
    SessionFilter, SessionState, WireFormat, WireMessage,
};
use keith_worker_runtime::{WorkerRunState, read_registration, registration_path};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

fn write_manifest(data_root: &Path, root: &RootTreeId, session: &SessionId) {
    let directory = data_root.join("sessions").join(root.to_string());
    fs::create_dir_all(&directory).unwrap();
    let manifest = RootManifest {
        version: CURRENT_SCHEMA_VERSION,
        root_tree_id: root.clone(),
        root_session_id: session.clone(),
        profile_id: ProfileId::new(),
        title: Some(format!("root {root}")),
        state: SessionState::Dormant,
        updated_at: UtcTimestamp::UNIX_EPOCH,
    };
    fs::write(
        directory.join("manifest.json"),
        keith_agent_types::canonical_json_bytes(&manifest).unwrap(),
    )
    .unwrap();
    fs::write(
        directory.join("session.jsonl"),
        b"corrupt session state that lazy discovery must never load",
    )
    .unwrap();
}

fn start_daemon(data_root: &Path, socket: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_agentd"))
        .arg("--data-root")
        .arg(data_root)
        .arg("--socket")
        .arg(socket)
        .arg("--worker-executable")
        .arg(env!("CARGO_BIN_EXE_keith-daemon-worker-host"))
        .arg("--idle-seconds")
        .arg("60")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap()
}

fn connect_when_ready(socket: &Path) -> UnixStream {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(stream) = UnixStream::connect(socket) {
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            return stream;
        }
        assert!(
            Instant::now() < deadline,
            "daemon socket did not become ready"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn open_connection(socket: &Path) -> (FramedTransport<UnixStream>, ClientId) {
    let mut transport = FramedTransport::new(connect_when_ready(socket), WireFormat::Json);
    let client_id = ClientId::new();
    transport
        .send(&WireMessage::ClientHello(ClientHello {
            protocol: CURRENT_PROTOCOL_VERSION,
            client_id: client_id.clone(),
            client_name: "daemon-process-test".into(),
            client_version: "1.0.0".into(),
            supported_features: BTreeSet::new(),
            resume: None,
        }))
        .unwrap();
    assert!(matches!(
        transport.receive().unwrap(),
        WireMessage::ServerHello(_)
    ));
    (transport, client_id)
}

fn execute(socket: &Path, command: ClientCommand) -> CommandResult {
    let (mut transport, client_id) = open_connection(socket);
    transport
        .send(&WireMessage::Command(CommandEnvelope {
            protocol: CURRENT_PROTOCOL_VERSION,
            command_id: CommandId::new(),
            client_id,
            sent_at: UtcTimestamp::UNIX_EPOCH,
            session_id: None,
            command,
        }))
        .unwrap();
    let WireMessage::CommandResult(result) = transport.receive().unwrap() else {
        panic!("daemon must return a command result");
    };
    result.result
}

fn wait_for_worker(data_root: &Path, root: &RootTreeId) -> u32 {
    let path = registration_path(&data_root.join("runtime"), root);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(registration) = read_registration(&path)
            && registration.state == WorkerRunState::Ready
        {
            return registration.pid;
        }
        assert!(Instant::now() < deadline, "worker did not become ready");
        thread::sleep(Duration::from_millis(10));
    }
}

fn send_signal(process: &Child, signal: Signal) {
    kill(Pid::from_raw(i32::try_from(process.id()).unwrap()), signal).unwrap();
}

fn process_is_alive(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    kill(Pid::from_raw(pid), None).is_ok()
}

#[test]
fn daemon_process_is_lazy_contains_crashes_and_adopts_after_restart() {
    let directory = tempfile::tempdir().unwrap();
    let data_root = directory.path().join("data");
    let socket = directory.path().join("agentd.sock");
    let first_root = RootTreeId::new();
    let first_session = SessionId::new();
    let second_root = RootTreeId::new();
    let second_session = SessionId::new();
    write_manifest(&data_root, &first_root, &first_session);
    write_manifest(&data_root, &second_root, &second_session);

    let mut daemon = start_daemon(&data_root, &socket);
    let _idle_connection = open_connection(&socket);
    let listed = execute(
        &socket,
        ClientCommand::ListSessions(SessionFilter::default()),
    );
    let CommandResult::Data(payload) = listed else {
        panic!("catalog listing must return data");
    };
    let ResponsePayload::Sessions(sessions) = *payload else {
        panic!("catalog listing must return sessions");
    };
    assert_eq!(sessions.len(), 2);
    assert!(!data_root.join("runtime/workers").exists());

    for session in [&first_session, &second_session] {
        assert!(matches!(
            execute(
                &socket,
                ClientCommand::AttachSession(AttachSession {
                    session_id: session.clone(),
                    resume: None,
                })
            ),
            CommandResult::Data(_)
        ));
    }
    let first_pid = wait_for_worker(&data_root, &first_root);
    let second_pid = wait_for_worker(&data_root, &second_root);

    kill(
        Pid::from_raw(i32::try_from(first_pid).unwrap()),
        Signal::SIGKILL,
    )
    .unwrap();
    thread::sleep(Duration::from_millis(150));
    assert!(matches!(
        execute(
            &socket,
            ClientCommand::ListSessions(SessionFilter::default())
        ),
        CommandResult::Data(_)
    ));
    assert!(daemon.try_wait().unwrap().is_none());
    assert!(process_is_alive(second_pid));

    send_signal(&daemon, Signal::SIGKILL);
    assert!(!daemon.wait().unwrap().success());
    assert!(process_is_alive(second_pid));

    let mut restarted = start_daemon(&data_root, &socket);
    assert!(matches!(
        execute(
            &socket,
            ClientCommand::ListSessions(SessionFilter::default())
        ),
        CommandResult::Data(_)
    ));
    assert!(process_is_alive(second_pid));

    send_signal(&restarted, Signal::SIGTERM);
    assert!(restarted.wait().unwrap().success());
    let deadline = Instant::now() + Duration::from_secs(2);
    while process_is_alive(second_pid) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!process_is_alive(second_pid));
}
