use super::*;
use std::path::Path;

async fn read_watch_output(path: &Path) -> String {
    match fs::read_to_string(path).await {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            crate::logging::warn(&format!(
                "Cannot read watchdog output {}: {error}",
                path.display()
            ));
            String::new()
        }
    }
}

fn optional_duration_secs(duration: Option<f64>) -> f64 {
    duration.unwrap_or(0.0)
}

fn elapsed_secs(started_at: chrono::DateTime<Utc>) -> f64 {
    match (Utc::now() - started_at).to_std() {
        Ok(duration) => duration.as_secs_f64(),
        Err(_) => 0.0,
    }
}

impl BackgroundTaskManager {
    pub(super) fn register_watch(&self, task_id: &str, session_id: &str, process_id: Option<u32>) {
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let spec = crate::orchestration_watchdog::WatchSpec {
            job_id: task_id.to_string(),
            dedupe_key: format!("background:{session_id}:{task_id}"),
            owner_session_id: session_id.to_string(),
            worker_session_id: None,
            process_id,
            working_dir,
            baseline_sha: None,
            expected_sha: None,
            expected_artifacts: Vec::new(),
            deadline: None,
            stale_after: chrono::Duration::days(7),
            retry: crate::orchestration_watchdog::RetryPolicy::default(),
        };
        if let Err(error) = self.watchdog.register(spec) {
            crate::logging::warn(&format!(
                "Failed to register orchestration watch for background task {task_id}: {error}"
            ));
        }
    }

    pub fn configure_watch(
        &self,
        task_id: &str,
        update: crate::orchestration_watchdog::WatchUpdate,
    ) -> Result<crate::orchestration_watchdog::WatchRecord> {
        self.watchdog.update(task_id, update)
    }

    pub(super) fn publish_completion_once(&self, mut event: BackgroundTaskCompleted) {
        use crate::orchestration_watchdog::{ReconcileObservation, RuntimeState, WatchStatus};

        let task_id = event.task_id.clone();
        let runtime = match event.status {
            BackgroundTaskStatus::Completed | BackgroundTaskStatus::Superseded => {
                RuntimeState::Completed
            }
            BackgroundTaskStatus::Failed => RuntimeState::Failed,
            BackgroundTaskStatus::Running => RuntimeState::Running,
        };
        let reconciliation = self.watchdog.reconcile(
            &task_id,
            ReconcileObservation {
                runtime_state: runtime,
                process_running: None,
                swarm_status: (event.tool_name == "swarm").then(|| "terminal".to_string()),
                retries_exhausted: true,
                observed_at: Utc::now(),
            },
        );
        let delivery_event_id = match reconciliation {
            Ok(record) => {
                if record.status == WatchStatus::Completed {
                    event.status = BackgroundTaskStatus::Completed;
                    if !record.completion_evidence.is_empty() {
                        if !event.output_preview.is_empty() {
                            event.output_preview.push_str("\n\n");
                        }
                        event.output_preview.push_str("Watchdog evidence:\n");
                        event
                            .output_preview
                            .push_str(&record.completion_evidence.join("\n"));
                    }
                }
                match self
                    .watchdog
                    .claim_terminal_delivery(&task_id, "background-task-manager")
                {
                    Ok(Some(claim)) => Some(claim.event_id),
                    Ok(None) => None,
                    Err(error) => {
                        crate::logging::warn(&format!(
                            "Failed to claim terminal delivery for background task {task_id}: {error}"
                        ));
                        Some(String::new())
                    }
                }
            }
            Err(error) => {
                // Legacy status files have no watch. Preserve their historical
                // delivery behavior instead of dropping completion.
                crate::logging::warn(&format!(
                    "Background task {task_id} has no usable orchestration watch: {error}"
                ));
                Some(String::new())
            }
        };

        if delivery_event_id.is_some() {
            Bus::global().publish(BusEvent::BackgroundTaskCompleted(event));
        }
    }

