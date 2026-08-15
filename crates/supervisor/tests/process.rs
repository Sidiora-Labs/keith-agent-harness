use std::time::Duration;

use keith_agent_types::{Generation, RootTreeId};
use keith_supervisor::{SupervisorOptions, WorkerEvent, WorkerHealth, WorkerSupervisor};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;

fn options() -> SupervisorOptions {
    SupervisorOptions {
        startup_timeout: Duration::from_secs(3),
        drain_timeout: Duration::from_secs(1),
        stale_heartbeat: Duration::from_secs(1),
        heartbeat_interval: Duration::from_millis(20),
    }
}

#[test]
fn real_workers_are_adopted_isolated_restarted_and_evicted() {
    let directory = tempfile::tempdir().unwrap();
    let executable = env!("CARGO_BIN_EXE_keith-worker-process-host");
    let first = RootTreeId::new();
    let second = RootTreeId::new();

    let mut initial = WorkerSupervisor::new(directory.path(), executable, options());
    let first_status = initial.start(first.clone(), Generation::new(1)).unwrap();
    let second_status = initial.start(second.clone(), Generation::new(1)).unwrap();
    assert_eq!(first_status.health, WorkerHealth::Healthy);
    assert_eq!(second_status.health, WorkerHealth::Healthy);

    drop(initial);
    let mut restarted_daemon = WorkerSupervisor::new(directory.path(), executable, options());
    let adopted = restarted_daemon.adopt_existing().unwrap();
    assert_eq!(adopted.len(), 2);

    kill(
        Pid::from_raw(i32::try_from(first_status.pid).unwrap()),
        Signal::SIGKILL,
    )
    .unwrap();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let event = loop {
        if let Some(event) = restarted_daemon.monitor().unwrap().into_iter().next() {
            break event;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(matches!(
        event,
        WorkerEvent::Exited { root_tree_id, .. } if root_tree_id == first
    ));
    assert_eq!(
        restarted_daemon.status(&second).unwrap().pid,
        second_status.pid
    );

    let replacement = restarted_daemon.restart(&first).unwrap();
    assert_eq!(replacement.generation, Generation::new(2));
    assert_ne!(replacement.pid, first_status.pid);

    std::thread::sleep(Duration::from_millis(5));
    let evicted = restarted_daemon.evict_idle(Duration::ZERO).unwrap();
    assert_eq!(evicted.len(), 2);
    assert!(restarted_daemon.statuses().is_empty());
}
