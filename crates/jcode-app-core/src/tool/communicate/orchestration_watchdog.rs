use super::*;

/// Drive `run_plan` as a managed background task and return immediately.
///
/// The coordinating agent stays responsive: the plan loop runs inside the
/// shared `BackgroundTaskManager` (task id, progress card, `bg` tool
/// integration), and completion is delivered through the standard notify/wake
/// path like any other background task.
pub(super) async fn run_swarm_plan_in_background(
    ctx: &ToolContext,
    params: CommunicateInput,
) -> Result<ToolOutput> {
    // Validate the plan inline so an empty/broken plan errors immediately
    // instead of as a delayed background failure.
    let initial_summary = fetch_plan_status(&ctx.session_id).await?;
    if initial_summary.item_count == 0 {
        return Ok(ToolOutput::new("No swarm plan items to run."));
    }

    // Refuse to start a second driver for the same session: two concurrent
    // run_plan loops would race on assignments and double-spawn workers. The
    // claim is check-and-insert under one lock, so two run_plan calls in the
    // same batch cannot both pass. Only drivers live in this process count; a
    // stale "running" status file left by a server reload must not block
    // restarting the driver (the claim map is per-process and dead task ids
    // fail the is_live_task check).
    let manager = crate::background::global();
    let claim = match try_claim_run_plan_driver(manager, &ctx.session_id) {
        RunPlanDriverClaimResult::Claimed(claim) => claim,
        RunPlanDriverClaimResult::AlreadyRunning(existing) => {
            return Ok(ToolOutput::new(match existing {
                Some(task_id) => format!(
                    "A swarm run_plan driver is already running for this session (task {}). \
                     Check it with `bg action=\"status\" task_id=\"{}\"` or `swarm plan_status` instead of starting another.",
                    task_id, task_id
                ),
                None => "A swarm run_plan driver is already starting for this session. \
                         Check it with `swarm plan_status` instead of starting another."
                    .to_string(),
            }));
        }
    };

    let notify = params.notify.unwrap_or(true);
    let wake = params.wake.unwrap_or(true);
    let model_fallbacks = params
        .model_fallbacks
        .clone()
        .unwrap_or(Vec::with_capacity(0));
    let max_retries = params.max_retries.unwrap_or(model_fallbacks.len() as u32);
    let retry_backoff_secs = params.retry_backoff_secs.unwrap_or(30).max(1);
    let retry_models_for_run = model_fallbacks.clone();
    let initial_model = params.model.clone();
    let expected_sha = params.expected_sha.clone();
    let expected_artifacts = params
        .expected_artifacts
        .clone()
        .unwrap_or(Vec::with_capacity(0));
    let working_dir = match ctx.working_dir.clone() {
        Some(path) => Some(path),
        None => match std::env::current_dir() {
            Ok(path) => Some(path),
            Err(error) => {
                crate::logging::warn(&format!(
                    "Cannot resolve run_plan working directory for watchdog registration: {error}"
                ));
                None
            }
        },
    };
    let watch_deadline = chrono::Utc::now()
        + chrono::Duration::minutes(params.timeout_minutes.unwrap_or(60).max(1) as i64);
    // Keep the display name free of the "·" separator used by the background
    // notification markdown header, or downstream parsing mis-splits the label.
    let display_name = format!(
        "run_plan ({} nodes, {} mode)",
        initial_summary.item_count, initial_summary.mode
    );

    let bg_ctx = ctx.clone();
    let info = crate::background::global()
        .spawn_with_notify(
            "swarm",
            Some(display_name.clone()),
            &ctx.session_id,
            notify,
            wake,
            move |output_path| async move {
                let reporter = RunPlanReporter::background(&output_path);
                let mut run_params = params;
                let mut retry_index = 0_u32;
                loop {
                    match run_swarm_plan_to_terminal(&bg_ctx, &run_params, &reporter).await {
                        Ok(output) => {
                            reporter.finalize(&output.output).await;
                            break Ok(TaskResult::completed(Some(0)));
                        }
                        Err(error) if retry_index < max_retries => {
                            let delay = retry_backoff_secs
                                .saturating_mul(1_u64 << retry_index.min(31))
                                .min(30 * 60);
                            let next_model = retry_models_for_run
                                .get(retry_index as usize)
                                .cloned();
                            if let Some(model) = next_model.as_ref() {
                                run_params.model = Some(model.clone());
                            }
                            retry_index += 1;
                            reporter
                                .checkpoint(&format!(
                                    "run_plan driver failed: {error}. Retrying attempt {}/{} in {}s{}.",
                                    retry_index,
                                    max_retries,
                                    delay,
                                    match next_model.as_deref() {
                                        Some(model) => format!(" with model {model}"),
                                        None => String::new(),
                                    }
                                ))
                                .await;
                            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                        }
                        Err(error) => {
                            let message = format!("run_plan failed: {}", error);
                            reporter.finalize(&message).await;
                            break Ok(TaskResult::failed(None, message));
                        }
                    }
                }
            },
        )
        .await;
    let mut watch_models = Vec::new();
    watch_models.push(initial_model.unwrap_or_else(|| "<inherited>".to_string()));
    watch_models.extend(model_fallbacks);
    let watch_update = crate::orchestration_watchdog::WatchUpdate {
        working_dir,
        expected_sha,
        expected_artifacts: Some(
            expected_artifacts
                .into_iter()
                .map(crate::orchestration_watchdog::ArtifactExpectation::required)
                .collect(),
        ),
        deadline: Some(watch_deadline),
        stale_after: Some(chrono::Duration::hours(24)),
        retry: Some(crate::orchestration_watchdog::RetryPolicy {
            max_attempts: max_retries.saturating_add(1),
            initial_backoff_secs: retry_backoff_secs,
            max_backoff_secs: 30 * 60,
            model_fallbacks: watch_models,
        }),
        ..crate::orchestration_watchdog::WatchUpdate::default()
    };
    if let Err(error) = crate::background::global().configure_watch(&info.task_id, watch_update) {
        crate::logging::warn(&format!(
            "Failed to configure run_plan orchestration watch {}: {}",
            info.task_id, error
        ));
    }
    claim.record_task(&info.task_id);

    let delivery_note = if wake {
        "You'll be woken with the result when the plan reaches a terminal state."
    } else if notify {
        "A notification will appear when the plan reaches a terminal state."
    } else {
        "Notifications disabled. Use the `bg` tool to check status."
    };
    let output = format!(
        "🐝 Swarm plan running in background.\n\n\
         Task ID: {}\n\
         Plan: {} node(s), {} mode\n\
         Output file: {}\n\n\
         {}\n\
         Check progress: use the `bg` tool with action=\"status\" and task_id=\"{}\", or `swarm plan_status`.\n\
         Note: a server reload stops this driver (workers keep running); rerun `swarm run_plan` to resume driving the same plan.",
        info.task_id,
        initial_summary.item_count,
        initial_summary.mode,
        info.output_file.display(),
        delivery_note,
        info.task_id,
    );

    Ok(ToolOutput::new(output)
        .with_title(format!("Swarm run_plan in background: {}", info.task_id))
        .with_metadata(json!({
            "background": true,
            "swarm": true,
            "task_id": info.task_id,
            "display_name": display_name,
            "output_file": info.output_file.to_string_lossy(),
            "status_file": info.status_file.to_string_lossy(),
        })))
}
