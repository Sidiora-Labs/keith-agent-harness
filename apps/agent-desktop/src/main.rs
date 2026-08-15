use std::path::PathBuf;
use std::process::ExitCode;

use keith_agent_desktop::{BrowserHandoff, DesktopBootstrap};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("agent-desktop: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    let Some(command) = arguments.next() else {
        return Err("expected setup or open".into());
    };
    if matches!(command.to_str(), Some("--version" | "-V")) {
        println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    match command.to_str() {
        Some("setup-default") => {
            let origin = arguments
                .next()
                .and_then(|value| value.into_string().ok())
                .unwrap_or_else(|| "http://127.0.0.1:7341".into());
            DesktopBootstrap::initialize_default(&origin).map_err(|error| error.to_string())?;
            Ok(())
        }
        Some("setup") => {
            let state_root = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "setup requires STATE_ROOT".to_owned())?;
            let data_root = arguments
                .next()
                .map(PathBuf::from)
                .ok_or_else(|| "setup requires DATA_ROOT".to_owned())?;
            let origin = arguments
                .next()
                .and_then(|value| value.into_string().ok())
                .unwrap_or_else(|| "http://127.0.0.1:7341".into());
            DesktopBootstrap::initialize(&state_root, &data_root, &origin)
                .map_err(|error| error.to_string())?;
            Ok(())
        }
        Some("open") => {
            let origin = arguments
                .next()
                .and_then(|value| value.into_string().ok())
                .ok_or_else(|| "open requires ORIGIN".to_owned())?;
            let path = arguments
                .next()
                .and_then(|value| value.into_string().ok())
                .unwrap_or_else(|| "/".into());
            BrowserHandoff::new(&origin, &path)
                .and_then(|handoff| handoff.open())
                .map_err(|error| error.to_string())
        }
        _ => Err("unknown desktop command".into()),
    }
}