    fn status_is_reconcilable_orphan(status: &TaskStatusFile) -> bool {
        if status.status != BackgroundTaskStatus::Running || status.detached || status.pid.is_some()
        {
            return false;
        }
        let Some(owner_pid) = status.owner_pid else {
            return false;
        };
        if status.owner_instance.as_deref() == Some(model::process_instance_token()) {
            return false;
        }
        if owner_pid == std::process::id() {
            return true;
        }
        !crate::platform::is_process_running(owner_pid)
    }

    /// Finalize an orphaned non-detached `Running` status file as `Failed`.
    ///
    /// The owning process's task future died with the process (crash or
    /// exec-based server reload), so without this the file reads `Running`
    /// forever: `bg list`/`bg status` show a phantom task and `bg wait`
    /// blocks until its timeout.
    pub(super) async fn finalize_orphaned_status_if_needed(
        &self,
        mut status: TaskStatusFile,
        status_path: &std::path::Path,
    ) -> TaskStatusFile {
        if !Self::status_is_reconcilable_orphan(&status) {
            return status;
        }
        // Belt and braces: never rewrite a task this process is executing.
        if self.is_live_task(&status.task_id) {
            return status;
        }
        // A reloaded run_plan driver can lose its task future while the durable
        // swarm plan and workers remain active. The app-core watchdog refreshes
        // this observation before the orphan sweep. Preserve the status file so
        // the plan can later reconcile to its real terminal state instead of
        // reporting a false server-reload failure.
        let active_swarm_watch = match self.watchdog.get(&status.task_id) {
            Ok(record) => record.is_some_and(|record| {
                record.runtime_state == crate::orchestration_watchdog::RuntimeState::Running
                    && record.last_swarm_status.is_some()
            }),
            Err(error) => {
                crate::logging::warn(&format!(
                    "Cannot inspect swarm watch {} during orphan recovery: {error}",
                    status.task_id
                ));
                false
            }
        };
        if status.tool_name == "swarm" && active_swarm_watch {
            return status;
        }

        let completed_at = Utc::now();
        let duration_secs = Self::status_duration_secs(&status.started_at, completed_at);
        let recovered_from_evidence = match self.watchdog.reconcile(
            &status.task_id,
            crate::orchestration_watchdog::ReconcileObservation {
                runtime_state: crate::orchestration_watchdog::RuntimeState::OwnershipLost,
                process_running: Some(false),
                swarm_status: (status.tool_name == "swarm").then(|| "ownership_lost".to_string()),
                retries_exhausted: false,
                observed_at: completed_at,
            },
        ) {
            Ok(record) => record.status == crate::orchestration_watchdog::WatchStatus::Completed,
            Err(error) => {
                crate::logging::warn(&format!(
                    "Cannot reconcile orphaned background task {}: {error}",
                    status.task_id
                ));
                false
            }
        };
        let (final_status, error) = if recovered_from_evidence {
            (BackgroundTaskStatus::Completed, None)
        } else {
            (
                BackgroundTaskStatus::Failed,
                Some(
                    "Task orphaned: the owning server process exited (reloaded or crashed) before the task finished"
                        .to_string(),
                ),
            )
        };
        status.status = final_status.clone();
        let exit_code = recovered_from_evidence.then_some(0);
        status.exit_code = exit_code;
        status.error = error.clone();
        status.completed_at = Some(completed_at.to_rfc3339());
        status.duration_secs = duration_secs;
        push_task_event(
            &mut status,
            terminal_event_record(final_status.clone(), exit_code, error.as_deref()),
        );
        self.write_status_file(status_path, &status).await;

        let output_path = self.output_path_for(&status.task_id);
        let output = read_watch_output(&output_path).await;
        let output_preview = if output.len() > 500 {
            format!("{}...", crate::util::truncate_str(&output, 500))
        } else {
            output
        };
        self.publish_completion_once(BackgroundTaskCompleted {
            task_id: status.task_id.clone(),
            tool_name: status.tool_name.clone(),
            display_name: status.display_name.clone(),
            session_id: status.session_id.clone(),
            status: final_status,
            exit_code,
            output_preview,
            output_file: output_path,
            duration_secs: optional_duration_secs(duration_secs),
            notify: status.notify,
            wake: status.wake,
        });

        status
    }

