use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutocompleteRequest {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub client: ClientInfo,
    pub document: DocumentInfo,
    pub cursor: CursorPosition,
    pub context: AutocompleteContext,
    pub options: AutocompleteOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DocumentInfo {
    pub uri: String,
    pub language_id: String,
    pub version: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CursorPosition {
    pub line: u64,
    pub character: u64,
    pub offset: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutocompleteContext {
    pub prefix: String,
    pub suffix: String,
    #[serde(default)]
    pub selected_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AutocompleteOptions {
    pub mode: AutocompleteMode,
    pub max_chars: u32,
    pub deadline_ms: u64,
    pub trigger: TriggerKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum AutocompleteResponse {
    Ok {
        protocol_version: u32,
        request_id: Uuid,
        insert_text: String,
        confidence: f64,
        source: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<ResponseMetadata>,
    },
    NoSuggestion {
        protocol_version: u32,
        request_id: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<ResponseMetadata>,
    },
    Cancelled {
        protocol_version: u32,
        request_id: Uuid,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<ResponseMetadata>,
    },
    Error {
        protocol_version: u32,
        request_id: Uuid,
        error: ProtocolError,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata: Option<ResponseMetadata>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResponseMetadata {
    pub latency_ms: u64,
    pub provider_latency_ms: u64,
    pub postprocessed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProtocolError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HealthResponse {
    pub protocol_version: u32,
    pub status: HealthStatus,
    pub provider: ProviderHealth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Ok,
    Degraded,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProviderHealth {
    pub name: String,
    pub status: ProviderStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProviderStatus {
    Available,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CancelResponse {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub status: CancelStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CancelStatus {
    Cancelled,
    NotFound,
    AlreadyCompleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReloadResponse {
    pub protocol_version: u32,
    pub status: ReloadStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReloadStatus {
    Reloaded,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    UnsupportedProtocolVersion,
    InvalidRequest,
    ContextTooLarge,
    MaxCharsOutOfRange,
    DeadlineOutOfRange,
    ProviderTimeout,
    ProviderError,
    ProviderMalformedOutput,
    Cancelled,
    InternalError,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutocompleteMode {
    InlineTab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    Idle,
    Manual,
    DocumentChange,
}
