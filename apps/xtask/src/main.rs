use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

mod security;

fn main() -> ExitCode {
    let result = match env::args().nth(1).as_deref() {
        Some("ci") => ci(),
        Some("clean-checkout") => clean_checkout(),
        Some("dependency-policy") => dependency_policy(&workspace_root()),
        Some("schema-doc") => schema_document(
            &workspace_root(),
            matches!(env::args().nth(2).as_deref(), Some("--write")),
        ),
        Some("protocol-doc") => protocol_document(
            &workspace_root(),
            matches!(env::args().nth(2).as_deref(), Some("--write")),
        ),
        Some("security-gate") => security::run(&workspace_root()),
        _ => Err(
            "usage: cargo xtask <ci|clean-checkout|dependency-policy|schema-doc [--write]|protocol-doc [--write]|security-gate>".into(),
        ),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask is nested under apps")
        .to_path_buf()
}

fn ci() -> Result<(), String> {
    let root = workspace_root();
    run(&root, "cargo", &["fmt", "--all", "--", "--check"])?;
    dependency_policy(&root)?;
    schema_document(&root, false)?;
    protocol_document(&root, false)?;
    security::run(&root)?;
    run(
        &root,
        "cargo",
        &["check", "--workspace", "--all-targets", "--locked"],
    )?;
    run(
        &root,
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run(&root, "cargo", &["test", "--workspace", "--locked"])?;
    run_with_env(
        &root,
        "cargo",
        &["doc", "--workspace", "--no-deps", "--locked"],
        "RUSTDOCFLAGS",
        "-D warnings",
    )
}

fn protocol_document(root: &Path, write: bool) -> Result<(), String> {
    let path = root.join("docs/reference/agent-connection.md");
    let expected = keith_protocol::schema_markdown().map_err(|error| error.to_string())?;
    checked_generated_document(&path, expected, write)
}

fn schema_document(root: &Path, write: bool) -> Result<(), String> {
    let path = root.join("docs/reference/common-types.md");
    let expected = keith_agent_types::schema_markdown().map_err(|error| error.to_string())?;
    checked_generated_document(&path, expected, write)
}

fn checked_generated_document(path: &Path, expected: String, write: bool) -> Result<(), String> {
    if write {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, expected).map_err(|error| error.to_string())?;
        return Ok(());
    }
    let actual = fs::read_to_string(path)
        .map_err(|error| format!("schema document {} is missing: {error}", path.display()))?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "generated document {} is stale; run the corresponding keith-xtask document command with --write",
            path.display()
        ))
    }
}

fn clean_checkout() -> Result<(), String> {
    let root = workspace_root();
    let destination = env::temp_dir().join(format!("keith-clean-{}", std::process::id()));
    if destination.exists() {
        fs::remove_dir_all(&destination).map_err(|error| error.to_string())?;
    }
    copy_tree(&root, &destination)?;
    let result = run(
        &destination,
        "cargo",
        &["test", "--workspace", "--locked", "--offline", "--quiet"],
    );
    fs::remove_dir_all(&destination).map_err(|error| error.to_string())?;
    result
}

