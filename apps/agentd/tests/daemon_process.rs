use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use keith_agent_types::{
    CURRENT_PROTOCOL_VERSION, CURRENT_SCHEMA_VERSION, ClientId, CommandId, ProfileId, RootTreeId,
    SessionId, UtcTimestamp,
};
use keith_connection::{
    AgentTransport, FramedTransport, LocalStream, connect_local, set_local_read_timeout,
    set_local_write_timeout,
};
use keith_daemon_core::RootManifest;
use keith_protocol::{
    AttachSession, ClientCommand, ClientHello, CommandEnvelope, CommandResult, ResponsePayload,
    SessionFilter, SessionState, WireFormat, WireMessage,
};
use keith_worker_runtime::{WorkerRunState, read_registration, registration_path};
#[cfg(unix)]
use nix::sys::signal::{Signal, kill};
#[cfg(unix)]
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

fn connect_when_ready(socket: &Path) -> LocalStream {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(stream) = connect_local(socket) {
            set_local_read_timeout(&stream, Some(Duration::from_secs(2))).unwrap();
            set_local_write_timeout(&stream, Some(Duration::from_secs(2))).unwrap();
            return stream;
        }
        assert!(
            Instant::now() < deadline,
            "daemon socket did not become ready"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn open_connection(socket: &Path) -> (FramedTransport<LocalStream>, ClientId) {
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

#[cfg(unix)]
fn send_signal(process: &mut Child, signal: Signal) {
    kill(Pid::from_raw(i32::try_from(process.id()).unwrap()), signal).unwrap();
}

#[cfg(windows)]
fn send_signal(process: &mut Child, _force: bool) {
    process.kill().unwrap();
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    kill(Pid::from_raw(pid), None).is_ok()
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        line.split(',')
            .nth(1)
            .is_some_and(|value| value.trim_matches('"') == pid.to_string())
    })
}

#[cfg(unix)]
fn terminate_pid(pid: u32) {
    kill(Pid::from_raw(i32::try_from(pid).unwrap()), Signal::SIGKILL).unwrap();
}

#[cfg(windows)]
fn terminate_pid(pid: u32) {
    assert!(
        Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status()
            .unwrap()
            .success()
    );
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

    terminate_pid(first_pid);
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

    #[cfg(unix)]
    send_signal(&mut daemon, Signal::SIGKILL);
    #[cfg(windows)]
    send_signal(&mut daemon, true);
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

    #[cfg(unix)]
    {
        send_signal(&mut restarted, Signal::SIGTERM);
        assert!(restarted.wait().unwrap().success());
    }
    #[cfg(windows)]
    {
        send_signal(&mut restarted, true);
        let _ = restarted.wait().unwrap();
        terminate_pid(second_pid);
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    while process_is_alive(second_pid) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!process_is_alive(second_pid));
}
