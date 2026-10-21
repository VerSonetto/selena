#![allow(clippy::result_large_err)]

pub mod chat;
pub mod convert;
pub mod error;
pub mod ir;
pub mod messages;
pub mod responses;
pub mod sse;
pub mod stream;
pub mod util;

pub use convert::{request_to, response_to, validate_request, validate_response};
pub use error::{ErrorKind, ProtocolError};
pub use ir::*;
pub use sse::{SseEvent, SseParser, encode_event, encode_named};
