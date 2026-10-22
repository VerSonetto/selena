use bytes::Bytes;
use futures::stream::Stream;
use selena_core::SharedCore;
use selena_core::models::RequestLogCompletion;
use selena_protocol::ir::{Protocol, Usage};
use selena_protocol::sse::{SseEvent, SseParser};
use selena_protocol::stream::{StreamDecoder, decoder_for};
use selena_protocol::{CanonicalStreamEvent, ProtocolError};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Instant;

#[derive(Default)]
struct MeterState {
    usage: Usage,
    usage_seen: bool,
    ttft_ms: Option<i64>,
    first_byte_ms: Option<i64>,
    error: Option<(String, String)>,
    warnings: Vec<String>,
    finalized: bool,
}

#[derive(Clone)]
pub struct StreamMeter {
    core: SharedCore,
    log_id: String,
    started: Instant,
    state: Arc<Mutex<MeterState>>,
}

impl StreamMeter {
    pub fn new(core: SharedCore, log_id: String, started: Instant, warnings: Vec<String>) -> Self {
        Self {
            core,
            log_id,
            started,
            state: Arc::new(Mutex::new(MeterState {
                warnings,
                ..MeterState::default()
            })),
        }
    }

    pub fn observe(&self, event: &CanonicalStreamEvent) {
        let mut state = self.state.lock().unwrap();
        match event {
            CanonicalStreamEvent::Usage { usage } => {
                state.usage = usage.clone();
                state.usage_seen = true;
            }
            CanonicalStreamEvent::TextDelta { .. }
            | CanonicalStreamEvent::RefusalDelta { .. }
            | CanonicalStreamEvent::ToolStart { .. }
            | CanonicalStreamEvent::ToolArgsDelta { .. } => {
                if state.ttft_ms.is_none() {
                    state.ttft_ms = Some(self.started.elapsed().as_millis() as i64);
                }
            }
            CanonicalStreamEvent::Error { error } => {
                state.error = Some((
                    error
                        .code
                        .clone()
                        .unwrap_or_else(|| error.kind.openai_type().to_string()),
                    error.message.clone(),
                ));
            }
            _ => {}
        }
    }

    pub fn observe_error(&self, error: &ProtocolError) {
        let mut state = self.state.lock().unwrap();
        state.error = Some((
            error
                .code
                .clone()
                .unwrap_or_else(|| error.kind.openai_type().to_string()),
            error.message.clone(),
        ));
    }

    pub fn observe_parse_error(&self, message: impl Into<String>) {
        let mut state = self.state.lock().unwrap();
        let message = message.into();
        if !state.warnings.iter().any(|warning| warning == &message) {
            state.warnings.push(message);
        }
    }

    fn note_first_byte(&self) {
        let mut state = self.state.lock().unwrap();
        if state.first_byte_ms.is_none() {
            state.first_byte_ms = Some(self.started.elapsed().as_millis() as i64);
        }
    }

