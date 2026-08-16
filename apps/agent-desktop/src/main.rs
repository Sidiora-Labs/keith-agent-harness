use std::path::PathBuf;
use std::process::ExitCode;

use keith_agent_desktop::{
    BrowserHandoff, DesktopBootstrap, DesktopUpdateManager, UninstallChoice, backup_state,
    execute_uninstall, plan_uninstall, restore_state,
};
use keith_agent_types::UtcTimestamp;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("agent-desktop: {error}");
            ExitCode::FAILURE
        }
    }
}

#[allow(clippy::too_many_lines)]
fn run() -> Result<(), String> {
    let mut arguments = std::env::args_os();
    let _program = arguments.next();
    let Some(command) = arguments.next() else {
        return Err(usage().into());
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
        Some("settings") => {
            let state_root = required_path(&mut arguments, "settings requires STATE_ROOT")?;
            let settings =
                DesktopBootstrap::load(&state_root).map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&settings).map_err(|error| error.to_string())?
            );
            Ok(())
        }
        Some("backup") => {
            let state_root = required_path(&mut arguments, "backup requires STATE_ROOT")?;
            let settings =
                DesktopBootstrap::load(&state_root).map_err(|error| error.to_string())?;
            let backup = backup_state(&settings).map_err(|error| error.to_string())?;
            println!("{}", backup.display());
            Ok(())
        }
        Some("restore") => {
            let backup = required_path(&mut arguments, "restore requires BACKUP")?;
            let target = required_path(&mut arguments, "restore requires TARGET_DATA_ROOT")?;
            restore_state(&backup, &target).map_err(|error| error.to_string())
        }
        Some("digest-release") => {
            let release =
                required_path(&mut arguments, "digest-release requires RELEASE_DIRECTORY")?;
            let digest = DesktopUpdateManager::digest_release(&release)
                .map_err(|error| error.to_string())?;
            println!("{digest}");
            Ok(())
        }
        Some("verify-release") => {
            let release =
                required_path(&mut arguments, "verify-release requires RELEASE_DIRECTORY")?;
            let encoded_key = required_string(
                &mut arguments,
                "verify-release requires EXPECTED_PUBLIC_KEY_HEX",
            )?;
            let public_key = keith_release::decode_public_key(&encoded_key)
                .map_err(|error| error.to_string())?;
            let verified = keith_release::verify_release(&release, &public_key)
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&verified).map_err(|error| error.to_string())?
            );
            Ok(())
        }
        Some("update") => {
            let state_root = required_path(&mut arguments, "update requires STATE_ROOT")?;
            let release = required_path(&mut arguments, "update requires RELEASE_DIRECTORY")?;
            let public_key =
                required_string(&mut arguments, "update requires EXPECTED_PUBLIC_KEY_HEX")?;
            DesktopBootstrap::load(&state_root).map_err(|error| error.to_string())?;
            let manager =
                DesktopUpdateManager::open(&state_root).map_err(|error| error.to_string())?;
            let active = manager
                .activate(
                    &release,
                    &public_key,
                    UtcTimestamp::now().map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&active).map_err(|error| error.to_string())?
            );
            Ok(())
        }
        Some("rollback") => {
            let state_root = required_path(&mut arguments, "rollback requires STATE_ROOT")?;
            DesktopBootstrap::load(&state_root).map_err(|error| error.to_string())?;
            let manager =
                DesktopUpdateManager::open(&state_root).map_err(|error| error.to_string())?;
            let active = manager
                .rollback(UtcTimestamp::now().map_err(|error| error.to_string())?)
                .map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&active).map_err(|error| error.to_string())?
            );
            Ok(())
        }
        Some("uninstall-plan") => {
            let state_root = required_path(&mut arguments, "uninstall-plan requires STATE_ROOT")?;
            let choice = uninstall_choice(&required_string(
                &mut arguments,
                "uninstall-plan requires DATA_CHOICE",
            )?)?;
            let settings =
                DesktopBootstrap::load(&state_root).map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&plan_uninstall(&settings, choice))
                    .map_err(|error| error.to_string())?
            );
            Ok(())
        }
        Some("uninstall") => {
            let state_root = required_path(&mut arguments, "uninstall requires STATE_ROOT")?;
            let choice = uninstall_choice(&required_string(
                &mut arguments,
                "uninstall requires DATA_CHOICE",
            )?)?;
            let confirmation = required_string(&mut arguments, "uninstall requires CONFIRMATION")?;
            let settings =
                DesktopBootstrap::load(&state_root).map_err(|error| error.to_string())?;
            let plan = plan_uninstall(&settings, choice);
            execute_uninstall(&settings, &plan, &confirmation).map_err(|error| error.to_string())
        }
        _ => Err(usage().into()),
    }
}

fn required_path(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
    message: &str,
) -> Result<PathBuf, String> {
    arguments
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| message.into())
}

fn required_string(
    arguments: &mut impl Iterator<Item = std::ffi::OsString>,
    message: &str,
) -> Result<String, String> {
    arguments
        .next()
        .and_then(|value| value.into_string().ok())
        .ok_or_else(|| message.into())
}

fn uninstall_choice(value: &str) -> Result<UninstallChoice, String> {
    match value {
        "keep-user-data" => Ok(UninstallChoice::KeepUserData),
        "remove-runtime" => Ok(UninstallChoice::RemoveRuntime),
        "remove-everything" => Ok(UninstallChoice::RemoveEverything),
        _ => Err("DATA_CHOICE must be keep-user-data, remove-runtime, or remove-everything".into()),
    }
}

fn usage() -> &'static str {
    "usage: agent-desktop <setup-default [ORIGIN]|setup STATE_ROOT DATA_ROOT [ORIGIN]|settings STATE_ROOT|backup STATE_ROOT|restore BACKUP TARGET_DATA_ROOT|digest-release RELEASE_DIRECTORY|verify-release RELEASE_DIRECTORY EXPECTED_PUBLIC_KEY_HEX|update STATE_ROOT RELEASE_DIRECTORY EXPECTED_PUBLIC_KEY_HEX|rollback STATE_ROOT|uninstall-plan STATE_ROOT DATA_CHOICE|uninstall STATE_ROOT DATA_CHOICE CONFIRMATION|open ORIGIN [PATH]>"
}
