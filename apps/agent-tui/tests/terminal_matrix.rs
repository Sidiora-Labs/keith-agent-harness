#![cfg(unix)]
#![forbid(unsafe_code)]

use std::fs::File;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use keith_agent_tui::{Accessibility, ColorMode, TuiApp, TuiOverlay, render};
use keith_agent_types::{
    CURRENT_PROTOCOL_VERSION, EntityId, EntryId, Generation, MessageId, ProfileId, Revision,
    RootTreeId, Sequence, SessionId, TurnId, UtcTimestamp,
};
use keith_connection::{AgentTransport, FramedTransport};
use keith_protocol::{
    ClientCommand, CommandResult, CommandResultEnvelope, DaemonEvent, EventEnvelope,
    MessageProjection, MessageRole, PresenceProjection, PresenceState, ResponsePayload,
    SessionSnapshot, SessionState, SessionSummary, TurnTerminalProjection, TurnTerminalStatus,
    WireFormat, WireMessage, negotiate,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;
use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::dup;
use rustix::process::{Pid, Signal, kill_process};
use rustix::pty::{OpenptFlags, grantpt, ioctl_tiocgptpeer, openpt, unlockpt};
use rustix::termios::{Winsize, tcgetattr, tcsetwinsize};

const COMPATIBILITY_MATRIX: &str = include_str!("terminal_compatibility.csv");

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalSnapshot {
    text: String,
    colors: Vec<(Color, Color)>,
}

#[derive(Clone, Copy)]
struct MatrixCase<'a> {
    name: &'a str,
    width: u16,
    height: u16,
    color: ColorMode,
    term: &'a str,
    overlay: Option<TuiOverlay>,
    scenario: &'a str,
    expected: &'a str,
}

#[test]
fn published_terminal_matrix_is_executable_and_deterministic() {
    let cases = matrix_cases();
    assert_eq!(cases.len(), 11);
    for case in cases {
        assert!(matches!(
            case.term,
            "xterm" | "xterm-256color" | "screen-256color" | "tmux-256color"
        ));
        let app = matrix_app(case);
        let first = snapshot(&app, case.width, case.height);
        let second = snapshot(&app, case.width, case.height);
        assert_eq!(first, second, "{} was not deterministic", case.name);
        assert!(
            first.text.contains(case.expected),
            "{} did not contain {:?}:\n{}",
            case.name,
            case.expected,
            first.text
        );
        for forbidden in ["┌", "┐", "└", "┘", "│", "─", "✨", "🟣", "💜"] {
            assert!(
                !first.text.contains(forbidden),
                "{} emitted {forbidden}",
                case.name
            );
        }
        if case.color == ColorMode::NoColor {
            assert!(
                first
                    .colors
                    .iter()
                    .all(|(foreground, background)| *foreground == Color::Reset
                        && *background == Color::Reset),
                "no-color mode emitted terminal colors"
            );
        } else {
            assert!(
                first
                    .colors
                    .iter()
                    .any(|(foreground, background)| *foreground != Color::Reset
                        || *background != Color::Reset),
                "{} did not exercise its color palette",
                case.name
            );
        }
    }
}