    pub fn finish(&self, completed: bool) {
        let (usage, usage_seen, ttft_ms, first_byte_ms, error, mut warnings) = {
            let mut state = self.state.lock().unwrap();
            if state.finalized {
                return;
            }
            state.finalized = true;
            (
                state.usage.clone(),
                state.usage_seen,
                state.ttft_ms,
                state.first_byte_ms,
                state.error.clone(),
                state.warnings.clone(),
            )
        };
        if !completed {
            warnings.push("stream ended before the upstream response completed".to_string());
        }
        let latency_ms = self.started.elapsed().as_millis() as i64;
        let total_tokens = usage.total_tokens.or_else(|| {
            if usage.input_tokens.is_some() || usage.output_tokens.is_some() {
                Some(usage.input_tokens.unwrap_or(0) + usage.output_tokens.unwrap_or(0))
            } else {
                None
            }
        });
        let completion = RequestLogCompletion {
            status_code: if error.is_some() { 502 } else { 200 },
            latency_ms,
            ttft_ms: ttft_ms.or(first_byte_ms),
            input_tokens: usage.input_tokens.map(|value| value as i64),
            output_tokens: usage.output_tokens.map(|value| value as i64),
            total_tokens: total_tokens.map(|value| value as i64),
            cached_tokens: usage.cached_tokens.map(|value| value as i64),
            cache_write_tokens: usage.cache_write_tokens.map(|value| value as i64),
            reasoning_tokens: usage.reasoning_tokens.map(|value| value as i64),
            conversion_warnings: warnings,
            error_code: error.as_ref().map(|(code, _)| code.clone()),
            error_message: error.map(|(_, message)| message.chars().take(2000).collect()),
        };
        if !usage_seen {
            tracing::debug!(
                "stream completed without provider usage for request {}",
                self.log_id
            );
        }
        let core = self.core.clone();
        let log_id = self.log_id.clone();
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                if let Err(error) = core.update_request_log(&log_id, completion).await {
                    tracing::warn!("failed to finalize request log: {error}");
                }
            });
        }
    }
}

pub struct MeteredStream<S> {
    inner: S,
    meter: StreamMeter,
    parser: SseParser,
    decoder: Box<dyn StreamDecoder>,
    completed: bool,
    meter_events: bool,
}

impl<S> MeteredStream<S> {
    pub fn passthrough(inner: S, protocol: Protocol, meter: StreamMeter) -> Self {
        Self {
            inner,
            meter,
            parser: SseParser::new(),
            decoder: decoder_for(protocol),
            completed: false,
            meter_events: true,
        }
    }

    fn observe_events(&mut self, events: Vec<SseEvent>) {
        if !self.meter_events {
            return;
        }
        for event in events {
            match self.decoder.decode(&event) {
                Ok(events) => {
                    for event in events {
                        self.meter.observe(&event);
                    }
                }
                Err(error) => {
                    self.meter
                        .observe_parse_error(format!("stream observation warning: {error}"));
                    self.meter_events = false;
                }
            }
        }
    }

    fn observe_chunk(&mut self, chunk: &Bytes) {
        if !self.meter_events {
            return;
        }
        self.meter.note_first_byte();
        let events = self.parser.push(chunk);
        for event in events {
            match event {
                Ok(event) => self.observe_events(vec![event]),
                Err(error) => {
                    self.meter
                        .observe_parse_error(format!("stream observation warning: {error}"));
                    self.meter_events = false;
                }
            }
        }
    }

    fn complete(&mut self, completed: bool) {
        if self.completed {
            return;
        }
        self.completed = true;
        if completed && self.meter_events {
            if let Some(event) = self.parser.finish() {
                match event {
                    Ok(event) => self.observe_events(vec![event]),
                    Err(error) => self
                        .meter
                        .observe_parse_error(format!("stream observation warning: {error}")),
                }
            }
            if let Ok(events) = self.decoder.finish() {
                for event in events {
                    self.meter.observe(&event);
                }
            }
        }
        self.meter.finish(completed);
    }
}

impl<S, E> Stream for MeteredStream<S>
where
    S: Stream<Item = Result<Bytes, E>> + Unpin,
{
    type Item = Result<Bytes, E>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        match Pin::new(&mut this.inner).poll_next(cx) {
            Poll::Ready(Some(Ok(chunk))) => {
                this.observe_chunk(&chunk);
                Poll::Ready(Some(Ok(chunk)))
            }
            Poll::Ready(Some(Err(error))) => {
                this.meter
                    .observe_parse_error("upstream stream ended with a transport error");
                this.complete(true);
                Poll::Ready(Some(Err(error)))
            }
            Poll::Ready(None) => {
                this.complete(true);
                Poll::Ready(None)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S> Drop for MeteredStream<S> {
    fn drop(&mut self) {
        self.complete(false);
    }
}
