pub mod capability;

use crate::chat;
use crate::error::ProtocolError;
use crate::ir::{CanonicalRequest, CanonicalResponse, Protocol};
use crate::messages;
use crate::responses;
use serde_json::Value;

pub use capability::{ConversionWarnings, validate_request, validate_response};

pub fn request_to(
    request: &CanonicalRequest,
    target: Protocol,
) -> Result<(Value, ConversionWarnings), ProtocolError> {
    let warnings = validate_request(request, target)?;
    let value = match target {
        Protocol::Chat => chat::encode_request(request)?,
        Protocol::Responses => responses::encode_request(request)?,
        Protocol::Messages => messages::encode_request(request)?,
    };
    Ok((value, warnings))
}

pub fn response_to(
    response: &CanonicalResponse,
    target: Protocol,
) -> Result<(Value, ConversionWarnings), ProtocolError> {
    let warnings = validate_response(response, target)?;
    let value = match target {
        Protocol::Chat => chat::encode_response(response)?,
        Protocol::Responses => responses::encode_response(response)?,
        Protocol::Messages => messages::encode_response(response)?,
    };
    Ok((value, warnings))
}
