use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn desktop(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
        .args(arguments)
        .output()
        .expect("execute packaged desktop lifecycle command")
}

fn stdout(output: Output) -> String {
    assert!(
        output.status.success(),
        "desktop command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf8 output")
        .trim()
        .to_owned()
}

fn path_string(path: &Path) -> String {
    path.to_str().expect("utf8 temporary path").to_owned()
}

#[test]
fn executable_backup_update_rollback_restore_and_uninstall_lifecycle() {
    let directory = tempfile::tempdir().unwrap();
    let state = directory.path().join("state");
    let data = directory.path().join("data");
    let state_string = path_string(&state);
    let data_string = path_string(&data);
    stdout(desktop(&[
        "setup",
        &state_string,
        &data_string,
        "http://127.0.0.1:7341",
    ]));

    let settings: Value = serde_json::from_str(&stdout(desktop(&["settings", &state_string])))
        .expect("settings JSON");
    let installation = settings["installation_id"].as_str().unwrap();
    fs::write(data.join("durable-session.jsonl"), b"session state\n").unwrap();

    let backup = stdout(desktop(&["backup", &state_string]));
    let restored = directory.path().join("restored-data");
    let restored_string = path_string(&restored);
    stdout(desktop(&["restore", &backup, &restored_string]));
    assert_eq!(
        fs::read(restored.join("durable-session.jsonl")).unwrap(),
        b"session state\n"
    );

    let release_one = directory.path().join("release-one");
    let release_two = directory.path().join("release-two");
    fs::create_dir(&release_one).unwrap();
    fs::create_dir(&release_two).unwrap();
    fs::write(release_one.join("agentd"), b"version one").unwrap();
    fs::write(release_two.join("agentd"), b"version two").unwrap();
    let release_one_string = path_string(&release_one);
    let release_two_string = path_string(&release_two);
    let digest_one = stdout(desktop(&["digest-release", &release_one_string]));
    let digest_two = stdout(desktop(&["digest-release", &release_two_string]));
    let active_one: Value = serde_json::from_str(&stdout(desktop(&[
        "update",
        &state_string,
        "1.0.0",
        &release_one_string,
        &digest_one,
    ])))
    .unwrap();
    assert_eq!(active_one["current"], "1.0.0");
    let active_two: Value = serde_json::from_str(&stdout(desktop(&[
        "update",
        &state_string,
        "2.0.0",
        &release_two_string,
        &digest_two,
    ])))
    .unwrap();
    assert_eq!(active_two["previous"], "1.0.0");
    let rolled: Value =
        serde_json::from_str(&stdout(desktop(&["rollback", &state_string]))).unwrap();
    assert_eq!(rolled["current"], "1.0.0");

    let plan: Value = serde_json::from_str(&stdout(desktop(&[
        "uninstall-plan",
        &state_string,
        "keep-user-data",
    ])))
    .unwrap();
    assert_eq!(plan["confirmation"], format!("REMOVE {installation}"));
    let rejected = desktop(&[
        "uninstall",
        &state_string,
        "keep-user-data",
        "wrong confirmation",
    ]);
    assert!(!rejected.status.success());
    stdout(desktop(&[
        "uninstall",
        &state_string,
        "keep-user-data",
        plan["confirmation"].as_str().unwrap(),
    ]));
    assert!(!state.join("updates").exists());
    assert!(state.join("desktop.json").exists());
    assert_eq!(
        fs::read(data.join("durable-session.jsonl")).unwrap(),
        b"session state\n"
    );
}
