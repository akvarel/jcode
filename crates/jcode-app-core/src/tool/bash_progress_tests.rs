use super::*;
use crate::bus::{BackgroundTaskProgressSource, BackgroundTaskStatus};
use serde_json::json;

fn make_ctx() -> ToolContext {
    ToolContext {
        session_id: "test-session".to_string(),
        message_id: "test-msg".to_string(),
        tool_call_id: "test-call".to_string(),
        working_dir: Some(std::path::PathBuf::from("/tmp")),
        stdin_request_tx: None,
        graceful_shutdown_signal: None,
        execution_mode: crate::tool::ToolExecutionMode::Direct,
    }
}

fn make_agent_ctx(signal: jcode_agent_runtime::InterruptSignal) -> ToolContext {
    ToolContext {
        session_id: "test-session".to_string(),
        message_id: "test-msg".to_string(),
        tool_call_id: "test-call-agent".to_string(),
        working_dir: Some(std::path::PathBuf::from("/tmp")),
        stdin_request_tx: None,
        graceful_shutdown_signal: Some(signal),
        execution_mode: crate::tool::ToolExecutionMode::AgentTurn,
    }
}

#[test]
fn parse_progress_line_classifies_markers_checkpoints_and_heuristics() {
    let update = parse_progress_line(r#"JCODE_PROGRESS {"percent":40,"message":"Working"}"#)
        .expect("parser should not fail")
        .expect("progress marker should parse");
    match update {
        ProgressLineUpdate::Progress(progress) => assert_eq!(progress.percent, Some(40.0)),
        other => panic!("expected a progress update, got {other:?}"),
    }

    let update = parse_progress_line(r#"JCODE_CHECKPOINT {"message":"Tests passed"}"#)
        .expect("parser should not fail")
        .expect("checkpoint marker should parse");
    match update {
        ProgressLineUpdate::Checkpoint(progress) => {
            assert_eq!(progress.message.as_deref(), Some("Tests passed"))
        }
        other => panic!("expected a checkpoint update, got {other:?}"),
    }

    let update = parse_progress_line("Copied 7/10 files")
        .expect("parser should not fail")
        .expect("heuristic ratio should parse");
    match update {
        ProgressLineUpdate::Progress(progress) => {
            assert_eq!(progress.percent, Some(70.0));
            assert_eq!(progress.source, BackgroundTaskProgressSource::ParsedOutput);
        }
        other => panic!("expected a progress update, got {other:?}"),
    }

    assert!(
        parse_progress_line("plain log line with no progress")
            .expect("parser should not fail")
            .is_none(),
        "non-progress output must not produce updates"
    );
}

/// The bug this guards against: a foreground command promoted to background at
/// the timeout showed 0% until it completed, because nothing parsed its output
/// for progress. Both the update emitted *before* promotion and updates
/// emitted *after* promotion must reach the background task's status.
#[tokio::test]
async fn test_timeout_promoted_command_reports_intermediate_progress() {
    let tool = BashTool::new();
    // Emits 10% before the 300ms foreground timeout, then 80% about 2s in.
    let input = json!({
        "command": "echo 'progress 10% done'; sleep 2; echo 'progress 80% done'; sleep 1",
        "timeout": 300,
    });
    let ctx = make_ctx();

    let result = tool
        .execute(input, ctx)
        .await
        .expect("timeout should promote to background");
    let metadata = result.metadata.expect("expected background metadata");
    assert_eq!(metadata["timeout_promoted"], true);
    let task_id = metadata["task_id"]
        .as_str()
        .expect("task_id should be present")
        .to_string();

    // The pre-promotion update (10%) must be attached at promotion time, and
    // the post-promotion update (80%) must stream in while still running.
    let mut observed: Vec<f32> = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        let status = crate::background::global()
            .status(&task_id)
            .await
            .expect("status should exist");
        if let Some(percent) = status.progress.as_ref().and_then(|p| p.percent)
            && observed.last() != Some(&percent)
        {
            observed.push(percent);
        }
        if observed.contains(&80.0) {
            break;
        }
        if status.status != BackgroundTaskStatus::Running {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    assert!(
        observed.contains(&80.0),
        "promoted task should reach 80% via parsed output, saw {observed:?}"
    );
    assert!(
        observed.contains(&10.0),
        "the pre-promotion 10% update should be flushed at promotion, saw {observed:?}"
    );

    let _ = crate::background::global().cancel(&task_id).await;
}

/// Same guarantee for the reload-persistable (detached) path: the command
/// writes straight to its output file, so a follower must translate progress
/// lines into status updates while the task is still running.
#[tokio::test]
async fn test_detached_promoted_command_reports_intermediate_progress() {
    let tool = BashTool::new();
    let signal = jcode_agent_runtime::InterruptSignal::new();
    let ctx = make_agent_ctx(signal);

    let result = tool
        .execute(
            json!({
                "command": "sleep 0.5; echo 'done 3/10 steps'; sleep 2; echo 'done 8/10 steps'; sleep 1",
                "timeout": 200,
            }),
            ctx,
        )
        .await
        .expect("timeout should promote the detached command to background");
    let metadata = result.metadata.expect("expected background metadata");
    assert_eq!(metadata["timeout_promoted"], true);
    let task_id = metadata["task_id"]
        .as_str()
        .expect("task_id should be present")
        .to_string();

    let mut observed: Vec<f32> = Vec::new();
    let mut saw_intermediate_while_running = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        let status = crate::background::global()
            .status(&task_id)
            .await
            .expect("status should exist");
        if let Some(percent) = status.progress.as_ref().and_then(|p| p.percent) {
            if observed.last() != Some(&percent) {
                observed.push(percent);
            }
            if status.status == BackgroundTaskStatus::Running && percent < 100.0 {
                saw_intermediate_while_running = true;
            }
        }
        if observed.contains(&80.0) {
            break;
        }
        if status.status != BackgroundTaskStatus::Running {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    assert!(
        observed.contains(&30.0) && observed.contains(&80.0),
        "detached task should report 30% then 80% from parsed output, saw {observed:?}"
    );
    assert!(
        saw_intermediate_while_running,
        "intermediate progress must be visible while the task is still running"
    );

    let output_file = std::path::PathBuf::from(
        metadata["output_file"]
            .as_str()
            .expect("output_file should be present"),
    );
    let status_file = std::path::PathBuf::from(
        metadata["status_file"]
            .as_str()
            .expect("status_file should be present"),
    );
    let _ = crate::background::global().cancel(&task_id).await;
    let _ = tokio::fs::remove_file(output_file).await;
    let _ = tokio::fs::remove_file(status_file).await;
}