    /// Startup/reload sweep: mark orphaned non-detached `Running` status
    /// files as `Failed` with a "server reloaded" note.
    ///
    /// Only owner-tagged files are considered, using the liveness rules of
    /// [`Self::status_is_reconcilable_orphan`]. Files without owner metadata
    /// (written by older builds, or by processes that legitimately still run
    /// them) are left untouched: the task dir is shared machine-wide, so
    /// without owner metadata there is no safe way to distinguish a phantom
    /// from another live process's task. Returns how many files were
    /// reconciled.
    pub async fn reconcile_orphaned_tasks(&self) -> usize {
        let mut reconciled = 0;
        let Ok(mut entries) = fs::read_dir(&self.output_dir).await else {
            return reconciled;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let Some(status) = self.read_status_file(&path).await else {
                continue;
            };
            if !Self::status_is_reconcilable_orphan(&status) {
                continue;
            }
            if self.tasks.read().await.contains_key(&status.task_id) {
                continue;
            }
            self.finalize_orphaned_status_if_needed(status, &path).await;
            reconciled += 1;
        }
        reconciled
    }

    /// Reconcile running swarm background tasks from the server's durable plan
    /// snapshot. This closes the ownership-loss gap where a run_plan driver
    /// future disappears on reload even though its worker plan later reaches a
    /// terminal state.
    pub async fn reconcile_swarm_plan_status(
        &self,
        owner_session_id: &str,
        runtime_state: crate::orchestration_watchdog::RuntimeState,
        swarm_status: String,
    ) -> usize {
        use crate::orchestration_watchdog::{ReconcileObservation, WatchStatus};

        let Ok(records) = self.watchdog.list() else {
            return 0;
        };
        let mut reconciled = 0;
        for record in records.into_iter().filter(|record| {
            record.owner_session_id == owner_session_id && !record.status.is_terminal()
        }) {
            // A live driver owns retries and terminal cleanup itself. Direct
            // plan reconciliation is only the reload/ownership-loss fallback.
            if self.is_live_task(&record.job_id) {
                continue;
            }
            let status_path = self.status_path_for(&record.job_id);
            let Some(mut status) = self.read_status_file(&status_path).await else {
                continue;
            };
            if status.tool_name != "swarm" || status.status != BackgroundTaskStatus::Running {
                continue;
            }
            let Ok(Some(lease)) = self
                .watchdog
                .claim_due_check(&record.job_id, "swarm-plan-watchdog")
            else {
                continue;
            };
            let observation = ReconcileObservation {
                runtime_state: runtime_state.clone(),
                process_running: status.pid.map(crate::platform::is_process_running),
                swarm_status: Some(swarm_status.clone()),
                retries_exhausted: runtime_state
                    == crate::orchestration_watchdog::RuntimeState::Failed,
                observed_at: Utc::now(),
            };
            let result = self.watchdog.reconcile(&record.job_id, observation);
            if let Err(error) = self.watchdog.finish_check(&record.job_id, &lease.lease_id) {
                crate::logging::warn(&format!(
                    "Cannot finish swarm watchdog check {}: {error}",
                    record.job_id
                ));
            }
            let Ok(record) = result else {
                continue;
            };
            reconciled += 1;
            if !record.status.is_terminal() {
                continue;
            }

            let completed_at = Utc::now();
            let background_status = if record.status == WatchStatus::Completed {
                BackgroundTaskStatus::Completed
            } else {
                BackgroundTaskStatus::Failed
            };
            let error = (background_status == BackgroundTaskStatus::Failed)
                .then(|| format!("Swarm plan reconciled as {}", swarm_status));
            status.status = background_status.clone();
            let exit_code = (background_status == BackgroundTaskStatus::Completed).then_some(0);
            status.exit_code = exit_code;
            status.error = error.clone();
            status.completed_at = Some(completed_at.to_rfc3339());
            status.duration_secs = Self::status_duration_secs(&status.started_at, completed_at);
            push_task_event(
                &mut status,
                terminal_event_record(background_status.clone(), exit_code, error.as_deref()),
            );
            self.write_status_file(&status_path, &status).await;

            let output_path = self.output_path_for(&status.task_id);
            let mut output_preview = read_watch_output(&output_path).await;
            if output_preview.len() > 500 {
                output_preview = format!("{}...", crate::util::truncate_str(&output_preview, 500));
            }
            if !output_preview.is_empty() {
                output_preview.push_str("\n\n");
            }
            output_preview.push_str(&format!("Swarm watchdog: {swarm_status}"));
            self.publish_completion_once(BackgroundTaskCompleted {
                task_id: status.task_id.clone(),
                tool_name: status.tool_name.clone(),
                display_name: status.display_name.clone(),
                session_id: status.session_id.clone(),
                status: background_status,
                exit_code: status.exit_code,
                output_preview,
                output_file: output_path,
                duration_secs: optional_duration_secs(status.duration_secs),
                notify: status.notify,
                wake: status.wake,
            });
        }
        reconciled
    }

