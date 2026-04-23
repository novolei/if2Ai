//! API Module - Provider Management
//! Migrated from /rust/crates/api
//! Handles LLM provider integration and routing

pub mod api_type;
mod client;
mod error;
pub mod providers;
mod sse;
mod types;

#[allow(unused_imports)]
pub use api_type::ApiType;

// Re-export key types
#[allow(unused_imports)]
pub use client::{
    oauth_token_is_expired, read_base_url, read_xai_base_url, resolve_saved_oauth_token,
    resolve_startup_auth_source, MessageStream, OAuthTokenSet, ProviderClient,
};
#[allow(unused_imports)]
pub use error::ApiError;
#[allow(unused_imports)]
pub use providers::claw_provider::{
    read_model_override, AuthSource, ClawApiClient, ClawApiClient as ApiClient,
};
#[allow(unused_imports)]
pub use providers::manager::{MockProvider, ProviderManager};
#[allow(unused_imports)]
pub use providers::openai_compat::{OpenAiCompatClient, OpenAiCompatConfig};
#[allow(unused_imports)]
pub use providers::{
    detect_provider_kind, max_tokens_for_model, resolve_model_alias, ProviderKind,
};
#[allow(unused_imports)]
pub use sse::{parse_frame, SseParser};
#[allow(unused_imports)]
pub use types::{
    ContentBlockDelta, ContentBlockDeltaEvent, ContentBlockStartEvent, ContentBlockStopEvent,
    ImageSource, InputContentBlock, InputMessage, MessageDelta, MessageDeltaEvent, MessageRequest,
    MessageResponse, MessageStartEvent, MessageStopEvent, OutputContentBlock, StreamEvent,
    ToolChoice, ToolDefinition, ToolResultContentBlock, Usage,
};
