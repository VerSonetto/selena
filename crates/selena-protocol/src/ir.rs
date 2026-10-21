use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub const DEFAULT_MESSAGES_MAX_TOKENS: u32 = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Chat,
    Responses,
    Messages,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat_completions",
            Self::Responses => "responses",
            Self::Messages => "anthropic_messages",
        }
    }

    pub fn endpoint_path(self) -> &'static str {
        match self {
            Self::Chat => "/v1/chat/completions",
            Self::Responses => "/v1/responses",
            Self::Messages => "/v1/messages",
        }
    }

    pub fn provider_path(self) -> &'static str {
        match self {
            Self::Chat => "/chat/completions",
            Self::Responses => "/responses",
            Self::Messages => "/messages",
        }
    }

    pub fn default_error_type(self) -> &'static str {
        match self {
            Self::Chat | Self::Responses => "invalid_request_error",
            Self::Messages => "invalid_request_error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    System,
    Developer,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text {
        text: String,
    },
    Image {
        url: Option<String>,
        data: Option<String>,
        file_id: Option<String>,
        media_type: Option<String>,
        detail: Option<String>,
    },
    File {
        file_data: Option<String>,
        file_id: Option<String>,
        file_url: Option<String>,
        filename: Option<String>,
        media_type: Option<String>,
    },
    Audio {
        data: String,
        format: String,
        transcript: Option<String>,
    },
    Refusal {
        refusal: String,
    },
    Thinking {
        thinking: String,
        signature: Option<String>,
        redacted: bool,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
        raw_arguments: Option<String>,
    },
    ToolResult {
        tool_use_id: String,
        content: Vec<ContentPart>,
        is_error: bool,
    },
    Unknown {
        protocol: Protocol,
        value: Value,
    },
}

