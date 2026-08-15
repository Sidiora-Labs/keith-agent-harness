#![forbid(unsafe_code)]

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use keith_daemon_core::{DaemonCore, DaemonOptions};
use keith_platform::PlatformPaths;
use signal_hook::consts::{SIGINT, SIGTERM};

struct Arguments {
    data_root: PathBuf,
    socket: PathBuf,
    worker_executable: PathBuf,
    idle_seconds: u64,
}

impl Arguments {
    fn parse<I, S>(arguments: I) -> Result<Option<Self>, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut arguments = arguments.into_iter().map(Into::into);
        let program = arguments.next().unwrap_or_else(|| OsString::from("agentd"));
        let mut data_root = None;
        let mut socket = None;
        let mut worker_executable = None;
        let mut idle_seconds = 15 * 60;
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
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--data-root" => data_root = Some(PathBuf::from(value)),
                "--socket" => socket = Some(PathBuf::from(value)),
                "--worker-executable" => worker_executable = Some(PathBuf::from(value)),
                "--idle-seconds" => {
                    idle_seconds = value
                        .into_string()
                        .map_err(|_| "idle seconds must be UTF-8".to_owned())?
                        .parse()
                        .map_err(|_| "idle seconds must be an integer".to_owned())?;
                }
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        let platform_paths = if data_root.is_none() {
            Some(PlatformPaths::discover().map_err(|error| error.to_string())?)
        } else {
            None
        };
        let data_root = data_root
            .or_else(|| platform_paths.as_ref().map(|paths| paths.data_root.clone()))
            .ok_or_else(|| "native data root is unavailable".to_owned())?;
        let socket = socket.unwrap_or_else(|| {
            platform_paths.as_ref().map_or_else(
                || data_root.join("agentd.sock"),
                |paths| paths.daemon_endpoint.clone(),
            )
        });
        let worker_executable = worker_executable.unwrap_or_else(|| {
            let mut sibling = PathBuf::from(program);
            sibling.set_file_name("agent-worker");
            sibling
        });
        Ok(Some(Self {
            data_root,
            socket,
            worker_executable,
            idle_seconds,
        }))
    }
}

fn run() -> Result<(), String> {
    let Some(arguments) = Arguments::parse(std::env::args_os())? else {
        return Ok(());
    };
    let shutdown = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGTERM, Arc::clone(&shutdown))
        .map_err(|error| format!("failed to register SIGTERM: {error}"))?;
    signal_hook::flag::register(SIGINT, Arc::clone(&shutdown))
        .map_err(|error| format!("failed to register SIGINT: {error}"))?;
    let options = DaemonOptions {
        idle_evict_after: Duration::from_secs(arguments.idle_seconds),
        ..DaemonOptions::default()
    };
    let mut daemon = DaemonCore::open(arguments.data_root, arguments.worker_executable, options)
        .map_err(|error| error.to_string())?;
    daemon
        .serve_local(&arguments.socket, &shutdown)
        .map_err(|error| error.to_string())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