#[test]
fn long_history_remains_reachable_and_control_bytes_never_execute() {
    let mut app = matrix_app(MatrixCase {
        name: "long-history",
        width: 72,
        height: 20,
        color: ColorMode::NoColor,
        term: "xterm-256color",
        overlay: None,
        scenario: "long-history",
        expected: "history item 119",
    });
    let tail = snapshot(&app, 72, 20).text;
    assert!(tail.contains("history item 119"));
    app.scroll_from_end = 346;
    let beginning = snapshot(&app, 72, 20).text;
    assert!(beginning.contains("history item 000"));
    assert!(!beginning.contains('\u{1b}'));
    assert!(beginning.contains('�'));

    let source = include_str!("../src/render.rs").to_ascii_lowercase();
    for forbidden in ["purple", "glow", ".borders("] {
        assert!(
            !source.contains(forbidden),
            "render source contains {forbidden}"
        );
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn real_pty_journey_preserves_scrollback_paste_resize_idle_events_editor_and_signal_restore() {
    let directory = tempfile::tempdir().unwrap();
    let socket = directory.path().join("agent.sock");
    let profile = ProfileId::new();
    let session = SessionId::new();
    let root = RootTreeId::new();
    let (ready_sender, ready_receiver) = mpsc::channel();
    let host_socket = socket.clone();
    let host_profile = profile.clone();
    let host_session = session.clone();
    let host_root = root.clone();
    let host = thread::spawn(move || {
        let listener = keith_connection::bind_permissioned_local(&host_socket).unwrap();
        ready_sender.send(()).unwrap();
        let stream = keith_connection::accept_local(&listener).unwrap();
        serve_terminal_connection(stream, &host_profile, &host_session, &host_root);
    });
    ready_receiver.recv_timeout(Duration::from_secs(2)).unwrap();

    let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).unwrap();
    grantpt(&master).unwrap();
    unlockpt(&master).unwrap();
    let slave = ioctl_tiocgptpeer(&master, OpenptFlags::RDWR | OpenptFlags::NOCTTY).unwrap();
    tcsetwinsize(
        &master,
        Winsize {
            ws_row: 28,
            ws_col: 96,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )
    .unwrap();
    let original_mode = format!("{:?}", tcgetattr(&slave).unwrap());
    let stdin = dup(&slave).unwrap();
    let stdout = dup(&slave).unwrap();
    let stderr = dup(&slave).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_agent-tui"))
        .arg("--socket")
        .arg(&socket)
        .arg("--session")
        .arg(session.to_string())
        .arg("--startup-timeout-ms")
        .arg("2000")
        .arg("--color")
        .arg("none")
        .arg("--reduced-motion")
        .env("TERM", "xterm-256color")
        .env("EDITOR", "/bin/true")
        .stdin(Stdio::from(stdin))
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .unwrap();
    let pid = Pid::from_child(&child);
    let mut master = File::from(master);
    let flags = fcntl_getfl(&master).unwrap();
    fcntl_setfl(&master, flags | OFlags::NONBLOCK).unwrap();
    let mut output = Vec::new();

    wait_for_output(
        &mut master,
        &mut output,
        "idle event arrived",
        Duration::from_secs(5),
    );
    let initial = String::from_utf8_lossy(&output);
    assert!(searchable_terminal_output(&output).contains("scrollbacksurvivestheliveviewport"));
    assert!(initial.contains("\u{1b}[?2004h"));
    assert!(!initial.contains("\u{1b}[?1049h"));

    master
        .write_all(b"\x1b[200~pasted safely\nsecond line\x1b[201~")
        .unwrap();
    wait_for_output(
        &mut master,
        &mut output,
        "pasted safely",
        Duration::from_secs(3),
    );
    master.write_all(&[0x05]).unwrap();
    thread::sleep(Duration::from_millis(250));
    drain(&mut master, &mut output);
    assert!(
        child.try_wait().unwrap().is_none(),
        "external editor ended the TUI"
    );

    tcsetwinsize(
        &master,
        Winsize {
            ws_row: 14,
            ws_col: 48,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )
    .unwrap();
    kill_process(pid, Signal::WINCH).unwrap();
    thread::sleep(Duration::from_millis(250));
    drain(&mut master, &mut output);
    assert!(searchable_terminal_output(&output).contains("Entersend"));

    kill_process(pid, Signal::TERM).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "TUI did not stop after SIGTERM");
        drain(&mut master, &mut output);
        thread::sleep(Duration::from_millis(20));
    };
    drain(&mut master, &mut output);
    assert!(status.success(), "TUI did not exit cleanly: {status}");
    assert_eq!(format!("{:?}", tcgetattr(&slave).unwrap()), original_mode);
    let final_output = String::from_utf8_lossy(&output);
    assert!(final_output.contains("\u{1b}[?2004l"));
    assert!(searchable_terminal_output(&output).contains("scrollbacksurvivestheliveviewport"));
    drop(slave);
    drop(master);
    host.join().unwrap();
}

fn matrix_cases() -> Vec<MatrixCase<'static>> {
    COMPATIBILITY_MATRIX
        .lines()
        .skip(1)
        .map(|line| {
            let fields = line.split(',').collect::<Vec<_>>();
            assert_eq!(fields.len(), 8, "invalid compatibility row: {line}");
            MatrixCase {
                name: fields[0],
                width: fields[1].parse().unwrap(),
                height: fields[2].parse().unwrap(),
                color: match fields[3] {
                    "truecolor" => ColorMode::TrueColor,
                    "256" => ColorMode::Ansi256,
                    "none" => ColorMode::NoColor,
                    "contrast" => ColorMode::HighContrast,
                    value => panic!("invalid matrix color {value}"),
                },
                term: fields[4],
                overlay: match fields[5] {
                    "none" => None,
                    "commands" => Some(TuiOverlay::Commands),
                    value => panic!("invalid matrix overlay {value}"),
                },
                scenario: fields[6],
                expected: fields[7],
            }
        })
        .collect()
}

