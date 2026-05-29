use crate::types::{AutocompleteRequest, AutocompleteResponse, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidationLimits {
    pub max_client_name_bytes: usize,
    pub max_client_version_bytes: usize,
    pub max_document_uri_bytes: usize,
    pub max_language_id_bytes: usize,
    pub max_context_bytes: usize,
    pub max_selected_text_bytes: usize,
    pub max_insert_text_bytes: usize,
    pub max_deadline_ms: u64,
}

impl Default for ValidationLimits {
    fn default() -> Self {
        Self {
            max_client_name_bytes: 64,
            max_client_version_bytes: 64,
            max_document_uri_bytes: 2048,
            max_language_id_bytes: 64,
            max_context_bytes: 128 * 1024,
            max_selected_text_bytes: 64 * 1024,
            max_insert_text_bytes: 4096,
            max_deadline_ms: 30_000,
        }
    }
}

pub trait Validate {
    fn validate(&self) -> Result<(), ValidationErrors>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Error)]
#[error("{field}: {message}")]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("protocol validation failed")]
pub struct ValidationErrors(pub Vec<ValidationError>);

impl ValidationErrors {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ValidationError> {
        self.0.iter()
    }

    pub fn into_inner(self) -> Vec<ValidationError> {
        self.0
    }
}

impl Validate for AutocompleteRequest {
    fn validate(&self) -> Result<(), ValidationErrors> {
        self.validate_with_limits(ValidationLimits::default())
    }
}

impl Validate for AutocompleteResponse {
    fn validate(&self) -> Result<(), ValidationErrors> {
        self.validate_with_limits(ValidationLimits::default())
    }
}

impl AutocompleteRequest {
    pub fn validate_with_limits(&self, limits: ValidationLimits) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();

        validate_protocol_version(&mut errors, self.protocol_version);

        require_non_empty_bounded(
            &mut errors,
            "client.name",
            &self.client.name,
            limits.max_client_name_bytes,
        );
        require_non_empty_bounded(
            &mut errors,
            "client.version",
            &self.client.version,
            limits.max_client_version_bytes,
        );
        require_non_empty_bounded(
            &mut errors,
            "document.uri",
            &self.document.uri,
            limits.max_document_uri_bytes,
        );
        require_non_empty_bounded(
            &mut errors,
            "document.language_id",
            &self.document.language_id,
            limits.max_language_id_bytes,
        );

        let context_bytes = self.context.prefix.len() + self.context.suffix.len();
        if context_bytes > limits.max_context_bytes {
            errors.push(error(
                "context",
                format!(
                    "prefix + suffix must be at most {} bytes, got {context_bytes}",
                    limits.max_context_bytes
                ),
            ));
        }
        if self.context.selected_text.len() > limits.max_selected_text_bytes {
            errors.push(error(
                "context.selected_text",
                format!(
                    "must be at most {} bytes, got {}",
                    limits.max_selected_text_bytes,
                    self.context.selected_text.len()
                ),
            ));
        }

        if self.options.max_chars == 0
            || self.options.max_chars as usize > limits.max_insert_text_bytes
        {
            errors.push(error(
                "options.max_chars",
                format!(
                    "must be between 1 and {}, got {}",
                    limits.max_insert_text_bytes, self.options.max_chars
                ),
            ));
        }
        if self.options.deadline_ms == 0 || self.options.deadline_ms > limits.max_deadline_ms {
            errors.push(error(
                "options.deadline_ms",
                format!(
                    "must be between 1 and {}, got {}",
                    limits.max_deadline_ms, self.options.deadline_ms
                ),
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationErrors(errors))
        }
    }
}

impl AutocompleteResponse {
    pub fn validate_with_limits(&self, limits: ValidationLimits) -> Result<(), ValidationErrors> {
        let mut errors = Vec::new();

        match self {
            AutocompleteResponse::Ok {
                protocol_version,
                insert_text,
                confidence,
                source,
                ..
            } => {
                validate_protocol_version(&mut errors, *protocol_version);
                if insert_text.is_empty() {
                    errors.push(error("insert_text", "must not be empty for ok responses"));
                } else if insert_text.len() > limits.max_insert_text_bytes {
                    errors.push(error(
                        "insert_text",
                        format!(
                            "must be at most {} bytes, got {}",
                            limits.max_insert_text_bytes,
                            insert_text.len()
                        ),
                    ));
                }
                if !(0.0..=1.0).contains(confidence) || confidence.is_nan() {
                    errors.push(error("confidence", "must be between 0 and 1"));
                }
                require_non_empty_bounded(&mut errors, "source", source, 256);
            }
            AutocompleteResponse::NoSuggestion {
                protocol_version, ..
            }
            | AutocompleteResponse::Cancelled {
                protocol_version, ..
            } => {
                validate_protocol_version(&mut errors, *protocol_version);
            }
            AutocompleteResponse::Error {
                protocol_version,
                error: protocol_error,
                ..
            } => {
                validate_protocol_version(&mut errors, *protocol_version);
                require_non_empty_bounded(
                    &mut errors,
                    "error.message",
                    &protocol_error.message,
                    1024,
                );
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ValidationErrors(errors))
        }
    }
}

fn validate_protocol_version(errors: &mut Vec<ValidationError>, protocol_version: u32) {
    if protocol_version != PROTOCOL_VERSION {
        errors.push(error(
            "protocol_version",
            format!("expected {PROTOCOL_VERSION}, got {protocol_version}"),
        ));
    }
}

fn require_non_empty_bounded(
    errors: &mut Vec<ValidationError>,
    field: &'static str,
    value: &str,
    max_bytes: usize,
) {
    if value.trim().is_empty() {
        errors.push(error(field, "must not be empty"));
    } else if value.len() > max_bytes {
        errors.push(error(
            field,
            format!("must be at most {max_bytes} bytes, got {}", value.len()),
        ));
    }
}

fn error(field: impl Into<String>, message: impl Into<String>) -> ValidationError {
    ValidationError {
        field: field.into(),
        message: message.into(),
    }
}
