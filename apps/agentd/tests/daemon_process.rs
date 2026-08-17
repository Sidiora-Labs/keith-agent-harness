use std::collections::BTreeSet;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use keith_agent_types::{
    CURRENT_PROTOCOL_VERSION, CURRENT_SCHEMA_VERSION, ClientId, CommandId, EntityId, ProfileId,
    RootTreeId, SessionId, UtcTimestamp, WorkerId,
};
use keith_agent_web::{PlatformCompatibilityConfig, WebServer, WebServerConfig};
use keith_connection::{
    AgentTransport, FramedTransport, LocalStream, connect_local, set_local_read_timeout,
    set_local_write_timeout,
};
use keith_credentials::{
    CredentialOwner, CredentialRef, EncryptedCredentialStore, MasterKey, RestrictedMasterKeyStore,
    SecretValue,
};
use keith_daemon_core::RootManifest;
use keith_local_runtime::{LocalRuntimeLaunchConfig, RuntimeCredentialKeySource};
use keith_protocol::{
    AttachSession, ClientCommand, ClientHello, CommandEnvelope, CommandResult, CreateGoal,
    GoalLimits, ResponsePayload, SessionFilter, SessionState, WireFormat, WireMessage,
};
use keith_worker_runtime::{WorkerRunState, read_registration, registration_path};
#[cfg(unix)]
use nix::sys::signal::{Signal, kill};
#[cfg(unix)]
use nix::unistd::Pid;