fn matrix_app(case: MatrixCase<'_>) -> TuiApp {
    let mut app = TuiApp::new(Accessibility {
        color_mode: case.color,
        reduced_motion: true,
    });
    app.connected = true;
    let mut state = snapshot_state(case.scenario);
    if case.scenario == "streaming" {
        state.presence.state = PresenceState::Thinking;
        state.messages.push(MessageProjection {
            message_id: MessageId::new(),
            final_id: None,
            role: MessageRole::Assistant,
            text: "streaming safely".into(),
            committed: false,
        });
    } else if case.scenario == "failure" {
        state.presence.state = PresenceState::Failed;
        state.terminal = Some(TurnTerminalProjection {
            session_id: state.session.session_id.clone(),
            turn_id: TurnId::new(),
            final_id: EntryId::new(),
            status: TurnTerminalStatus::Failed,
            execution_succeeded: false,
            final_created: true,
            artifacts_persisted: true,
            delivery_enqueued: false,
            delivery_acknowledged: false,
            detail: Some("A safe failure".into()),
        });
    } else if case.scenario == "long-history" {
        state.messages = (0..120)
            .map(|index| MessageProjection {
                message_id: MessageId::new(),
                final_id: Some(EntryId::new()),
                role: if index % 2 == 0 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                text: if index == 0 {
                    "history item 000 safe�[2J".into()
                } else {
                    format!("history item {index:03}")
                },
                committed: true,
            })
            .collect();
    }
    app.apply_wire_message(WireMessage::Snapshot(keith_protocol::SnapshotFrame {
        protocol: CURRENT_PROTOCOL_VERSION,
        root_tree_id: state.session.root_tree_id.clone(),
        generation: state.generation,
        first_sequence: state.through_sequence,
        sequence: state.through_sequence,
        occurred_at: UtcTimestamp::UNIX_EPOCH,
        snapshot: Box::new(state),
    }));
    app.overlay = case.overlay;
    app
}

fn snapshot(app: &TuiApp, width: u16, height: u16) -> TerminalSnapshot {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| render(frame, app)).unwrap();
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    let mut colors = Vec::new();
    for y in 0..height {
        let mut line = String::new();
        for x in 0..width {
            let cell = buffer.cell((x, y)).unwrap();
            line.push_str(cell.symbol());
            colors.push((cell.fg, cell.bg));
        }
        text.push_str(line.trim_end());
        text.push('\n');
    }
    TerminalSnapshot { text, colors }
}

