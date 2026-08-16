#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use keith_credentials::{MasterKey, NativeMasterKeyStore, RestrictedMasterKeyStore};
use keith_daemon_core::{DaemonCore, DaemonOptions};
use keith_local_runtime::{LocalRuntime, LocalRuntimeConfig};
use keith_platform::PlatformPaths;
use signal_hook::consts::{SIGINT, SIGTERM};

struct Arguments {
    data_root: PathBuf,
    socket: PathBuf,
    worker_executable: PathBuf,
    idle_seconds: u64,
    credential_root: PathBuf,
    credential_key_source: CredentialKeySource,
    workspace_root: PathBuf,
    openai_base_url: String,
    anthropic_base_url: String,
    provider_base_urls: BTreeMap<String, String>,
}

enum CredentialKeySource {
    Environment(String),
    Native(String),
    Restricted(PathBuf),
}

impl Arguments {
    #[allow(clippy::too_many_lines)]
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
        let mut credential_root = None;
        let mut credential_key_source = None;
        let mut workspace_root = None;
        let mut openai_base_url = "https://api.openai.com".to_owned();
        let mut anthropic_base_url = "https://api.anthropic.com".to_owned();
        let mut provider_base_urls = BTreeMap::new();
        while let Some(argument) = arguments.next() {
            let argument = argument
                .into_string()
                .map_err(|_| "arguments must be UTF-8".to_owned())?;
            if matches!(argument.as_str(), "--version" | "-V") {
                println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            if argument == "--build-info" {
                let report = keith_build_info::BuildReport::current(
                    "daemon",
                    &["framed_json", "replay", "session_lifecycle", "snapshots"],
                );
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
                );
                return Ok(None);
            }
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--data-root" => data_root = Some(PathBuf::from(value)),
                "--socket" => socket = Some(PathBuf::from(value)),
                "--worker-executable" => worker_executable = Some(PathBuf::from(value)),
                "--credential-root" => credential_root = Some(PathBuf::from(value)),
                "--credential-key-env" => {
                    credential_key_source = Some(CredentialKeySource::Environment(
                        value
                            .into_string()
                            .map_err(|_| "credential key environment must be UTF-8".to_owned())?,
                    ));
                }
                "--credential-key-native-account" => {
                    credential_key_source =
                        Some(CredentialKeySource::Native(value.into_string().map_err(
                            |_| "native key account must be UTF-8".to_owned(),
                        )?));
                }
                "--workspace-root" => workspace_root = Some(PathBuf::from(value)),
                "--openai-base-url" => {
                    openai_base_url = value
                        .into_string()
                        .map_err(|_| "OpenAI base URL must be UTF-8".to_owned())?;
                }
                "--anthropic-base-url" => {
                    anthropic_base_url = value
                        .into_string()
                        .map_err(|_| "Anthropic base URL must be UTF-8".to_owned())?;
                }
                "--provider-base-url" => {
                    let value = value
                        .into_string()
                        .map_err(|_| "provider base URL must be UTF-8".to_owned())?;
                    let (provider, base_url) = value.split_once('=').ok_or_else(|| {
                        "provider base URL must use the form PROVIDER=https://endpoint".to_owned()
                    })?;
                    if provider.trim().is_empty() || base_url.trim().is_empty() {
                        return Err(
                            "provider base URL must use the form PROVIDER=https://endpoint".into(),
                        );
                    }
                    if provider_base_urls
                        .insert(provider.to_owned(), base_url.to_owned())
                        .is_some()
                    {
                        return Err(format!("provider base URL for {provider} was repeated"));
                    }
                }
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
        let credential_root = credential_root.unwrap_or_else(|| data_root.join("credentials"));
        let credential_key_source = credential_key_source
            .unwrap_or_else(|| CredentialKeySource::Restricted(credential_root.clone()));
        let workspace_root = workspace_root
            .map_or_else(std::env::current_dir, Ok)
            .map_err(|error| error.to_string())?;
        Ok(Some(Self {
            data_root,
            socket,
            worker_executable,
            idle_seconds,
            credential_root,
            credential_key_source,
            workspace_root,
            openai_base_url,
            anthropic_base_url,
            provider_base_urls,
        }))
    }

    fn credential_key(&self) -> Result<MasterKey, String> {
        match &self.credential_key_source {
            CredentialKeySource::Environment(environment) => {
                let encoded = std::env::var_os(environment)
                    .ok_or_else(|| format!("{environment} is unavailable"))?
                    .into_encoded_bytes();
                decode_key(&encoded).map(MasterKey::from_bytes)
            }
            CredentialKeySource::Native(account) => {
                NativeMasterKeyStore::new("keith-agent", account.clone())
                    .and_then(|store| store.load_or_create())
                    .map_err(|error| error.to_string())
            }
            CredentialKeySource::Restricted(root) => RestrictedMasterKeyStore::open(root)
                .and_then(|store| store.load_or_create())
                .map_err(|error| error.to_string()),
        }
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
    let credential_key = arguments.credential_key()?;
    let runtime = LocalRuntimeConfig {
        data_root: arguments.data_root.clone(),
        credential_root: arguments.credential_root,
        credential_key,
        workspace_root: arguments.workspace_root,
        openai_base_url: arguments.openai_base_url,
        anthropic_base_url: arguments.anthropic_base_url,
        provider_base_urls: arguments.provider_base_urls,
    };
    let runtime = LocalRuntime::open(runtime).map_err(|error| error.to_string())?;
    let mut daemon = DaemonCore::open_with_runtime(
        arguments.data_root,
        arguments.worker_executable,
        options,
        Box::new(runtime),
    )
    .map_err(|error| error.to_string())?;
    daemon
        .serve_local(&arguments.socket, &shutdown)
        .map_err(|error| error.to_string())
}

fn decode_key(encoded: &[u8]) -> Result<[u8; 32], String> {
    if encoded.len() != 64 {
        return Err("credential key must be 64 hexadecimal characters".into());
    }
    let mut decoded = [0_u8; 32];
    for (target, pair) in decoded.iter_mut().zip(encoded.chunks_exact(2)) {
        *target = (hex_digit(pair[0])? << 4) | hex_digit(pair[1])?;
    }
    Ok(decoded)
}

fn hex_digit(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err("credential key must be hexadecimal".into()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
