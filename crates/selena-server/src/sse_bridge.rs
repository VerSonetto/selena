use axum::body::Body;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use bytes::Bytes;
use futures::stream::{self, Stream, StreamExt};
use selena_protocol::ir::Protocol;
use selena_protocol::sse::{SseEvent, SseParser, encode_event};
use selena_protocol::stream::{StreamDecoder, StreamEncoder, decoder_for, encoder_for};
use selena_protocol::{CanonicalStreamEvent, ProtocolError};
use std::collections::VecDeque;
use std::convert::Infallible;
use std::pin::Pin;

type UpstreamStream = Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>;

struct BridgeState {
    upstream: UpstreamStream,
    parser: SseParser,
    decoder: Box<dyn StreamDecoder>,
    encoder: Box<dyn StreamEncoder>,
    pending: VecDeque<Bytes>,
    done: bool,
}

pub fn bridge_response(
    upstream: reqwest::Response,
    ingress: Protocol,
    upstream_protocol: Protocol,
    chat_include_usage: bool,
) -> Response {
    let state = BridgeState {
        upstream: Box::pin(upstream.bytes_stream()),
        parser: SseParser::new(),
        decoder: decoder_for(upstream_protocol),
        encoder: encoder_for(ingress, chat_include_usage),
        pending: VecDeque::new(),
        done: false,
    };

    let stream = stream::unfold(state, |mut state| async move {
        loop {
            if let Some(bytes) = state.pending.pop_front() {
                return Some((Ok::<_, Infallible>(bytes), state));
            }
            if state.done {
                return None;
            }

            match state.upstream.next().await {
                Some(Ok(chunk)) => {
                    let parsed = state.parser.push(&chunk);
                    for event in parsed {
                        match event {
                            Ok(event) => process_event(&mut state, event),
                            Err(error) => {
                                enqueue_error(&mut state, error);
                                break;
                            }
                        }
                    }
                }
                Some(Err(error)) => {
                    enqueue_error(
                        &mut state,
                        ProtocolError::stream(format!("upstream stream error: {error}")),
                    );
                }
                None => {
                    if let Some(event) = state.parser.finish() {
                        match event {
                            Ok(event) => process_event(&mut state, event),
                            Err(error) => enqueue_error(&mut state, error),
                        }
                    }
                    if !state.done {
                        match state.decoder.finish() {
                            Ok(events) => {
                                for event in events {
                                    encode_canonical(&mut state, &event);
                                }
                            }
                            Err(error) => enqueue_error(&mut state, error),
                        }
                    }
                    if !state.done {
                        if let Ok(events) = state.encoder.finish() {
                            for event in events {
                                queue_event(&mut state, &event);
                            }
                        }
                        state.done = true;
                    }
                }
            }
        }
    });

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/event-stream; charset=utf-8"),
            (header::CACHE_CONTROL, "no-cache"),
            (header::CONNECTION, "keep-alive"),
            (header::HeaderName::from_static("x-accel-buffering"), "no"),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

fn process_event(state: &mut BridgeState, event: SseEvent) {
    match state.decoder.decode(&event) {
        Ok(events) => {
            for event in events {
                encode_canonical(state, &event);
                if state.done {
                    return;
                }
            }
        }
        Err(error) => enqueue_error(state, error),
    }
}

fn encode_canonical(state: &mut BridgeState, event: &CanonicalStreamEvent) {
    match state.encoder.encode(event) {
        Ok(events) => {
            for event in events {
                queue_event(state, &event);
            }
        }
        Err(error) => enqueue_error(state, error),
    }
}

fn enqueue_error(state: &mut BridgeState, error: ProtocolError) {
    if state.done {
        return;
    }
    state.done = true;
    if let Ok(events) = state.encoder.encode(&CanonicalStreamEvent::Error { error }) {
        for event in events {
            queue_event(state, &event);
        }
    }
    if let Ok(events) = state.encoder.finish() {
        for event in events {
            queue_event(state, &event);
        }
    }
}

fn queue_event(state: &mut BridgeState, event: &SseEvent) {
    state.pending.push_back(Bytes::from(encode_event(event)));
}