fn snapshot_state(scenario: &str) -> SessionSnapshot {
    let session_id = SessionId::new();
    SessionSnapshot {
        session: SessionSummary {
            session_id: session_id.clone(),
            root_tree_id: RootTreeId::new(),
            profile_id: ProfileId::new(),
            title: Some(format!("{scenario} conversation")),
            state: SessionState::Ready,
            updated_at: UtcTimestamp::UNIX_EPOCH,
        },
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
            session_id,
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

fn serve_terminal_connection(
    stream: keith_connection::LocalStream,
    profile: &ProfileId,
    session: &SessionId,
    root: &RootTreeId,
) {
    let mut transport = FramedTransport::new(stream, WireFormat::Json);
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
    let mut sent_idle_event = false;
    while let Ok(WireMessage::Command(envelope)) = transport.receive() {
        let attached = matches!(&envelope.command, ClientCommand::AttachSession(_));
        let result = match envelope.command {
            ClientCommand::ListSessions(_) => CommandResult::Data(Box::new(
                ResponsePayload::Sessions(vec![session_summary(profile, session, root)]),
            )),
            ClientCommand::AttachSession(_) => CommandResult::Data(Box::new(
                ResponsePayload::Snapshot(Box::new(pty_snapshot(profile, session, root))),
            )),
            _ => CommandResult::Accepted { action_id: None },
        };
        transport
            .send(&WireMessage::CommandResult(CommandResultEnvelope {
                protocol: CURRENT_PROTOCOL_VERSION,
                command_id: envelope.command_id,
                completed_at: UtcTimestamp::UNIX_EPOCH,
                result,
            }))
            .unwrap();
        if attached && !sent_idle_event {
            thread::sleep(Duration::from_millis(175));
            let message_id = MessageId::new();
            transport
                .send(&WireMessage::Event(EventEnvelope {
                    protocol: CURRENT_PROTOCOL_VERSION,
                    root_tree_id: root.clone(),
                    generation: Generation::new(1),
                    first_sequence: Sequence::new(1),
                    sequence: Sequence::new(1),
                    occurred_at: UtcTimestamp::UNIX_EPOCH,
                    event: DaemonEvent::AssistantDelta {
                        message_id,
                        text: "idle event arrived".into(),
                    },
                }))
                .unwrap();
            sent_idle_event = true;
        }
    }
}

fn pty_snapshot(profile: &ProfileId, session: &SessionId, root: &RootTreeId) -> SessionSnapshot {
    let mut state = snapshot_state("PTY");
    state.session = session_summary(profile, session, root);
    state.presence.session_id = session.clone();
    state.messages.push(MessageProjection {
        message_id: MessageId::new(),
        final_id: Some(EntryId::new()),
        role: MessageRole::Assistant,
        text: "scrollback survives the live viewport".into(),
        committed: true,
    });
    state
}

fn session_summary(profile: &ProfileId, session: &SessionId, root: &RootTreeId) -> SessionSummary {
    SessionSummary {
        session_id: session.clone(),
        root_tree_id: root.clone(),
        profile_id: profile.clone(),
        title: Some("Terminal journey".into()),
        state: SessionState::Ready,
        updated_at: UtcTimestamp::UNIX_EPOCH,
    }
}

fn wait_for_output(file: &mut File, output: &mut Vec<u8>, expected: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        drain(file, output);
        let expected = expected
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>();
        if searchable_terminal_output(output).contains(&expected) {
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "PTY output did not contain {expected:?}:\n{}",
        String::from_utf8_lossy(output)
    );
}

fn searchable_terminal_output(output: &[u8]) -> String {
    let mut searchable = String::new();
    let decoded = String::from_utf8_lossy(output);
    let mut characters = decoded.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\u{1b}' {
            if characters.next_if_eq(&'[').is_some() {
                for parameter in characters.by_ref() {
                    if ('@'..='~').contains(&parameter) {
                        break;
                    }
                }
            }
            continue;
        }
        if !character.is_whitespace() && !character.is_control() {
            searchable.push(character);
        }
    }
    searchable
}

fn drain(file: &mut File, output: &mut Vec<u8>) {
    let mut chunk = [0_u8; 16 * 1024];
    loop {
        match file.read(&mut chunk) {
            Ok(0) => return,
            Ok(read) => {
                let received = &chunk[..read];
                output.extend_from_slice(received);
                if received.windows(4).any(|window| window == b"\x1b[6n") {
                    file.write_all(b"\x1b[1;1R").unwrap();
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return,
            Err(error) if error.raw_os_error() == Some(5) => return,
            Err(error) => panic!("failed to read PTY: {error}"),
        }
    }
}