fn write_manifest(
    data_root: &Path,
    root: &RootTreeId,
    session: &SessionId,
    profile_id: &ProfileId,
) {
    let directory = data_root.join("sessions").join(root.to_string());
    fs::create_dir_all(&directory).unwrap();
    let manifest = RootManifest {
        version: CURRENT_SCHEMA_VERSION,
        root_tree_id: root.clone(),
        root_session_id: session.clone(),
        profile_id: profile_id.clone(),
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

fn seed_runtime_session(
    launch: &LocalRuntimeLaunchConfig,
    root: &RootTreeId,
    session: &SessionId,
) -> ProfileId {
    let runtime = launch
        .open_worker(root.clone(), WorkerId::new(), EntityId::new())
        .unwrap();
    let profile = runtime.registered_profiles().unwrap().remove(0);
    runtime
        .create_session_assigned(
            &profile.profile.id,
            &profile.profile.workspace_id,
            session.clone(),
            root.clone(),
            Some(format!("root {root}")),
        )
        .unwrap();
    profile.profile.id
}

fn seed_provider_credential(launch: &LocalRuntimeLaunchConfig) {
    let key = RestrictedMasterKeyStore::open(&launch.credential_root)
        .unwrap()
        .load_or_create()
        .unwrap();
    EncryptedCredentialStore::open(&launch.credential_root, key)
        .unwrap()
        .put(
            CredentialRef::new("default", CredentialOwner::Provider("openai".into())).unwrap(),
            SecretValue::new("unused-process-test-credential").unwrap(),
            UtcTimestamp::now().unwrap(),
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

async fn http_request(address: SocketAddr, request: Vec<u8>) -> String {
    tokio::task::spawn_blocking(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        stream.write_all(&request).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        response
    })
    .await
    .unwrap()
}

async fn http_stream_prefix(address: SocketAddr, request: Vec<u8>, marker: &str) -> String {
    let marker = marker.to_owned();
    tokio::task::spawn_blocking(move || {
        let mut stream = TcpStream::connect(address).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        stream.write_all(&request).unwrap();
        let mut response = Vec::new();
        let mut chunk = [0_u8; 4096];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    response.extend_from_slice(&chunk[..read]);
                    if String::from_utf8_lossy(&response).contains(&marker) {
                        break;
                    }
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) =>
                {
                    break;
                }
                Err(error) => panic!("event stream read failed: {error}"),
            }
        }
        String::from_utf8(response).unwrap()
    })
    .await
    .unwrap()
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
    let launch = LocalRuntimeLaunchConfig {
        data_root: data_root.clone(),
        credential_root: data_root.join("credentials"),
        credential_key_source: RuntimeCredentialKeySource::Restricted(
            data_root.join("credentials"),
        ),
        workspace_root: directory.path().join("workspace"),
        openai_base_url: "http://127.0.0.1:1".into(),
        anthropic_base_url: "http://127.0.0.1:1".into(),
        provider_base_urls: std::collections::BTreeMap::new(),
    };
    seed_provider_credential(&launch);
    let first_profile = seed_runtime_session(&launch, &first_root, &first_session);
    let second_profile = seed_runtime_session(&launch, &second_root, &second_session);
    assert_eq!(first_profile, second_profile);
    write_manifest(&data_root, &first_root, &first_session, &first_profile);
    write_manifest(&data_root, &second_root, &second_session, &second_profile);

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_platform_bridge_reaches_the_real_daemon_and_leased_worker() {
    let directory = tempfile::tempdir().unwrap();
    let data_root = directory.path().join("data");
    let socket = directory.path().join("agentd.sock");
    let workspace = directory.path().join("workspace");
    let root = RootTreeId::new();
    let requested_session = SessionId::new();
    let launch = LocalRuntimeLaunchConfig {
        data_root: data_root.clone(),
        credential_root: data_root.join("credentials"),
        credential_key_source: RuntimeCredentialKeySource::Restricted(
            data_root.join("credentials"),
        ),
        workspace_root: workspace,
        openai_base_url: "http://127.0.0.1:1".into(),
        anthropic_base_url: "http://127.0.0.1:1".into(),
        provider_base_urls: std::collections::BTreeMap::new(),
    };
    seed_provider_credential(&launch);
    let runtime = launch
        .open_worker(root.clone(), WorkerId::new(), EntityId::new())
        .unwrap();
    let registered = runtime.registered_profiles().unwrap().remove(0);
    let profile = registered.profile.id;
    let created = runtime
        .create_session_assigned(
            &profile,
            &registered.profile.workspace_id,
            requested_session,
            root.clone(),
            Some("Native bridge process proof".into()),
        )
        .unwrap();
    let _created_session = created.session_id;
    drop(runtime);
    let mut daemon = start_daemon(&data_root, &socket);
    let _connection = open_connection(&socket);

    let assets = directory.path().join("assets");
    fs::create_dir(&assets).unwrap();
    fs::write(assets.join("agent_web.js"), b"export default function(){}").unwrap();
    fs::write(assets.join("agent_web_bg.wasm"), b"\0asm\x01\0\0\0").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let platform_key = b"real-daemon-platform-bridge-key-0001";
    let web = WebServer::new(WebServerConfig {
        bind: address,
        exact_origin: format!("http://{address}"),
        daemon_socket: socket.clone(),
        asset_root: assets,
        credential_root: directory.path().join("web-credentials"),
        credential_key: MasterKey::from_bytes([0x42; 32]),
        login_secret: b"real-daemon-web-login-secret-0001".to_vec(),
        session_lifetime: Duration::from_secs(60),
        mutation_limit_per_second: 8,
        daemon_timeout: Duration::from_secs(2),
        openai_compatibility: None,
        platform_compatibility: Some(PlatformCompatibilityConfig {
            api_key: platform_key.to_vec(),
            allow_non_loopback: false,
            max_in_flight: 2,
        }),
    })
    .unwrap();
    let web_task = tokio::spawn(web.serve_listener(listener));

    let catalog = http_request(
        address,
        format!(
            "GET /platform/v1/catalog HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",
            String::from_utf8_lossy(platform_key)
        )
        .into_bytes(),
    )
    .await;
    assert!(catalog.starts_with("HTTP/1.1 200 OK"), "{catalog}");
    assert!(catalog.contains(&profile.to_string()), "{catalog}");
    let (_, catalog_body) = catalog.split_once("\r\n\r\n").unwrap();
    let catalog_json: serde_json::Value = serde_json::from_str(catalog_body).unwrap();
    let session: SessionId = catalog_json["sessions"][0]["session_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let session_root: RootTreeId = catalog_json["sessions"][0]["root_tree_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let body = serde_json::to_vec(&serde_json::json!({
        "session_id": session.clone(),
        "command": ClientCommand::AttachSession(AttachSession {
            session_id: session.clone(),
            resume: None,
        }),
    }))
    .unwrap();
    let mut command = format!(
        "POST /platform/v1/profiles/{profile}/commands HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        String::from_utf8_lossy(platform_key),
        body.len()
    )
    .into_bytes();
    command.extend_from_slice(&body);
    let attached = http_request(address, command).await;
    assert!(attached.starts_with("HTTP/1.1 200 OK"), "{attached}");
    let (_, attached_body) = attached.split_once("\r\n\r\n").unwrap();
    let attached_json: serde_json::Value = serde_json::from_str(attached_body).unwrap();
    let generation = attached_json["result"]["payload"]["value"]["generation"]
        .as_u64()
        .unwrap();
    let worker_pid = wait_for_worker(&data_root, &session_root);
    assert!(process_is_alive(worker_pid));

    let goal_body = serde_json::to_vec(&serde_json::json!({
        "session_id": session.clone(),
        "command": ClientCommand::CreateGoal(CreateGoal {
            session_id: session.clone(),
            objective: "Prove native replay".into(),
            limits: GoalLimits {
                max_turns: Some(1),
                max_tokens: Some(64),
                deadline: None,
            },
        }),
    }))
    .unwrap();
    let mut goal_command = format!(
        "POST /platform/v1/profiles/{profile}/commands HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        String::from_utf8_lossy(platform_key),
        goal_body.len()
    )
    .into_bytes();
    goal_command.extend_from_slice(&goal_body);
    let goal = http_request(address, goal_command).await;
    assert!(goal.starts_with("HTTP/1.1 200 OK"), "{goal}");

    let wrong_profile = ProfileId::new();
    let mut denied = format!(
        "POST /platform/v1/profiles/{wrong_profile}/commands HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        String::from_utf8_lossy(platform_key),
        body.len()
    )
    .into_bytes();
    denied.extend_from_slice(&body);
    let denied = http_request(address, denied).await;
    assert!(denied.starts_with("HTTP/1.1 403 Forbidden"), "{denied}");
    assert!(denied.contains("scope_denied"), "{denied}");

    let replay = http_stream_prefix(
        address,
        format!(
            "GET /platform/v1/events/{profile}/{session}?generation={generation}&sequence=0 HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nAccept: text/event-stream\r\nConnection: close\r\n\r\n",
            String::from_utf8_lossy(platform_key)
        )
        .into_bytes(),
        "\"message\":\"snapshot\"",
    )
    .await;
    assert!(replay.starts_with("HTTP/1.1 200 OK"), "{replay}");
    assert!(replay.contains("text/event-stream"), "{replay}");
    assert!(replay.contains("\"message\":\"snapshot\""), "{replay}");
    assert!(replay.contains("Prove native replay"), "{replay}");

    web_task.abort();
    let _ = web_task.await;
    #[cfg(unix)]
    {
        send_signal(&mut daemon, Signal::SIGTERM);
        assert!(daemon.wait().unwrap().success());
    }
    #[cfg(windows)]
    {
        send_signal(&mut daemon, true);
        let _ = daemon.wait().unwrap();
        terminate_pid(worker_pid);
    }
}
