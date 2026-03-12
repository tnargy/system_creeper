use std::collections::VecDeque;

use reqwest::Client;
use shared::MetricPayload;

/// Handles buffered, retry-on-failure HTTP delivery of metric payloads.
///
/// On each tick the caller pushes a freshly collected payload via
/// [`Sender::send_with_retry`].  The sender appends it to the internal
/// `VecDeque` and then tries to drain the queue oldest-first.  A successful
/// `200` response removes the entry; any error (network failure or non-2xx
/// status) stops processing for this tick so the remaining payloads are
/// retried next time.
///
/// The buffer is bounded: it holds at most
/// `buffer_duration_secs / interval_secs` entries.  When it would exceed that
/// limit the *oldest* entry is dropped and a warning is logged.  The agent
/// never panics due to buffer overflow.
pub struct Sender {
    client: Client,
    collector_url: String,
    buffer: VecDeque<MetricPayload>,
    /// Maximum number of payloads to keep in memory.
    capacity: usize,
}

impl Sender {
    /// Create a new `Sender`.
    ///
    /// `capacity` should be `buffer_duration_secs / interval_secs` (integer
    /// division, minimum 1).
    pub fn new(collector_url: String, buffer_duration_secs: u64, interval_secs: u64) -> Self {
        let capacity = (buffer_duration_secs / interval_secs.max(1)).max(1) as usize;
        Self {
            client: Client::new(),
            collector_url,
            buffer: VecDeque::with_capacity(capacity + 1),
            capacity,
        }
    }

    /// Enqueue `payload` and attempt to flush the buffer to the collector.
    ///
    /// This is called once per collection tick.  The function is `async` so it
    /// can `await` the HTTP calls without blocking the scheduler.
    pub async fn send_with_retry(&mut self, payload: MetricPayload) {
        // Enforce capacity *before* pushing so the new payload is always kept
        // even when the buffer is at its limit.
        if self.buffer.len() >= self.capacity {
            let dropped = self.buffer.pop_front();
            if let Some(ref p) = dropped {
                tracing::warn!(
                    agent_id = %p.agent_id,
                    timestamp = %p.timestamp,
                    buffer_capacity = self.capacity,
                    "buffer full — dropping oldest payload",
                );
            }
        }
        self.buffer.push_back(payload);

        // Drain oldest-first; stop on the first failure.
        while let Some(pending) = self.buffer.front() {
            match self.try_send(pending).await {
                Ok(()) => {
                    self.buffer.pop_front();
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        buffered = self.buffer.len(),
                        "send failed — will retry next interval",
                    );
                    break;
                }
            }
        }
    }

    /// Send a single payload to the collector.
    ///
    /// Returns `Ok(())` on HTTP 200; `Err(String)` for network errors or any
    /// non-2xx status code.
    async fn try_send(&self, payload: &MetricPayload) -> Result<(), String> {
        let response = self
            .client
            .post(&self.collector_url)
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("network error: {e}"))?;

        let status = response.status();
        if status.is_success() {
            tracing::debug!(
                agent_id = %payload.agent_id,
                timestamp = %payload.timestamp,
                "payload delivered",
            );
            Ok(())
        } else {
            Err(format!("collector returned {status}"))
        }
    }
}
