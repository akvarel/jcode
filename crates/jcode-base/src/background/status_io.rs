use super::TaskStatusFile;
use std::path::Path;

/// Publish complete JSON via atomic rename, never truncate a status being polled.
pub(super) fn persist_status(path: &Path, status: &TaskStatusFile) {
    if let Err(error) = crate::storage::write_json_fast(path, status) {
        crate::logging::warn(&format!("Failed to persist background status: {error}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::background::{BackgroundTaskManager, TaskResult};

    #[tokio::test]
    async fn concurrent_status_reads_never_observe_partial_json() {
        let temp = tempfile::tempdir().unwrap();
        let manager = BackgroundTaskManager::with_output_dir(temp.path().to_path_buf());
        let info = manager
            .spawn_with_notify("test", None, "atomic-status", false, false, |_| async {
                Ok(TaskResult::completed(Some(0)))
            })
            .await;
        let finished = manager
            .wait(&info.task_id, std::time::Duration::from_secs(5), false)
            .await
            .unwrap();
        let status = finished.task;
        let path = info.status_file;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let reader_path = path.clone();
        let reader_barrier = barrier.clone();
        let reader = std::thread::spawn(move || {
            reader_barrier.wait();
            for _ in 0..2000 {
                let bytes = std::fs::read(&reader_path).unwrap();
                serde_json::from_slice::<TaskStatusFile>(&bytes).unwrap();
            }
        });
        barrier.wait();
        for _ in 0..200 {
            persist_status(&path, &status);
        }
        reader.join().unwrap();
    }
}
