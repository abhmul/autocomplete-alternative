//! Protocol types for the local autocomplete broker.

mod types;
mod validation;

pub mod schema;

pub use types::{
    AutocompleteContext, AutocompleteMode, AutocompleteOptions, AutocompleteRequest,
    AutocompleteResponse, CancelResponse, CancelStatus, ClientInfo, CursorPosition, DocumentInfo,
    ErrorCode, HealthResponse, HealthStatus, PROTOCOL_VERSION, ProtocolError, ProviderHealth,
    ProviderStatus, ReloadResponse, ReloadStatus, ResponseMetadata, TriggerKind,
};
pub use validation::{Validate, ValidationError, ValidationErrors, ValidationLimits};
