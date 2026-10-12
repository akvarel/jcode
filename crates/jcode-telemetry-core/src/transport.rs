#[cfg(not(test))]
use jcode_logging as logging;
use serde_json::Value;
#[cfg(not(test))]
use std::sync::OnceLock;
#[cfg(not(test))]
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
#[cfg(not(test))]
use std::sync::mpsc::TrySendError;
use std::sync::mpsc::{SyncSender, sync_channel};
use std::time::Duration;

pub(super) const TELEMETRY_ENDPOINT: &str = "https://telemetry.jcode.sh/v1/event";
pub(super) const TRANSCRIPT_ENDPOINT: &str = "https://telemetry.jcode.sh/v1/transcript";
#[cfg(not(test))]
const ASYNC_SEND_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(not(test))]
const BACKGROUND_QUEUE_CAPACITY: usize = 2048;
#[cfg(not(test))]
static TELEMETRY_PERMANENTLY_REJECTED: AtomicBool = AtomicBool::new(false);
#[cfg(not(test))]
static TELEMETRY_QUEUE_OVERFLOW_WARNED: AtomicBool = AtomicBool::new(false);
static TELEMETRY_IN_FLIGHT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
#[cfg(not(test))]
static TELEMETRY_BACKGROUND_SENDER: OnceLock<Result<SyncSender<Value>, String>> = OnceLock::new();
#[cfg(not(test))]
static TRANSCRIPT_BACKGROUND_SENDER: OnceLock<Result<SyncSender<Value>, String>> = OnceLock::new();
#[cfg(not(test))]
static TELEMETRY_HTTP_CLIENT: OnceLock<Result<reqwest::blocking::Client, String>> = OnceLock::new();
#[cfg(test)]
pub(super) static TEST_EMITTED_PAYLOADS: std::sync::Mutex<Vec<Value>> =
    std::sync::Mutex::new(Vec::new());

#[derive(Debug, Clone, Copy)]
pub(super) enum DeliveryMode {
    Background,
    Blocking(Duration),
}

/// Wait for queued telemetry delivery, bounded by the caller's deadline.
pub fn flush_pending(timeout: Duration) {
    let deadline = std::time::Instant::now() + timeout;
    while TELEMETRY_IN_FLIGHT.load(Ordering::Acquire) > 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(test)]
#[test]
fn flush_pending_respects_deadline_and_drained_queue() {
    TELEMETRY_IN_FLIGHT.store(1, Ordering::Release);
    flush_pending(Duration::ZERO);
    assert_eq!(TELEMETRY_IN_FLIGHT.load(Ordering::Acquire), 1);
    TELEMETRY_IN_FLIGHT.store(0, Ordering::Release);
    flush_pending(Duration::from_secs(1));
    assert_eq!(TELEMETRY_IN_FLIGHT.load(Ordering::Acquire), 0);
}

#[cfg(not(test))]
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

#[cfg(not(test))]
fn post_payload(payload: Value, timeout: Duration) -> bool {
    if TELEMETRY_PERMANENTLY_REJECTED.load(Ordering::Relaxed) {
        return false;
    }
    let Some(client) = http_client() else {
        return false;
    };
    // Newer event types can reach an older worker that rejects them as unknown.
    // That must only drop the one payload, never disable all telemetry.
    let breaker_exempt = payload_is_breaker_exempt(&payload);
    match client
        .post(TELEMETRY_ENDPOINT)
        .timeout(timeout)
        .json(&payload)
        .send()
    {
        Ok(response) if response.status().is_success() => true,
        Ok(response) => {
            let status = response.status();
            if telemetry_status_is_permanent(status.as_u16()) && !breaker_exempt {
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

/// Event types added after the worker's validation was frozen. A 4xx for one of
/// these means "this worker does not know the event yet", not "stop sending".
pub(super) fn payload_is_breaker_exempt(payload: &Value) -> bool {
    matches!(
        payload.get("event").and_then(|value| value.as_str()),
        Some("usage_report" | "desktop_active" | "desktop_upgrade" | "desktop_update")
    )
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

#[cfg(not(test))]
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

#[cfg(not(test))]
fn background_sender() -> Option<&'static SyncSender<Value>> {
    initialized_sender(&TELEMETRY_BACKGROUND_SENDER, "telemetry", || {
        spawn_background_worker(BACKGROUND_QUEUE_CAPACITY, |payload| {
            post_payload_with_retry(payload, ASYNC_SEND_TIMEOUT);
            TELEMETRY_IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
        })
    })
}

#[cfg(not(test))]
fn post_payload_with_retry(payload: Value, timeout: Duration) -> bool {
    if post_payload(payload.clone(), timeout) {
        return true;
    }
    for delay in [Duration::from_millis(200), Duration::from_millis(800)] {
        if TELEMETRY_PERMANENTLY_REJECTED.load(Ordering::Relaxed) {
            return false;
        }
        std::thread::sleep(delay);
        if post_payload(payload.clone(), timeout) {
            return true;
        }
    }
    false
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

pub(super) fn send_payload(mut payload: Value, mode: DeliveryMode) -> bool {
    super::concurrency::mark_legacy_concurrency_unavailable(&mut payload);
    #[cfg(test)]
    super::tests::TEST_DELIVERY_MODES.lock().unwrap().push(mode);
    #[cfg(test)]
    if let Ok(mut emitted) = TEST_EMITTED_PAYLOADS.lock() {
        emitted.push(payload);
    }
    #[cfg(test)]
    {
        true
    }
    #[cfg(not(test))]
    match mode {
        DeliveryMode::Background => send_in_background(payload),
        DeliveryMode::Blocking(timeout) => send_blocking(payload, timeout),
    }
}

#[cfg(not(test))]
fn send_in_background(payload: Value) -> bool {
    if TELEMETRY_PERMANENTLY_REJECTED.load(Ordering::Relaxed) {
        return false;
    }
    logging::debug("queueing telemetry payload for background delivery");
    let Some(sender) = background_sender() else {
        return false;
    };
    TELEMETRY_IN_FLIGHT.fetch_add(1, Ordering::AcqRel);
    match sender.try_send(payload) {
        Ok(()) => {
            TELEMETRY_QUEUE_OVERFLOW_WARNED.store(false, Ordering::Relaxed);
            true
        }
        Err(TrySendError::Full(_)) => {
            TELEMETRY_IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
            if !TELEMETRY_QUEUE_OVERFLOW_WARNED.swap(true, Ordering::Relaxed) {
                logging::warn(&format!(
                    "telemetry background queue is full (capacity={BACKGROUND_QUEUE_CAPACITY}); dropping events until delivery catches up"
                ));
            }
            false
        }
        Err(TrySendError::Disconnected(_)) => {
            TELEMETRY_IN_FLIGHT.fetch_sub(1, Ordering::AcqRel);
            logging::warn("telemetry background worker stopped; dropping payload");
            false
        }
    }
}

#[cfg(not(test))]
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
