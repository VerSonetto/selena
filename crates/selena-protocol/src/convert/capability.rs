use crate::error::ProtocolError;
use crate::ir::{CanonicalRequest, CanonicalResponse, Capability, OutputItem, Protocol};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionWarnings(pub Vec<String>);

impl ConversionWarnings {
    pub fn push(&mut self, message: impl Into<String>) {
        self.0.push(message.into());
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

pub fn validate_request(
    request: &CanonicalRequest,
    target: Protocol,
) -> Result<ConversionWarnings, ProtocolError> {
    if request.source == target {
        return Ok(ConversionWarnings(Vec::new()));
    }
    let mut warnings = ConversionWarnings(Vec::new());
    for capability in &request.capabilities {
        match capability {
            Capability::HostedTools => {
                return Err(ProtocolError::unsupported(
                    "hosted_tools",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::ServerTools => {
                return Err(ProtocolError::unsupported(
                    "server_tools",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::ResponsesState => {
                return Err(ProtocolError::unsupported(
                    "responses_state",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::ItemReference => {
                return Err(ProtocolError::unsupported(
                    "item_reference",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::Background => {
                return Err(ProtocolError::unsupported(
                    "background",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::Include => {
                return Err(ProtocolError::unsupported(
                    "include",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::Truncation => {
                return Err(ProtocolError::unsupported(
                    "truncation",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::ReasoningItems => {
                return Err(ProtocolError::unsupported(
                    "reasoning_items",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::ThinkingBlocks => {
                warnings.push(
                    "thinking blocks and signatures are dropped; the thinking budget is approximated as reasoning effort",
                );
            }
            Capability::Audio => {
                return Err(ProtocolError::unsupported(
                    "audio",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::MultipleChoices => {
                return Err(ProtocolError::unsupported(
                    "multiple_choices",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::AllowedTools => {
                return Err(ProtocolError::unsupported(
                    "allowed_tools",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::PromptTemplate => {
                return Err(ProtocolError::unsupported(
                    "prompt_template",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::Moderation => {
                return Err(ProtocolError::unsupported(
                    "moderation",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::Prediction => {
                return Err(ProtocolError::unsupported(
                    "prediction",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::Unknown => {
                return Err(ProtocolError::unsupported(
                    "unknown_native_feature",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::ToolResultMedia if target == Protocol::Chat => {
                return Err(ProtocolError::unsupported(
                    "tool_result_media",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::FileUrl if target == Protocol::Chat => {
                return Err(ProtocolError::unsupported(
                    "file_url",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::ImageFileId if target == Protocol::Chat => {
                return Err(ProtocolError::unsupported(
                    "image_file_id",
                    request.source.as_str(),
                    target.as_str(),
                ));
            }
            Capability::ToolResultMedia => warnings.push("tool_result_media may be flattened"),
            Capability::FileUrl => warnings.push("file_url will be converted where supported"),
            Capability::ImageFileId => warnings.push("image file_id may be re-uploaded"),
            Capability::Logprobs => warnings.push("logprobs are not representable in the target"),
            Capability::CacheControl => warnings.push("cache control semantics are approximate"),
            Capability::ServiceTier => warnings.push("service tier is approximated"),
            Capability::Citations => warnings.push("citations may be dropped"),
            Capability::CustomToolGrammar => warnings.push("custom tool grammar is approximated"),
        }
    }
    Ok(warnings)
}

pub fn validate_response(
    response: &CanonicalResponse,
    target: Protocol,
) -> Result<ConversionWarnings, ProtocolError> {
    if response.source == target {
        return Ok(ConversionWarnings(Vec::new()));
    }
    let mut warnings = ConversionWarnings(Vec::new());
    for item in &response.output {
        match item {
            OutputItem::Reasoning { .. } => {
                return Err(ProtocolError::unsupported(
                    "reasoning_output",
                    response.source.as_str(),
                    target.as_str(),
                ));
            }
            OutputItem::Hosted { .. } => {
                return Err(ProtocolError::unsupported(
                    "hosted_output",
                    response.source.as_str(),
                    target.as_str(),
                ));
            }
            OutputItem::Message { content, .. } => {
                if content.iter().any(|part| {
                    matches!(
                        part,
                        crate::ir::ContentPart::Audio { .. }
                            | crate::ir::ContentPart::Thinking { .. }
                    )
                }) {
                    return Err(ProtocolError::unsupported(
                        "native_response_content",
                        response.source.as_str(),
                        target.as_str(),
                    ));
                }
            }
            OutputItem::ToolCall { .. } => {}
        }
    }
    if response.usage.server_tool_use.is_some() {
        warnings.push("server tool usage is not representable");
    }
    Ok(warnings)
}
