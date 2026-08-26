//! Durable orchestration watch registry and reconciliation engine.
//!
//! The watchdog is deliberately observation-only. It records process, swarm,
//! repository, and artifact evidence, schedules retries, and exposes a durable
//! terminal-delivery outbox. It never resets, cleans, checks out, or otherwise
//! mutates a watched worktree.

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use uuid::Uuid;

const CHECK_LEASE_SECS: i64 = 300;
const DELIVERY_LEASE_SECS: i64 = 300;
const MAX_AUDIT_EVENTS: usize = 256;

fn registry_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WatchStatus {
    Watching,
    RetryScheduled,
    Completed,
    Failed,
    Stale,
}

impl WatchStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Stale)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeState {
    Running,
    Completed,
    Failed,
    Unknown,
    OwnershipLost,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactExpectation {
    pub path: PathBuf,
    pub required: bool,
}

impl ArtifactExpectation {
    pub fn required(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            required: true,
        }
    }

    pub fn optional(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            required: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_backoff_secs: u64,
    pub max_backoff_secs: u64,
    pub model_fallbacks: Vec<String>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            initial_backoff_secs: 30,
            max_backoff_secs: 15 * 60,
            model_fallbacks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WatchSpec {
    pub job_id: String,
    pub dedupe_key: String,
    pub owner_session_id: String,
    pub worker_session_id: Option<String>,
    pub process_id: Option<u32>,
    pub working_dir: PathBuf,
    pub baseline_sha: Option<String>,
    pub expected_sha: Option<String>,
    pub expected_artifacts: Vec<ArtifactExpectation>,
    pub deadline: Option<DateTime<Utc>>,
    pub stale_after: Duration,
    pub retry: RetryPolicy,
}

#[derive(Debug, Clone, Default)]
pub struct WatchUpdate {
    pub worker_session_id: Option<String>,
    pub process_id: Option<u32>,
    pub working_dir: Option<PathBuf>,
    pub expected_sha: Option<String>,
    pub expected_artifacts: Option<Vec<ArtifactExpectation>>,
    pub deadline: Option<DateTime<Utc>>,
    pub stale_after: Option<Duration>,
    pub retry: Option<RetryPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositorySnapshot {
    pub head_sha: Option<String>,
    pub dirty: bool,
    pub status_summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactSnapshot {
    pub path: PathBuf,
    pub required: bool,
    pub exists: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub at: DateTime<Utc>,
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckLease {
    pub lease_id: String,
    pub claimant: String,
    pub claimed_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum DeliveryState {
    Pending,
    Claimed {
        claimant: String,
        claimed_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    },
    Delivered {
        delivered_at: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct TerminalDelivery {
    event_id: String,
    state: DeliveryState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchRecord {
    pub job_id: String,
    pub dedupe_key: String,
    pub owner_session_id: String,
    pub worker_session_id: Option<String>,
    pub process_id: Option<u32>,
    pub working_dir: PathBuf,
    pub baseline_sha: Option<String>,
    pub expected_sha: Option<String>,
    pub expected_artifacts: Vec<ArtifactExpectation>,
    pub deadline: Option<DateTime<Utc>>,
    pub deadline_exceeded_at: Option<DateTime<Utc>>,
    pub stale_after_secs: i64,
    pub status: WatchStatus,
    pub runtime_state: RuntimeState,
    #[serde(default)]
    pub last_swarm_status: Option<String>,
    pub attempt: u32,
    pub retry: RetryPolicy,
    pub next_model: Option<String>,
    pub next_check_at: Option<DateTime<Utc>>,
    pub active_check: Option<CheckLease>,
    pub repository: Option<RepositorySnapshot>,
    pub artifacts: Vec<ArtifactSnapshot>,
    pub completion_evidence: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub terminal_at: Option<DateTime<Utc>>,
    pub audit: Vec<AuditEvent>,
    terminal_delivery: Option<TerminalDelivery>,
}

#[derive(Debug, Clone)]
pub struct ReconcileObservation {
    pub runtime_state: RuntimeState,
    pub process_running: Option<bool>,
    pub swarm_status: Option<String>,
    pub retries_exhausted: bool,
    pub observed_at: DateTime<Utc>,
}

impl ReconcileObservation {
    pub fn runtime(runtime_state: RuntimeState) -> Self {
        Self {
            runtime_state,
            process_running: None,
            swarm_status: None,
            retries_exhausted: false,
            observed_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryClaim {
    pub event_id: String,
    pub job_id: String,
    pub owner_session_id: String,
    pub status: WatchStatus,
    pub completion_evidence: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct OrchestrationWatchdog {
    root: PathBuf,
}

impl OrchestrationWatchdog {
    pub fn new() -> Self {
        Self::with_root(crate::storage::durable_state_dir().join("orchestration-watchdog"))
    }

    pub fn with_root(root: impl AsRef<Path>) -> Self {
        let root = root.as_ref().to_path_buf();
        let _ = fs::create_dir_all(&root);
        Self { root }
    }

    pub fn register(&self, mut spec: WatchSpec) -> Result<WatchRecord> {
        validate_spec(&spec)?;
        let _guard = registry_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if let Some(existing) = self
            .list_unlocked()?
            .into_iter()
            .find(|record| record.dedupe_key == spec.dedupe_key && !record.status.is_terminal())
        {
            return Ok(existing);
        }

        let now = Utc::now();
        let repository = inspect_repository(&spec.working_dir);
        if spec.baseline_sha.is_none() {
            spec.baseline_sha = repository.as_ref().and_then(|repo| repo.head_sha.clone());
        }
        let artifacts = inspect_artifacts(&spec.working_dir, &spec.expected_artifacts)?;
        let mut record = WatchRecord {
            job_id: spec.job_id,
            dedupe_key: spec.dedupe_key,
            owner_session_id: spec.owner_session_id,
            worker_session_id: spec.worker_session_id,
            process_id: spec.process_id,
            working_dir: spec.working_dir,
            baseline_sha: spec.baseline_sha,
            expected_sha: spec.expected_sha,
            expected_artifacts: spec.expected_artifacts,
            deadline: spec.deadline,
            deadline_exceeded_at: None,
            stale_after_secs: spec.stale_after.num_seconds().max(0),
            status: WatchStatus::Watching,
            runtime_state: RuntimeState::Running,
            last_swarm_status: None,
            attempt: 1,
            retry: spec.retry,
            next_model: None,
            next_check_at: Some(now),
            active_check: None,
            repository,
            artifacts,
            completion_evidence: Vec::new(),
            created_at: now,
            updated_at: now,
            terminal_at: None,
            audit: Vec::new(),
            terminal_delivery: None,
        };
        push_audit(&mut record, now, "registered", "durable watch registered");
        self.save_unlocked(&record)?;
        Ok(record)
    }

    pub fn get(&self, job_id: &str) -> Result<Option<WatchRecord>> {
        let _guard = registry_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.load_unlocked(job_id)
    }

    pub fn update(&self, job_id: &str, update: WatchUpdate) -> Result<WatchRecord> {
        let _guard = registry_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut record = self
            .load_unlocked(job_id)?
            .ok_or_else(|| anyhow!("unknown orchestration watch {job_id}"))?;
        if record.status.is_terminal() {
            return Ok(record);
        }
        if let Some(working_dir) = update.working_dir {
            if !working_dir.is_absolute() {
                return Err(anyhow!("watch working_dir must be absolute"));
            }
            record.working_dir = working_dir;
            record.repository = inspect_repository(&record.working_dir);
            record.baseline_sha = record
                .repository
                .as_ref()
                .and_then(|repository| repository.head_sha.clone());
        }
        if let Some(expected) = update.expected_artifacts {
            for artifact in &expected {
                validate_relative_artifact(&artifact.path)?;
            }
            record.expected_artifacts = expected;
            record.artifacts = inspect_artifacts(&record.working_dir, &record.expected_artifacts)?;
        }
        if update.worker_session_id.is_some() {
            record.worker_session_id = update.worker_session_id;
        }
        if update.process_id.is_some() {
            record.process_id = update.process_id;
        }
        if update.expected_sha.is_some() {
            record.expected_sha = update.expected_sha;
        }
        if update.deadline.is_some() {
            record.deadline = update.deadline;
        }
        if let Some(stale_after) = update.stale_after {
            record.stale_after_secs = stale_after.num_seconds().max(0);
        }
        if let Some(retry) = update.retry {
            record.retry = retry;
        }
        let now = Utc::now();
        record.updated_at = now;
        push_audit(
            &mut record,
            now,
            "configured",
            "watch expectations or retry policy updated",
        );
        self.save_unlocked(&record)?;
        Ok(record)
    }

    pub fn list(&self) -> Result<Vec<WatchRecord>> {
        let _guard = registry_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.list_unlocked()
    }

    pub fn reconcile(
        &self,
        job_id: &str,
        observation: ReconcileObservation,
    ) -> Result<WatchRecord> {
        let _guard = registry_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut record = self
            .load_unlocked(job_id)?
            .ok_or_else(|| anyhow!("unknown orchestration watch {job_id}"))?;
        if record.status.is_terminal() {
            return Ok(record);
        }

        let now = observation.observed_at;
        record.runtime_state = observation.runtime_state.clone();
        record.repository = inspect_repository(&record.working_dir);
        record.artifacts = inspect_artifacts(&record.working_dir, &record.expected_artifacts)?;
        record.updated_at = now;

        if let Some(status) = observation.swarm_status.as_deref() {
            record.last_swarm_status = Some(status.to_string());
            push_audit(
                &mut record,
                now,
                "swarm_observed",
                &format!("swarm status: {status}"),
            );
        }

        let mut evidence = completion_evidence(&record);
        if matches!(observation.runtime_state, RuntimeState::Completed) {
            evidence.insert(0, "runtime reported completion".to_string());
        }

        if matches!(observation.runtime_state, RuntimeState::Completed) || !evidence.is_empty() {
            record.completion_evidence = evidence;
            set_terminal(
                &mut record,
                WatchStatus::Completed,
                now,
                "completion reconciled",
            );
        } else if matches!(observation.runtime_state, RuntimeState::Failed) {
            if observation.retries_exhausted {
                set_terminal(
                    &mut record,
                    WatchStatus::Failed,
                    now,
                    "worker failed after exhausting the runtime retry policy",
                );
            } else {
                schedule_retry_or_fail(&mut record, now);
            }
        } else {
            if let Some(deadline) = record.deadline
                && now >= deadline
                && record.deadline_exceeded_at.is_none()
            {
                record.deadline_exceeded_at = Some(now);
                push_audit(
                    &mut record,
                    now,
                    "deadline_exceeded",
                    "deadline passed; watch remains active for late completion",
                );
            }

            let process_is_running = observation
                .process_running
                .or_else(|| record.process_id.map(crate::platform::is_process_running));
            let stale_at = record.created_at + Duration::seconds(record.stale_after_secs);
            if now >= stale_at
                && !matches!(process_is_running, Some(true))
                && !matches!(observation.runtime_state, RuntimeState::Running)
            {
                set_terminal(
                    &mut record,
                    WatchStatus::Stale,
                    now,
                    "watch became stale; no destructive recovery attempted",
                );
            } else {
                record.status = WatchStatus::Watching;
                let delay = backoff_secs(&record.retry, record.attempt).max(1);
                record.next_check_at = Some(now + Duration::seconds(delay as i64));
                if matches!(observation.runtime_state, RuntimeState::OwnershipLost) {
                    push_audit(
                        &mut record,
                        now,
                        "ownership_lost",
                        "owner status unavailable; repository and artifacts inspected",
                    );
                }
            }
        }

        record.active_check = None;
        self.save_unlocked(&record)?;
        Ok(record)
    }

    pub fn claim_due_check(&self, job_id: &str, claimant: &str) -> Result<Option<CheckLease>> {
        let _guard = registry_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut record = self
            .load_unlocked(job_id)?
            .ok_or_else(|| anyhow!("unknown orchestration watch {job_id}"))?;
        if record.status.is_terminal() {
            return Ok(None);
        }
        let now = Utc::now();
        if record.next_check_at.is_some_and(|due| due > now) {
            return Ok(None);
        }
        if record
            .active_check
            .as_ref()
            .is_some_and(|lease| lease.expires_at > now)
        {
            return Ok(None);
        }
        let lease = CheckLease {
            lease_id: Uuid::new_v4().to_string(),
            claimant: claimant.to_string(),
            claimed_at: now,
            expires_at: now + Duration::seconds(CHECK_LEASE_SECS),
        };
        record.active_check = Some(lease.clone());
        record.updated_at = now;
        push_audit(
            &mut record,
            now,
            "check_claimed",
            &format!("check claimed by {claimant}"),
        );
        self.save_unlocked(&record)?;
        Ok(Some(lease))
    }

    pub fn finish_check(&self, job_id: &str, lease_id: &str) -> Result<()> {
        let _guard = registry_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut record = self
            .load_unlocked(job_id)?
            .ok_or_else(|| anyhow!("unknown orchestration watch {job_id}"))?;
        if record
            .active_check
            .as_ref()
            .is_some_and(|lease| lease.lease_id == lease_id)
        {
            record.active_check = None;
            record.updated_at = Utc::now();
            self.save_unlocked(&record)?;
        }
        Ok(())
    }

    pub fn mark_retry_started(&self, job_id: &str, process_id: Option<u32>) -> Result<WatchRecord> {
        let _guard = registry_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut record = self
            .load_unlocked(job_id)?
            .ok_or_else(|| anyhow!("unknown orchestration watch {job_id}"))?;
        if record.status != WatchStatus::RetryScheduled {
            return Err(anyhow!("watch {job_id} has no scheduled retry"));
        }
        let now = Utc::now();
        record.attempt = record.attempt.saturating_add(1);
        record.process_id = process_id;
        record.status = WatchStatus::Watching;
        record.runtime_state = RuntimeState::Running;
        record.next_check_at = Some(now);
        record.updated_at = now;
        let attempt = record.attempt;
        push_audit(
            &mut record,
            now,
            "retry_started",
            &format!("retry attempt {attempt} started"),
        );
        self.save_unlocked(&record)?;
        Ok(record)
    }

    pub fn claim_terminal_delivery(
        &self,
        job_id: &str,
        claimant: &str,
    ) -> Result<Option<DeliveryClaim>> {
        let _guard = registry_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut record = self
            .load_unlocked(job_id)?
            .ok_or_else(|| anyhow!("unknown orchestration watch {job_id}"))?;
        if !record.status.is_terminal() {
            return Ok(None);
        }
        let now = Utc::now();
        let delivery = record
            .terminal_delivery
            .as_mut()
            .ok_or_else(|| anyhow!("terminal watch {job_id} has no delivery event"))?;
        match &delivery.state {
            DeliveryState::Delivered { .. } => return Ok(None),
            DeliveryState::Claimed { expires_at, .. } if *expires_at > now => return Ok(None),
            DeliveryState::Pending | DeliveryState::Claimed { .. } => {}
        }
        delivery.state = DeliveryState::Claimed {
            claimant: claimant.to_string(),
            claimed_at: now,
            expires_at: now + Duration::seconds(DELIVERY_LEASE_SECS),
        };
        let claim = DeliveryClaim {
            event_id: delivery.event_id.clone(),
            job_id: record.job_id.clone(),
            owner_session_id: record.owner_session_id.clone(),
            status: record.status.clone(),
            completion_evidence: record.completion_evidence.clone(),
        };
        record.updated_at = now;
        push_audit(
            &mut record,
            now,
            "delivery_claimed",
            &format!("terminal delivery claimed by {claimant}"),
        );
        self.save_unlocked(&record)?;
        Ok(Some(claim))
    }

    pub fn mark_terminal_delivered(&self, job_id: &str, event_id: &str) -> Result<()> {
        let _guard = registry_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut record = self
            .load_unlocked(job_id)?
            .ok_or_else(|| anyhow!("unknown orchestration watch {job_id}"))?;
        let delivery = record
            .terminal_delivery
            .as_mut()
            .ok_or_else(|| anyhow!("terminal watch {job_id} has no delivery event"))?;
        if delivery.event_id != event_id {
            return Err(anyhow!("delivery event mismatch for watch {job_id}"));
        }
        if matches!(delivery.state, DeliveryState::Delivered { .. }) {
            return Ok(());
        }
        let now = Utc::now();
        delivery.state = DeliveryState::Delivered { delivered_at: now };
        record.updated_at = now;
        push_audit(
            &mut record,
            now,
            "delivered",
            "terminal notification acknowledged",
        );
        self.save_unlocked(&record)
    }

    /// Acknowledge the currently claimed delivery after the server has actually
    /// handed it to a client, live turn, or durable soft-interrupt queue.
    pub fn acknowledge_claimed_delivery(&self, job_id: &str) -> Result<bool> {
        let _guard = registry_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(mut record) = self.load_unlocked(job_id)? else {
            return Ok(false);
        };
        let Some(delivery) = record.terminal_delivery.as_mut() else {
            return Ok(false);
        };
        if matches!(delivery.state, DeliveryState::Delivered { .. }) {
            return Ok(false);
        }
        if !matches!(delivery.state, DeliveryState::Claimed { .. }) {
            return Ok(false);
        }
        let now = Utc::now();
        delivery.state = DeliveryState::Delivered { delivered_at: now };
        record.updated_at = now;
        push_audit(
            &mut record,
            now,
            "delivered",
            "terminal notification acknowledged by server dispatch",
        );
        self.save_unlocked(&record)?;
        Ok(true)
    }

    pub fn cleanup_terminal_before(&self, cutoff: DateTime<Utc>) -> Result<usize> {
        let _guard = registry_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut removed = 0;
        for record in self.list_unlocked()? {
            if record.status.is_terminal() && record.updated_at < cutoff {
                let path = self.path_for(&record.job_id);
                if path.exists() {
                    fs::remove_file(&path)
                        .with_context(|| format!("remove stale watch {}", path.display()))?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }

    fn path_for(&self, job_id: &str) -> PathBuf {
        let digest = Sha256::digest(job_id.as_bytes());
        self.root.join(format!("{}.json", hex::encode(digest)))
    }

    fn load_unlocked(&self, job_id: &str) -> Result<Option<WatchRecord>> {
        let path = self.path_for(job_id);
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(&path).with_context(|| format!("read watch {}", path.display()))?;
        let record = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse watch {}", path.display()))?;
        Ok(Some(record))
    }

    fn list_unlocked(&self) -> Result<Vec<WatchRecord>> {
        let mut records = Vec::new();
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Ok(records);
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            match fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<WatchRecord>(&bytes).ok())
            {
                Some(record) => records.push(record),
                None => crate::logging::warn(&format!(
                    "Ignoring unreadable orchestration watch {}",
                    path.display()
                )),
            }
        }
        records.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(records)
    }

    fn save_unlocked(&self, record: &WatchRecord) -> Result<()> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("create watchdog dir {}", self.root.display()))?;
        let path = self.path_for(&record.job_id);
        let temporary = path.with_extension(format!("{}.tmp", Uuid::new_v4().simple()));
        let bytes = serde_json::to_vec_pretty(record)?;
        fs::write(&temporary, bytes)
            .with_context(|| format!("write watch {}", temporary.display()))?;
        fs::rename(&temporary, &path)
            .with_context(|| format!("commit watch {}", path.display()))?;
        Ok(())
    }
}

impl Default for OrchestrationWatchdog {
    fn default() -> Self {
        Self::new()
    }
}

fn validate_spec(spec: &WatchSpec) -> Result<()> {
    if spec.job_id.trim().is_empty() {
        return Err(anyhow!("watch job_id must not be blank"));
    }
    if spec.dedupe_key.trim().is_empty() {
        return Err(anyhow!("watch dedupe_key must not be blank"));
    }
    if spec.owner_session_id.trim().is_empty() {
        return Err(anyhow!("watch owner_session_id must not be blank"));
    }
    if !spec.working_dir.is_absolute() {
        return Err(anyhow!("watch working_dir must be absolute"));
    }
    for artifact in &spec.expected_artifacts {
        validate_relative_artifact(&artifact.path)?;
    }
    Ok(())
}

fn validate_relative_artifact(path: &Path) -> Result<()> {
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(anyhow!(
            "expected artifact must stay inside the watched working directory: {}",
            path.display()
        ));
    }
    Ok(())
}

fn inspect_repository(working_dir: &Path) -> Option<RepositorySnapshot> {
    let head = Command::new("git")
        .args(["-C", working_dir.to_str()?, "rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !head.status.success() {
        return None;
    }
    let status = Command::new("git")
        .args(["-C", working_dir.to_str()?, "status", "--short"])
        .output()
        .ok()?;
    let status_summary = String::from_utf8_lossy(&status.stdout).trim().to_string();
    Some(RepositorySnapshot {
        head_sha: Some(String::from_utf8_lossy(&head.stdout).trim().to_string()),
        dirty: !status_summary.is_empty(),
        status_summary,
    })
}

fn inspect_artifacts(
    working_dir: &Path,
    expected: &[ArtifactExpectation],
) -> Result<Vec<ArtifactSnapshot>> {
    expected
        .iter()
        .map(|artifact| {
            validate_relative_artifact(&artifact.path)?;
            Ok(ArtifactSnapshot {
                path: artifact.path.clone(),
                required: artifact.required,
                exists: working_dir.join(&artifact.path).exists(),
            })
        })
        .collect()
}

fn completion_evidence(record: &WatchRecord) -> Vec<String> {
    let mut evidence = Vec::new();
    if let Some(repo) = record.repository.as_ref() {
        if let (Some(expected), Some(head)) = (record.expected_sha.as_ref(), repo.head_sha.as_ref())
            && expected == head
        {
            evidence.push(format!("repository HEAD matches expected SHA {head}"));
        } else if let (Some(baseline), Some(head)) =
            (record.baseline_sha.as_ref(), repo.head_sha.as_ref())
            && baseline != head
        {
            evidence.push(format!(
                "repository HEAD advanced from {baseline} to {head}"
            ));
        }
    }

    let required: Vec<&ArtifactSnapshot> = record
        .artifacts
        .iter()
        .filter(|artifact| artifact.required)
        .collect();
    if !required.is_empty() && required.iter().all(|artifact| artifact.exists) {
        evidence.extend(
            required
                .iter()
                .map(|artifact| format!("required artifact exists: {}", artifact.path.display())),
        );
    }
    evidence
}

fn backoff_secs(policy: &RetryPolicy, attempt: u32) -> u64 {
    let exponent = attempt.saturating_sub(1).min(31);
    policy
        .initial_backoff_secs
        .saturating_mul(1_u64 << exponent)
        .min(policy.max_backoff_secs.max(policy.initial_backoff_secs))
}

fn schedule_retry_or_fail(record: &mut WatchRecord, now: DateTime<Utc>) {
    if record.attempt < record.retry.max_attempts {
        record.status = WatchStatus::RetryScheduled;
        let delay = backoff_secs(&record.retry, record.attempt);
        record.next_check_at = Some(now + Duration::seconds(delay as i64));
        record.next_model = record
            .retry
            .model_fallbacks
            .get(record.attempt as usize)
            .cloned()
            .or_else(|| record.retry.model_fallbacks.last().cloned());
        push_audit(
            record,
            now,
            "retry_scheduled",
            &format!(
                "retry attempt {} scheduled after {}s{}",
                record.attempt + 1,
                delay,
                record
                    .next_model
                    .as_deref()
                    .map(|model| format!(" using model {model}"))
                    .unwrap_or_default()
            ),
        );
    } else {
        set_terminal(
            record,
            WatchStatus::Failed,
            now,
            "worker failed and retry budget is exhausted",
        );
    }
}

fn set_terminal(record: &mut WatchRecord, status: WatchStatus, now: DateTime<Utc>, detail: &str) {
    record.status = status;
    record.terminal_at = Some(now);
    record.next_check_at = None;
    record.active_check = None;
    record.updated_at = now;
    record
        .terminal_delivery
        .get_or_insert_with(|| TerminalDelivery {
            event_id: Uuid::new_v4().to_string(),
            state: DeliveryState::Pending,
        });
    push_audit(record, now, "terminal", detail);
}

fn push_audit(record: &mut WatchRecord, at: DateTime<Utc>, kind: &str, detail: &str) {
    if record
        .audit
        .last()
        .is_some_and(|event| event.kind == kind && event.detail == detail)
    {
        return;
    }
    record.audit.push(AuditEvent {
        at,
        kind: kind.to_string(),
        detail: detail.to_string(),
    });
    let overflow = record.audit.len().saturating_sub(MAX_AUDIT_EVENTS);
    if overflow > 0 {
        record.audit.drain(0..overflow);
    }
}

#[cfg(test)]
#[path = "orchestration_watchdog_tests.rs"]
mod tests;