fn copy_tree(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        if matches!(name.to_str(), Some(".git" | ".codegraph" | "target")) {
            continue;
        }
        let source_path = entry.path();
        let destination_path = destination.join(name);
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_dir() {
            copy_tree(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(source_path, destination_path).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn dependency_policy(root: &Path) -> Result<(), String> {
    let manifests = manifests(root)?;
    let graph = dependency_graph(&manifests)?;
    let forbidden_layers = BTreeSet::from([
        "keith-daemon-core",
        "keith-supervisor",
        "keith-worker-runtime",
        "keith-provider-adapters",
        "keith-channel-adapters",
        "keith-ui-model",
    ]);
    let domains = [
        "keith-session",
        "keith-goals",
        "keith-memory",
        "keith-scheduler",
        "keith-routing",
        "keith-attention",
    ];

    require_no_internal_dependencies(&graph, "keith-agent-types")?;
    for domain in domains {
        reject_reachable(&graph, domain, &forbidden_layers)?;
    }
    reject_reachable(
        &graph,
        "keith-session",
        &BTreeSet::from([
            "keith-provider-adapters",
            "keith-channel-adapters",
            "keith-tool-runner-core",
            "keith-ui-model",
        ]),
    )?;
    reject_reachable(
        &graph,
        "keith-provider-adapters",
        &BTreeSet::from(["keith-session-store"]),
    )?;
    reject_reachable(
        &graph,
        "keith-channel-adapters",
        &BTreeSet::from(["keith-worker-runtime", "keith-session"]),
    )?;
    reject_reachable(
        &graph,
        "keith-state-store",
        &BTreeSet::from([
            "keith-daemon-core",
            "keith-supervisor",
            "keith-worker-runtime",
        ]),
    )?;
    reject_reachable(
        &graph,
        "keith-daemon-core",
        &BTreeSet::from([
            "keith-agent-loop",
            "keith-provider-adapters",
            "keith-tool-runner-core",
            "keith-sandbox",
            "keith-plugin-host",
        ]),
    )?;

    println!(
        "dependency policy passed for {} workspace packages",
        graph.len()
    );
    Ok(())
}

fn manifests(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut result = Vec::new();
    for parent in [root.join("crates"), root.join("apps")] {
        for entry in fs::read_dir(parent).map_err(|error| error.to_string())? {
            let path = entry
                .map_err(|error| error.to_string())?
                .path()
                .join("Cargo.toml");
            if path.is_file() {
                result.push(path);
            }
        }
    }
    Ok(result)
}

fn dependency_graph(manifests: &[PathBuf]) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let mut names = BTreeMap::new();
    for manifest in manifests {
        let content = fs::read_to_string(manifest).map_err(|error| error.to_string())?;
        let name = package_name(&content)
            .ok_or_else(|| format!("missing package name in {}", manifest.display()))?;
        names.insert(manifest.clone(), name);
    }
    let package_names: BTreeSet<_> = names.values().cloned().collect();
    let mut graph = BTreeMap::new();
    for (manifest, name) in names {
        let content = fs::read_to_string(manifest).map_err(|error| error.to_string())?;
        let dependencies = content
            .lines()
            .filter_map(dependency_name)
            .filter(|dependency| package_names.contains(*dependency))
            .map(str::to_owned)
            .collect();
        graph.insert(name, dependencies);
    }
    Ok(graph)
}

fn package_name(content: &str) -> Option<String> {
    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
        } else if in_package && trimmed.starts_with("name") {
            return quoted_value(trimmed).map(str::to_owned);
        }
    }
    None
}

fn dependency_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let (name, value) = trimmed.split_once('=')?;
    let name = name.trim();
    if name.starts_with("keith-") && value.contains("path") {
        Some(name)
    } else {
        None
    }
}

fn quoted_value(line: &str) -> Option<&str> {
    let (_, value) = line.split_once('=')?;
    value.trim().strip_prefix('"')?.strip_suffix('"')
}

fn require_no_internal_dependencies(
    graph: &BTreeMap<String, BTreeSet<String>>,
    package: &str,
) -> Result<(), String> {
    let dependencies = graph
        .get(package)
        .ok_or_else(|| format!("policy package missing: {package}"))?;
    if dependencies.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "{package} must not depend on internal packages: {dependencies:?}"
        ))
    }
}

fn reject_reachable(
    graph: &BTreeMap<String, BTreeSet<String>>,
    start: &str,
    forbidden: &BTreeSet<&str>,
) -> Result<(), String> {
    let mut pending = VecDeque::from([start]);
    let mut visited = BTreeSet::new();
    while let Some(package) = pending.pop_front() {
        if !visited.insert(package) {
            continue;
        }
        for dependency in graph.get(package).into_iter().flatten() {
            if forbidden.contains(dependency.as_str()) {
                return Err(format!(
                    "prohibited dependency path: {start} reaches {dependency}"
                ));
            }
            pending.push_back(dependency);
        }
    }
    Ok(())
}

fn run(root: &Path, program: &str, args: &[&str]) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .current_dir(root)
        .status()
        .map_err(|error| format!("failed to run {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed with {status}", args.join(" ")))
    }
}

fn run_with_env(
    root: &Path,
    program: &str,
    args: &[&str],
    key: impl AsRef<OsStr>,
    value: impl AsRef<OsStr>,
) -> Result<(), String> {
    let status = Command::new(program)
        .args(args)
        .env(key, value)
        .current_dir(root)
        .status()
        .map_err(|error| format!("failed to run {program}: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{program} {} failed with {status}", args.join(" ")))
    }
}
