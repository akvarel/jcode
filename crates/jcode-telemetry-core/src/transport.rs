use jcode_logging as logging;
use serde_json::Value;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError, sync_channel};
use std::time::Duration;

pub(super) const TELEMETRY_ENDPOINT: &str = "https://telemetry.jcode.sh/v1/event";
pub(super) const TRANSCRIPT_ENDPOINT: &str = "https://telemetry.jcode.sh/v1/transcript";
const ASYNC_SEND_TIMEOUT: Duration = Duration::from_secs(5);
const BACKGROUND_QUEUE_CAPACITY: usize = 2048;
static TELEMETRY_PERMANENTLY_REJECTED: AtomicBool = AtomicBool::new(false);
static TELEMETRY_QUEUE_OVERFLOW_WARNED: AtomicBool = AtomicBool::new(false);
static TELEMETRY_BACKGROUND_SENDER: OnceLock<Result<SyncSender<Value>, String>> = OnceLock::new();
#[cfg(not(test))]
static TRANSCRIPT_BACKGROUND_SENDER: OnceLock<Result<SyncSender<Value>, String>> = OnceLock::new();
static TELEMETRY_HTTP_CLIENT: OnceLock<Result<reqwest::blocking::Client, String>> = OnceLock::new();
#[cfg(test)]
pub(super) static TEST_EMITTED_PAYLOADS: std::sync::Mutex<Vec<Value>> =
    std::sync::Mutex::new(Vec::new());

#[derive(Debug, Clone, Copy)]
pub(super) enum DeliveryMode {
    Background,
    Blocking(Duration),
}

fn http_client() -> Option<&'static reqwest::blocking::Client> {
    match TELEMETRY_HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .user_agent(jcode_provider_core::JCODE_USER_AGENT)
            .build()
            .map_err(|error| error.to_string())
    }) {
        Ok(client) => Some(client),
        Err(error) => {
            logging::warn(&format!(
                "telemetry HTTP client initialization failed: {error}"
            ));
            None
        }
    }
}

fn post_payload(payload: Value, timeout: Duration) -> bool {
    if TELEMETRY_PERMANENTLY_REJECTED.load(Ordering::Relaxed) {
        return false;
    }
    let Some(client) = http_client() else {
        return false;
    };
    match client
        .post(TELEMETRY_ENDPOINT)
        .timeout(timeout)
        .json(&payload)
        .send()
    {
        Ok(response) if response.status().is_success() => true,
        Ok(response) => {
            let status = response.status();
            if telemetry_status_is_permanent(status.as_u16()) {
                TELEMETRY_PERMANENTLY_REJECTED.store(true, Ordering::Relaxed);
                logging::warn(&format!(
                    "telemetry endpoint permanently rejected payload with HTTP {status}; suppressing telemetry delivery for this process"
                ));
            } else {
                logging::warn(&format!(
                    "telemetry endpoint temporarily rejected payload with HTTP {status}"
                ));
            }
            false
        }
        Err(error) => {
            logging::warn(&format!("telemetry payload send failed: {error}"));
            false
        }
    }
}

#[cfg(not(test))]
fn post_transcript_payload(payload: Value, timeout: Duration) -> bool {
    let Some(client) = http_client() else {
        return false;
    };
    match client
        .post(TRANSCRIPT_ENDPOINT)
        .timeout(timeout)
        .json(&payload)
        .send()
    {
        Ok(response) if response.status().is_success() => true,
        Ok(response) => {
            logging::warn(&format!(
                "transcript endpoint rejected upload with HTTP {}",
                response.status()
            ));
            false
        }
        Err(error) => {
            logging::warn(&format!("transcript upload failed: {error}"));
            false
        }
    }
}

pub(super) fn telemetry_status_is_permanent(status: u16) -> bool {
    (400..500).contains(&status) && !matches!(status, 408 | 425 | 429)
}