impl ContentPart {
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    pub fn plain_text(&self) -> Option<&str> {
        match self {
            Self::Text { text } => Some(text),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemBlock {
    pub role: Role,
    pub content: Vec<ContentPart>,
}

impl SystemBlock {
    pub fn text(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            content: vec![ContentPart::text(text)],
        }
    }

    pub fn as_text(&self) -> String {
        self.content
            .iter()
            .filter_map(ContentPart::plain_text)
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentPart>,
    pub name: Option<String>,
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn text(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            content: vec![ContentPart::text(text)],
            name: None,
            tool_call_id: None,
        }
    }

    pub fn tool_result(tool_use_id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            role: Role::Tool,
            content: vec![ContentPart::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: vec![ContentPart::text(text)],
                is_error: false,
            }],
            name: None,
            tool_call_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub input: Value,
    pub raw_arguments: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub content: Vec<ContentPart>,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tool_type", rename_all = "snake_case")]
pub enum ToolDef {
    Function {
        name: String,
        description: Option<String>,
        parameters: Value,
        strict: Option<bool>,
        output_schema: Option<Value>,
    },
    Custom {
        name: String,
        description: Option<String>,
        format: Option<Value>,
    },
    Hosted {
        protocol: Protocol,
        kind: String,
        raw: Value,
    },
}

impl ToolDef {
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Function { name, .. } | Self::Custom { name, .. } => Some(name),
            Self::Hosted { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolChoice {
    #[default]
    Auto,
    None,
    Required,
    Function {
        name: String,
    },
    Custom {
        name: String,
    },
    Hosted {
        protocol: Protocol,
        raw: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    #[default]
    Text,
    JsonObject,
    JsonSchema {
        name: String,
        description: Option<String>,
        schema: Value,
        strict: Option<bool>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CachePolicy {
    pub key: Option<String>,
    pub retention: Option<String>,
    pub control_ttl: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StateOptions {
    pub store: Option<bool>,
    pub previous_response_id: Option<String>,
    pub conversation: Option<String>,
    pub background: Option<bool>,
    pub include: Vec<String>,
    pub truncation: Option<String>,
    pub max_tool_calls: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReasoningOptions {
    pub effort: Option<String>,
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    HostedTools,
    ServerTools,
    ResponsesState,
    ItemReference,
    Background,
    Include,
    Truncation,
    ReasoningItems,
    ThinkingBlocks,
    LegacyThinkingBudget,
    ServerToolHistory,
    UnknownContent(String),
    Audio,
    MultipleChoices,
    AllowedTools,
    PromptTemplate,
    ToolResultMedia,
    FileUrl,
    ImageFileId,
    Citations,
    Logprobs,
    CacheControl,
    ServiceTier,
    Moderation,
    Prediction,
    CustomToolGrammar,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalRequest {
    pub model: String,
    pub system: Vec<SystemBlock>,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDef>,
    pub tool_choice: ToolChoice,
    pub parallel_tool_calls: Option<bool>,
    pub response_format: ResponseFormat,
    pub max_output_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub stop: Vec<String>,
    pub stream: bool,
    pub metadata: BTreeMap<String, String>,
    pub user: Option<String>,
    pub safety_identifier: Option<String>,
    pub service_tier: Option<String>,
    pub reasoning: ReasoningOptions,
    pub cache: CachePolicy,
    pub state: StateOptions,
    pub n: u32,
    pub logprobs: bool,
    pub top_logprobs: Option<u32>,
    pub presence_penalty: Option<f64>,
    pub frequency_penalty: Option<f64>,
    pub seed: Option<i64>,
    pub capabilities: BTreeSet<Capability>,
    pub source: Protocol,
}

impl CanonicalRequest {
    pub fn new(source: Protocol, model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            system: Vec::new(),
            messages: Vec::new(),
            tools: Vec::new(),
            tool_choice: ToolChoice::Auto,
            parallel_tool_calls: None,
            response_format: ResponseFormat::Text,
            max_output_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            stop: Vec::new(),
            stream: false,
            metadata: BTreeMap::new(),
            user: None,
            safety_identifier: None,
            service_tier: None,
            reasoning: ReasoningOptions::default(),
            cache: CachePolicy::default(),
            state: StateOptions::default(),
            n: 1,
            logprobs: false,
            top_logprobs: None,
            presence_penalty: None,
            frequency_penalty: None,
            seed: None,
            capabilities: BTreeSet::new(),
            source,
        }
    }

    pub fn mark(&mut self, capability: Capability) {
        self.capabilities.insert(capability);
    }

    pub fn has(&self, capability: &Capability) -> bool {
        self.capabilities.contains(capability)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputItem {
    Message {
        id: String,
        role: Role,
        content: Vec<ContentPart>,
        annotations: Vec<Value>,
    },
    ToolCall {
        id: String,
        call_id: String,
        name: String,
        arguments: String,
        status: String,
    },
    Reasoning {
        id: String,
        summary: Vec<String>,
        content: Vec<String>,
        encrypted_content: Option<String>,
        signature: Option<String>,
    },
    Hosted {
        protocol: Protocol,
        kind: String,
        raw: Value,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Completed,
    Incomplete,
    Failed,
    InProgress,
    Cancelled,
    Queued,
}

impl ResponseStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Incomplete => "incomplete",
            Self::Failed => "failed",
            Self::InProgress => "in_progress",
            Self::Cancelled => "cancelled",
            Self::Queued => "queued",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    ToolUse,
    ContentFilter,
    StopSequence(Option<String>),
    Refusal,
    Pause,
    ContextExceeded,
    Unknown(String),
}

impl StopReason {
    pub fn as_chat_finish_reason(&self) -> &'static str {
        match self {
            Self::EndTurn | Self::StopSequence(_) | Self::Pause | Self::Unknown(_) => "stop",
            Self::MaxTokens | Self::ContextExceeded => "length",
            Self::ToolUse => "tool_calls",
            Self::ContentFilter | Self::Refusal => "content_filter",
        }
    }

    pub fn as_messages_stop_reason(&self) -> &'static str {
        match self {
            Self::EndTurn | Self::Unknown(_) => "end_turn",
            Self::MaxTokens => "max_tokens",
            Self::ToolUse => "tool_use",
            Self::ContentFilter | Self::Refusal => "refusal",
            Self::StopSequence(_) => "stop_sequence",
            Self::Pause => "pause_turn",
            Self::ContextExceeded => "model_context_window_exceeded",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub total_tokens: Option<u64>,
    pub cached_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub reasoning_tokens: Option<u64>,
    pub server_tool_use: Option<Value>,
}

impl Usage {
    pub fn total_or_sum(&self) -> Option<u64> {
        self.total_tokens
            .or_else(|| Some(self.input_tokens.unwrap_or(0) + self.output_tokens.unwrap_or(0)))
    }

    pub fn is_empty(&self) -> bool {
        self.input_tokens.is_none() && self.output_tokens.is_none() && self.total_tokens.is_none()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanonicalResponse {
    pub id: String,
    pub model: String,
    pub created_at: i64,
    pub status: ResponseStatus,
    pub stop_reason: StopReason,
    pub incomplete_reason: Option<String>,
    pub output: Vec<OutputItem>,
    pub usage: Usage,
    pub error: Option<crate::error::ProtocolError>,
    pub metadata: BTreeMap<String, String>,
    pub service_tier: Option<String>,
    pub source: Protocol,
}

impl CanonicalResponse {
    pub fn new(source: Protocol, id: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            model: model.into(),
            created_at: chrono::Utc::now().timestamp(),
            status: ResponseStatus::Completed,
            stop_reason: StopReason::EndTurn,
            incomplete_reason: None,
            output: Vec::new(),
            usage: Usage::default(),
            error: None,
            metadata: BTreeMap::new(),
            service_tier: None,
            source,
        }
    }

    pub fn effective_stop_reason(&self) -> StopReason {
        if matches!(
            self.stop_reason,
            StopReason::EndTurn | StopReason::Unknown(_)
        ) && self
            .output
            .iter()
            .any(|item| matches!(item, OutputItem::ToolCall { .. }))
        {
            StopReason::ToolUse
        } else {
            self.stop_reason.clone()
        }
    }

    pub fn text(&self) -> String {
        let mut out = String::new();
        for item in &self.output {
            if let OutputItem::Message { content, .. } = item {
                for part in content {
                    if let ContentPart::Text { text } = part {
                        out.push_str(text);
                    }
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CanonicalStreamEvent {
    Start {
        id: String,
        model: String,
        created_at: i64,
    },
    TextDelta {
        item_id: String,
        output_index: u32,
        content_index: u32,
        delta: String,
    },
    TextDone {
        item_id: String,
        output_index: u32,
        content_index: u32,
        text: String,
    },
    RefusalDelta {
        item_id: String,
        output_index: u32,
        content_index: u32,
        delta: String,
    },
    RefusalDone {
        item_id: String,
        output_index: u32,
        content_index: u32,
        refusal: String,
    },
    ToolStart {
        item_id: String,
        output_index: u32,
        tool_call_id: String,
        name: String,
    },
    ToolArgsDelta {
        item_id: String,
        output_index: u32,
        delta: String,
    },
    ToolArgsDone {
        item_id: String,
        output_index: u32,
        arguments: String,
    },
    ReasoningDelta {
        item_id: String,
        output_index: u32,
        content_index: u32,
        delta: String,
    },
    Usage {
        usage: Usage,
    },
    Finish {
        status: ResponseStatus,
        stop_reason: StopReason,
        incomplete_reason: Option<String>,
    },
    Error {
        error: crate::error::ProtocolError,
    },
    Ping,
    End,
}
