use super::{
    ArtifactExpectation, OrchestrationWatchdog, ReconcileObservation, RetryPolicy, RuntimeState,
    WatchSpec, WatchStatus,
};
use chrono::{Duration, Utc};
use std::fs;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git should run");
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn init_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]);
    git(
        dir.path(),
        &["config", "user.email", "watchdog@example.test"],
    );
    git(dir.path(), &["config", "user.name", "Watchdog Test"]);
    fs::write(dir.path().join("README.md"), "initial\n").unwrap();
    git(dir.path(), &["add", "README.md"]);
    git(dir.path(), &["commit", "-qm", "initial"]);
    dir
}

fn spec(job_id: &str, repo: &Path) -> WatchSpec {
    WatchSpec {
        job_id: job_id.to_string(),
        dedupe_key: format!("session-1:{job_id}"),
        owner_session_id: "session-1".to_string(),
        worker_session_id: Some("worker-1".to_string()),
        process_id: None,
        working_dir: repo.to_path_buf(),
        baseline_sha: None,
        expected_sha: None,
        expected_artifacts: Vec::new(),
        deadline: Some(Utc::now() + Duration::minutes(10)),
        stale_after: Duration::hours(1),
        retry: RetryPolicy::default(),
    }
}

#[test]
fn normal_completion_is_terminal_and_deliverable() {
    let state = tempfile::tempdir().unwrap();
    let repo = init_repo();
    let watchdog = OrchestrationWatchdog::with_root(state.path());
    watchdog.register(spec("normal", repo.path())).unwrap();

    let record = watchdog
        .reconcile(
            "normal",
            ReconcileObservation::runtime(RuntimeState::Completed),
        )
        .unwrap();
    assert_eq!(record.status, WatchStatus::Completed);
    assert!(
        watchdog
            .claim_terminal_delivery("normal", "dispatcher-a")
            .unwrap()
            .is_some()
    );
}

#[test]
fn timeout_can_later_reconcile_to_completion() {
    let state = tempfile::tempdir().unwrap();
    let repo = init_repo();
    let watchdog = OrchestrationWatchdog::with_root(state.path());
    let mut watch = spec("late", repo.path());
    watch.deadline = Some(Utc::now() - Duration::seconds(1));
    watchdog.register(watch).unwrap();

    let timed_out = watchdog
        .reconcile("late", ReconcileObservation::runtime(RuntimeState::Running))
        .unwrap();
    assert_eq!(timed_out.status, WatchStatus::Watching);
    assert!(timed_out.deadline_exceeded_at.is_some());

    let completed = watchdog
        .reconcile(
            "late",
            ReconcileObservation::runtime(RuntimeState::Completed),
        )
        .unwrap();
    assert_eq!(completed.status, WatchStatus::Completed);
}

#[test]
fn registry_recovers_after_server_reload() {
    let state = tempfile::tempdir().unwrap();
    let repo = init_repo();
    OrchestrationWatchdog::with_root(state.path())
        .register(spec("reload", repo.path()))
        .unwrap();
    OrchestrationWatchdog::with_root(state.path())
        .reconcile(
            "reload",
            ReconcileObservation {
                runtime_state: RuntimeState::Running,
                process_running: None,
                swarm_status: Some("swarm=abc active=1".to_string()),
                retries_exhausted: false,
                observed_at: Utc::now(),
            },
        )
        .unwrap();

    let reloaded = OrchestrationWatchdog::with_root(state.path());
    let record = reloaded.get("reload").unwrap().unwrap();
    assert_eq!(record.owner_session_id, "session-1");
    assert_eq!(
        record.last_swarm_status.as_deref(),
        Some("swarm=abc active=1")
    );
}

#[test]
fn lost_swarm_ownership_falls_back_to_artifact_evidence() {
    let state = tempfile::tempdir().unwrap();
    let repo = init_repo();
    let watchdog = OrchestrationWatchdog::with_root(state.path());
    let mut watch = spec("lost-owner", repo.path());
    watch.expected_artifacts = vec![ArtifactExpectation::required("result.json")];
    watchdog.register(watch).unwrap();
    fs::write(repo.path().join("result.json"), "{}\n").unwrap();

    let record = watchdog
        .reconcile(
            "lost-owner",
            ReconcileObservation::runtime(RuntimeState::OwnershipLost),
        )
        .unwrap();
    assert_eq!(record.status, WatchStatus::Completed);
    assert!(
        record
            .completion_evidence
            .iter()
            .any(|item| item.contains("result.json"))
    );
}

#[test]
fn duplicate_scheduled_checks_are_coalesced() {
    let state = tempfile::tempdir().unwrap();
    let repo = init_repo();
    let watchdog = OrchestrationWatchdog::with_root(state.path());
    watchdog.register(spec("dedupe", repo.path())).unwrap();

    let first = watchdog
        .claim_due_check("dedupe", "scheduler-a")
        .unwrap()
        .unwrap();
    assert!(
        watchdog
            .claim_due_check("dedupe", "scheduler-b")
            .unwrap()
            .is_none()
    );
    watchdog.finish_check("dedupe", &first.lease_id).unwrap();
}