pub(super) fn spawn_background_worker<F>(
    capacity: usize,
    mut deliver: F,
) -> std::io::Result<SyncSender<Value>>
where
    F: FnMut(Value) + Send + 'static,
{
    let (sender, receiver) = sync_channel(capacity);
    std::thread::Builder::new()
        .name("jcode-telemetry".to_string())
        .spawn(move || {
            while let Ok(payload) = receiver.recv() {
                deliver(payload);
            }
        })?;
    Ok(sender)
}

fn initialized_sender(
    slot: &'static OnceLock<Result<SyncSender<Value>, String>>,
    label: &str,
    initialize: impl FnOnce() -> std::io::Result<SyncSender<Value>>,
) -> Option<&'static SyncSender<Value>> {
    match slot.get_or_init(|| initialize().map_err(|error| error.to_string())) {
        Ok(sender) => Some(sender),
        Err(error) => {
            logging::warn(&format!(
                "{label} background worker failed to start: {error}"
            ));
            None
        }
    }
}

fn background_sender() -> Option<&'static SyncSender<Value>> {
    initialized_sender(&TELEMETRY_BACKGROUND_SENDER, "telemetry", || {
        spawn_background_worker(BACKGROUND_QUEUE_CAPACITY, |payload| {
            post_payload(payload, ASYNC_SEND_TIMEOUT);
        })
    })
}

#[cfg(not(test))]
fn transcript_background_sender() -> Option<&'static SyncSender<Value>> {
    initialized_sender(
        &TRANSCRIPT_BACKGROUND_SENDER,
        "transcript telemetry",
        || {
            spawn_background_worker(64, |payload| {
                post_transcript_payload(payload, ASYNC_SEND_TIMEOUT);
            })
        },
    )
}

pub(super) fn send_transcript_payload(payload: Value) -> bool {
    #[cfg(test)]
    {
        if let Ok(mut emitted) = TEST_EMITTED_PAYLOADS.lock() {
            emitted.push(payload);
        }
        true
    }
    #[cfg(not(test))]
    {
        let Some(sender) = transcript_background_sender() else {
            return false;
        };
        match sender.try_send(payload) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                logging::warn("transcript upload queue is full; dropping transcript");
                false
            }
            Err(TrySendError::Disconnected(_)) => {
                logging::warn("transcript upload worker stopped; dropping transcript");
                false
            }
        }
    }
}

pub(super) fn send_payload(payload: Value, mode: DeliveryMode) -> bool {
    #[cfg(test)]
    if let Ok(mut emitted) = TEST_EMITTED_PAYLOADS.lock() {
        emitted.push(payload.clone());
    }
    match mode {
        DeliveryMode::Background => send_in_background(payload),
        DeliveryMode::Blocking(timeout) => send_blocking(payload, timeout),
    }
}

fn send_in_background(payload: Value) -> bool {
    if TELEMETRY_PERMANENTLY_REJECTED.load(Ordering::Relaxed) {
        return false;
    }
    logging::debug("queueing telemetry payload for background delivery");
    let Some(sender) = background_sender() else {
        return false;
    };
    match sender.try_send(payload) {
        Ok(()) => {
            TELEMETRY_QUEUE_OVERFLOW_WARNED.store(false, Ordering::Relaxed);
            true
        }
        Err(TrySendError::Full(_)) => {
            if !TELEMETRY_QUEUE_OVERFLOW_WARNED.swap(true, Ordering::Relaxed) {
                logging::warn(&format!(
                    "telemetry background queue is full (capacity={BACKGROUND_QUEUE_CAPACITY}); dropping events until delivery catches up"
                ));
            }
            false
        }
        Err(TrySendError::Disconnected(_)) => {
            logging::warn("telemetry background worker stopped; dropping payload");
            false
        }
    }
}

fn send_blocking(payload: Value, timeout: Duration) -> bool {
    logging::debug(&format!(
        "sending telemetry payload with blocking timeout={}ms",
        timeout.as_millis()
    ));
    if tokio::runtime::Handle::try_current().is_ok() {
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            if sender.send(post_payload(payload, timeout)).is_err() {
                logging::debug("telemetry blocking response receiver was dropped");
            }
        });
        receiver.recv_timeout(timeout).unwrap_or(false)
    } else {
        post_payload(payload, timeout)
    }
}
