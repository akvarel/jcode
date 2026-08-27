use super::*;

pub(super) fn swarm_watch_runtime(
    summary: &crate::protocol::PlanGraphStatus,
    in_flight_workers: usize,
) -> crate::orchestration_watchdog::RuntimeState {
    use crate::orchestration_watchdog::RuntimeState;

    if summary.item_count == 0 {
        return RuntimeState::Unknown;
    }
    let terminal_count = summary.completed_ids.len() + summary.failed_ids.len();
    let no_more_runnable = summary.active_ids.is_empty()
        && summary.next_ready_ids.is_empty()
        && in_flight_workers == 0;
    if terminal_count >= summary.item_count || no_more_runnable {
        if summary.failed_ids.is_empty() {
            RuntimeState::Completed
        } else {
            RuntimeState::Failed
        }
    } else {
        RuntimeState::Running
    }
}

pub(super) fn swarm_watch_status(
    swarm_id: &str,
    summary: &crate::protocol::PlanGraphStatus,
    in_flight_workers: usize,
) -> String {
    format!(
        "swarm={swarm_id} version={} completed={} failed={} active={} ready={} in_flight={}",
        summary.version,
        summary.completed_ids.len(),
        summary.failed_ids.len(),
        summary.active_ids.len(),
        summary.next_ready_ids.len(),
        in_flight_workers
    )
}

pub(super) fn spawn(server: &Server) {
    // Continuously reconcile durable orchestration watches. The first tick
    // runs immediately at startup, recovering tasks and terminal delivery
    // after a crash/reload. Later ticks catch detached process exits,
    // ownership loss, repository/artifact completion evidence, and stale
    // watches. Check leases make duplicate scheduler loops idempotent.
    let watchdog_swarm_plans = Arc::clone(&server.swarm_state.plans);
    let watchdog_swarm_coordinators = Arc::clone(&server.swarm_state.coordinators);
    let watchdog_swarm_members = Arc::clone(&server.swarm_state.members);
    tokio::spawn(async move {
        let interval_secs = match std::env::var("JCODE_ORCHESTRATION_WATCHDOG_INTERVAL_SECS") {
            Ok(value) => match value.parse::<u64>() {
                Ok(value) if value > 0 => value,
                Ok(_) | Err(_) => {
                    crate::logging::warn(
                        "Ignoring invalid JCODE_ORCHESTRATION_WATCHDOG_INTERVAL_SECS; using 30s",
                    );
                    30
                }
            },
            Err(std::env::VarError::NotPresent) => 30,
            Err(error) => {
                crate::logging::warn(&format!(
                    "Cannot read JCODE_ORCHESTRATION_WATCHDOG_INTERVAL_SECS: {error}; using 30s"
                ));
                30
            }
        };
        let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let swarm_observations = {
                let plans = watchdog_swarm_plans.read().await;
                let coordinators = watchdog_swarm_coordinators.read().await;
                let members = watchdog_swarm_members.read().await;
                plans
                    .iter()
                    .filter_map(|(swarm_id, plan)| {
                        let owner_session_id = coordinators.get(swarm_id)?.clone();
                        let summary = crate::protocol::PlanGraphStatus::from_versioned_plan(
                            swarm_id.clone(),
                            plan,
                            Some(8),
                            Vec::new(),
                        );
                        let in_flight_workers = members
                            .values()
                            .filter(|member| {
                                member.swarm_id.as_deref() == Some(swarm_id.as_str())
                                    && member.session_id != owner_session_id
                                    && matches!(
                                        member.status.as_str(),
                                        "queued" | "running" | "running_stale"
                                    )
                                    && (member.is_headless
                                        || member.report_back_to_session_id.as_deref()
                                            == Some(owner_session_id.as_str()))
                            })
                            .count();
                        let runtime = swarm_watch_runtime(&summary, in_flight_workers);
                        let status = swarm_watch_status(swarm_id, &summary, in_flight_workers);
                        Some((owner_session_id, runtime, status))
                    })
                    .collect::<Vec<_>>()
            };
            let mut swarm_reconciled = 0;
            for (owner_session_id, runtime, status) in swarm_observations {
                swarm_reconciled += crate::background::global()
                    .reconcile_swarm_plan_status(&owner_session_id, runtime, status)
                    .await;
            }
            let orphaned = crate::background::global().reconcile_orphaned_tasks().await;
            let watched = crate::background::global().reconcile_watchdog_tasks().await;
            if swarm_reconciled > 0 || orphaned > 0 || watched > 0 {
                crate::logging::info(&format!(
                    "Orchestration watchdog reconciled {} swarm, {} orphaned, and {} watched background task(s)",
                    swarm_reconciled, orphaned, watched
                ));
            }
        }
    });
}