    /// Reconcile every durable background watch against its persisted status,
    /// process liveness, repository, and expected artifacts. This is safe to run
    /// repeatedly and concurrently: check leases coalesce duplicate schedulers,
    /// while terminal delivery uses a durable exactly-once claim.
    pub async fn reconcile_watchdog_tasks(&self) -> usize {
        use crate::orchestration_watchdog::{ReconcileObservation, RuntimeState, WatchStatus};

        let Ok(records) = self.watchdog.list() else {
            return 0;
        };
        let mut reconciled = 0;

        for record in records {
            let status_path = self.status_path_for(&record.job_id);
            let Some(mut status) = self.read_status_file(&status_path).await else {
                if !record.status.is_terminal()
                    && let Ok(Some(lease)) = self
                        .watchdog
                        .claim_due_check(&record.job_id, "background-watchdog")
                {
                    let reconciled_record = self.watchdog.reconcile(
                        &record.job_id,
                        ReconcileObservation::runtime(RuntimeState::Unknown),
                    );
                    if let Err(error) = self.watchdog.finish_check(&record.job_id, &lease.lease_id)
                    {
                        crate::logging::warn(&format!(
                            "Cannot finish background watchdog check {}: {error}",
                            record.job_id
                        ));
                    }
                    if let Ok(reconciled_record) = reconciled_record
                        && reconciled_record.status.is_terminal()
                    {
                        let status = if reconciled_record.status == WatchStatus::Completed {
                            BackgroundTaskStatus::Completed
                        } else {
                            BackgroundTaskStatus::Failed
                        };
                        let output_preview = if reconciled_record.completion_evidence.is_empty() {
                            "Watchdog reached a terminal state after the original status file was lost."
                                .to_string()
                        } else {
                            format!(
                                "Watchdog evidence:\n{}",
                                reconciled_record.completion_evidence.join("\n")
                            )
                        };
                        self.publish_completion_once(BackgroundTaskCompleted {
                            task_id: reconciled_record.job_id.clone(),
                            tool_name: "orchestration_watchdog".to_string(),
                            display_name: Some("recovered orchestration watch".to_string()),
                            session_id: reconciled_record.owner_session_id.clone(),
                            status,
                            exit_code: None,
                            output_preview,
                            output_file: self.output_path_for(&reconciled_record.job_id),
                            duration_secs: elapsed_secs(reconciled_record.created_at),
                            notify: true,
                            wake: true,
                        });
                    }
                } else if record.status.is_terminal() {
                    let status = if record.status == WatchStatus::Completed {
                        BackgroundTaskStatus::Completed
                    } else {
                        BackgroundTaskStatus::Failed
                    };
                    self.publish_completion_once(BackgroundTaskCompleted {
                        task_id: record.job_id.clone(),
                        tool_name: "orchestration_watchdog".to_string(),
                        display_name: Some("recovered orchestration watch".to_string()),
                        session_id: record.owner_session_id.clone(),
                        status,
                        exit_code: None,
                        output_preview: if record.completion_evidence.is_empty() {
                            "Recovered terminal orchestration watch pending delivery.".to_string()
                        } else {
                            format!(
                                "Watchdog evidence:\n{}",
                                record.completion_evidence.join("\n")
                            )
                        },
                        output_file: self.output_path_for(&record.job_id),
                        duration_secs: elapsed_secs(record.created_at),
                        notify: true,
                        wake: true,
                    });
                }
                continue;
            };

            if status.status == BackgroundTaskStatus::Running {
                status = self
                    .finalize_detached_status_if_needed(status, &status_path)
                    .await;
                status = self
                    .finalize_orphaned_status_if_needed(status, &status_path)
                    .await;
            }

            if status.status == BackgroundTaskStatus::Running {
                let Ok(Some(lease)) = self
                    .watchdog
                    .claim_due_check(&record.job_id, "background-watchdog")
                else {
                    continue;
                };
                let runtime = if status.detached
                    && status.pid.is_some_and(crate::platform::is_process_running)
                {
                    RuntimeState::Running
                } else if Self::status_is_reconcilable_orphan(&status) {
                    RuntimeState::OwnershipLost
                } else {
                    RuntimeState::Running
                };
                let observation = ReconcileObservation {
                    runtime_state: runtime,
                    process_running: status.pid.map(crate::platform::is_process_running),
                    swarm_status: (status.tool_name == "swarm").then(|| "running".to_string()),
                    retries_exhausted: false,
                    observed_at: Utc::now(),
                };
                if let Err(error) = self.watchdog.reconcile(&record.job_id, observation) {
                    crate::logging::warn(&format!(
                        "Cannot reconcile background watchdog task {}: {error}",
                        record.job_id
                    ));
                }
                if let Err(error) = self.watchdog.finish_check(&record.job_id, &lease.lease_id) {
                    crate::logging::warn(&format!(
                        "Cannot finish background watchdog check {}: {error}",
                        record.job_id
                    ));
                }
                reconciled += 1;
                continue;
            }

            let output_path = self.output_path_for(&status.task_id);
            let output = read_watch_output(&output_path).await;
            let output_preview = if output.len() > 500 {
                format!("{}...", crate::util::truncate_str(&output, 500))
            } else {
                output
            };
            self.publish_completion_once(BackgroundTaskCompleted {
                task_id: status.task_id.clone(),
                tool_name: status.tool_name.clone(),
                display_name: status.display_name.clone(),
                session_id: status.session_id.clone(),
                status: status.status.clone(),
                exit_code: status.exit_code,
                output_preview,
                output_file: output_path,
                duration_secs: optional_duration_secs(status.duration_secs),
                notify: status.notify,
                wake: status.wake,
            });
            if matches!(
                record.status,
                WatchStatus::Watching | WatchStatus::RetryScheduled
            ) {
                reconciled += 1;
            }
        }

        let cutoff = Utc::now() - chrono::Duration::days(7);
        if let Err(error) = self.watchdog.cleanup_terminal_before(cutoff) {
            crate::logging::warn(&format!(
                "Failed to clean stale orchestration watches: {error}"
            ));
        }
        reconciled
    }
}
