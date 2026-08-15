#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use keith_agent_types::UtcTimestamp;
use keith_channel_core::{
    AgentConnection, EnqueueOutcome, GatewayLimits, GatewayQueue, ReconnectPolicy, RoutedInbound,
    SessionAction,
};
use keith_connection::{FramedTransport, LocalStream, connect_local};
use keith_protocol::{CommandResult, WireFormat};
use serde::Serialize;

struct Arguments {
    socket: PathBuf,
    reconnect: ReconnectPolicy,
}

impl Arguments {
    fn parse<I, S>(arguments: I) -> Result<Option<Self>, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut arguments = arguments.into_iter().map(Into::into);
        let _program = arguments.next();
        let mut socket = None;
        let mut reconnect = ReconnectPolicy::default();
        while let Some(argument) = arguments.next() {
            let argument = argument
                .into_string()
                .map_err(|_| "arguments must be UTF-8".to_owned())?;
            if matches!(argument.as_str(), "--version" | "-V") {
                println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?
                .into_string()
                .map_err(|_| format!("value for {argument} must be UTF-8"))?;
            match argument.as_str() {
                "--socket" => socket = Some(PathBuf::from(value)),
                "--reconnect-attempts" => {
                    reconnect.max_attempts = value
                        .parse()
                        .map_err(|_| "reconnect attempts must be an integer".to_owned())?;
                }
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Some(Self {
            socket: socket.ok_or_else(|| "--socket is required".to_owned())?,
            reconnect,
        }))
    }
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct GatewayReport {
    message_id: String,
    outcome: &'static str,
    safe_error: Option<String>,
}

type LocalAgentConnection = AgentConnection<FramedTransport<LocalStream>>;

fn connect(socket: &Path) -> Result<LocalAgentConnection, String> {
    let stream = connect_local(socket).map_err(|error| error.to_string())?;
    AgentConnection::connect(FramedTransport::new(stream, WireFormat::Json))
        .map_err(|error| error.to_string())
}

fn submit_with_reconnect(
    connection: &mut Option<LocalAgentConnection>,
    socket: &Path,
    policy: ReconnectPolicy,
    action: &SessionAction,
) -> Result<CommandResult, String> {
    let mut attempt = 0;
    loop {
        if connection.is_none() {
            match connect(socket) {
                Ok(connected) => *connection = Some(connected),
                Err(error) => {
                    let Some(delay) = policy.delay_ms(attempt) else {
                        return Err(error);
                    };
                    attempt = attempt.saturating_add(1);
                    thread::sleep(Duration::from_millis(delay));
                    continue;
                }
            }
        }
        let now = UtcTimestamp::now().unwrap_or(UtcTimestamp::UNIX_EPOCH);
        match connection
            .as_mut()
            .expect("connection initialized")
            .submit(action, now)
        {
            Ok(result) => return Ok(result),
            Err(error) => {
                *connection = None;
                let Some(delay) = policy.delay_ms(attempt) else {
                    return Err(error.to_string());
                };
                attempt = attempt.saturating_add(1);
                thread::sleep(Duration::from_millis(delay));
            }
        }
    }
}

fn write_report(report: &GatewayReport) -> Result<(), String> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer(&mut output, report).map_err(|error| error.to_string())?;
    output.write_all(b"\n").map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())
}

fn run() -> Result<(), String> {
    let Some(arguments) = Arguments::parse(std::env::args_os())? else {
        return Ok(());
    };
    let stdin = io::stdin();
    let mut queue =
        GatewayQueue::new(GatewayLimits::default()).map_err(|error| error.to_string())?;
    let mut connection = None;
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| error.to_string())?;
        let Ok(routed) = serde_json::from_str::<RoutedInbound>(&line) else {
            write_report(&GatewayReport {
                message_id: String::new(),
                outcome: "rejected",
                safe_error: Some("malformed normalized inbound message".to_owned()),
            })?;
            continue;
        };
        let message_id = routed.message.message_id.clone();
        match queue.enqueue(routed) {
            Ok(EnqueueOutcome::Duplicate) => {
                write_report(&GatewayReport {
                    message_id,
                    outcome: "duplicate",
                    safe_error: None,
                })?;
                continue;
            }
            Ok(EnqueueOutcome::Queued) => {}
            Err(error) => {
                write_report(&GatewayReport {
                    message_id,
                    outcome: "rejected",
                    safe_error: Some(error.to_string()),
                })?;
                continue;
            }
        }
        while let Some(ready) = queue.take_ready() {
            let session_id = ready.session_id.clone();
            let action = SessionAction::from(ready);
            let report = match submit_with_reconnect(
                &mut connection,
                &arguments.socket,
                arguments.reconnect,
                &action,
            ) {
                Ok(CommandResult::Accepted { .. } | CommandResult::Data(_)) => GatewayReport {
                    message_id: action.message_id,
                    outcome: "submitted",
                    safe_error: None,
                },
                Ok(CommandResult::Rejected(rejection)) => GatewayReport {
                    message_id: action.message_id,
                    outcome: "rejected",
                    safe_error: Some(rejection.error.message),
                },
                Err(error) => GatewayReport {
                    message_id: action.message_id,
                    outcome: "connection_failed",
                    safe_error: Some(error),
                },
            };
            queue
                .complete(&session_id)
                .map_err(|error| error.to_string())?;
            write_report(&report)?;
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_require_socket_and_bound_reconnect_attempts() {
        let parsed = Arguments::parse([
            "channel-gateway",
            "--socket",
            "/tmp/agent.sock",
            "--reconnect-attempts",
            "3",
        ])
        .expect("arguments")
        .expect("run");
        assert_eq!(parsed.socket, PathBuf::from("/tmp/agent.sock"));
        assert_eq!(parsed.reconnect.max_attempts, 3);
        assert!(Arguments::parse(["channel-gateway"]).is_err());
    }
}