#[test]
fn worker_failure_uses_backoff_and_model_fallback_before_terminal_failure() {
    let state = tempfile::tempdir().unwrap();
    let repo = init_repo();
    let watchdog = OrchestrationWatchdog::with_root(state.path());
    let mut watch = spec("retry", repo.path());
    watch.retry = RetryPolicy {
        max_attempts: 2,
        initial_backoff_secs: 1,
        max_backoff_secs: 10,
        model_fallbacks: vec!["model-a".into(), "model-b".into()],
    };
    watchdog.register(watch).unwrap();

    let retry = watchdog
        .reconcile("retry", ReconcileObservation::runtime(RuntimeState::Failed))
        .unwrap();
    assert_eq!(retry.status, WatchStatus::RetryScheduled);
    assert_eq!(retry.next_model.as_deref(), Some("model-b"));
    assert!(retry.next_check_at.is_some());

    watchdog.mark_retry_started("retry", Some(123)).unwrap();
    let failed = watchdog
        .reconcile("retry", ReconcileObservation::runtime(RuntimeState::Failed))
        .unwrap();
    assert_eq!(failed.status, WatchStatus::Failed);
}

#[test]
fn stale_job_becomes_terminal_without_destructive_recovery() {
    let state = tempfile::tempdir().unwrap();
    let repo = init_repo();
    let watchdog = OrchestrationWatchdog::with_root(state.path());
    let mut watch = spec("stale", repo.path());
    watch.stale_after = Duration::zero();
    watchdog.register(watch).unwrap();

    let record = watchdog
        .reconcile(
            "stale",
            ReconcileObservation::runtime(RuntimeState::Unknown),
        )
        .unwrap();
    assert_eq!(record.status, WatchStatus::Stale);
    assert!(repo.path().join("README.md").exists());
}

#[test]
fn repo_commit_is_completion_evidence_without_agent_status() {
    let state = tempfile::tempdir().unwrap();
    let repo = init_repo();
    let watchdog = OrchestrationWatchdog::with_root(state.path());
    watchdog.register(spec("commit", repo.path())).unwrap();
    fs::write(repo.path().join("done.txt"), "done\n").unwrap();
    git(repo.path(), &["add", "done.txt"]);
    git(repo.path(), &["commit", "-qm", "finish job"]);

    let record = watchdog
        .reconcile(
            "commit",
            ReconcileObservation::runtime(RuntimeState::Unknown),
        )
        .unwrap();
    assert_eq!(record.status, WatchStatus::Completed);
    assert!(
        record
            .completion_evidence
            .iter()
            .any(|item| item.contains("HEAD advanced"))
    );
}

#[test]
fn terminal_notification_is_claimed_exactly_once() {
    let state = tempfile::tempdir().unwrap();
    let repo = init_repo();
    let watchdog = OrchestrationWatchdog::with_root(state.path());
    watchdog.register(spec("once", repo.path())).unwrap();
    watchdog
        .reconcile(
            "once",
            ReconcileObservation::runtime(RuntimeState::Completed),
        )
        .unwrap();

    let delivery = watchdog
        .claim_terminal_delivery("once", "dispatcher-a")
        .unwrap()
        .unwrap();
    assert!(!delivery.event_id.is_empty());
    assert!(watchdog.acknowledge_claimed_delivery("once").unwrap());
    assert!(
        watchdog
            .claim_terminal_delivery("once", "dispatcher-b")
            .unwrap()
            .is_none()
    );

    let reloaded = OrchestrationWatchdog::with_root(state.path());
    assert!(
        reloaded
            .claim_terminal_delivery("once", "dispatcher-c")
            .unwrap()
            .is_none()
    );
}

#[test]
fn dirty_worktree_is_observed_but_never_reset() {
    let state = tempfile::tempdir().unwrap();
    let repo = init_repo();
    let watchdog = OrchestrationWatchdog::with_root(state.path());
    watchdog.register(spec("dirty", repo.path())).unwrap();
    fs::write(repo.path().join("README.md"), "user work\n").unwrap();

    let record = watchdog
        .reconcile(
            "dirty",
            ReconcileObservation::runtime(RuntimeState::Running),
        )
        .unwrap();
    assert!(record.repository.as_ref().unwrap().dirty);
    assert_eq!(
        fs::read_to_string(repo.path().join("README.md")).unwrap(),
        "user work\n"
    );
}

#[test]
fn terminal_watch_cleanup_removes_only_registry_state() {
    let state = tempfile::tempdir().unwrap();
    let repo = init_repo();
    let watchdog = OrchestrationWatchdog::with_root(state.path());
    watchdog.register(spec("cleanup", repo.path())).unwrap();
    watchdog
        .reconcile(
            "cleanup",
            ReconcileObservation::runtime(RuntimeState::Completed),
        )
        .unwrap();

    assert_eq!(
        watchdog
            .cleanup_terminal_before(Utc::now() + Duration::seconds(1))
            .unwrap(),
        1
    );
    assert!(watchdog.get("cleanup").unwrap().is_none());
    assert!(repo.path().join("README.md").exists());
}
