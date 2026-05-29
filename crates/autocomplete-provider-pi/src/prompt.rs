use autocomplete_core::{ProviderDiagnostics, ProviderError};
use autocomplete_protocol::AutocompleteRequest;
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPrompt {
    pub system_prompt: String,
    pub request_json: String,
}

impl RenderedPrompt {
    pub fn for_completion(request: &AutocompleteRequest) -> Result<Self, ProviderError> {
        Self::for_completion_with_system_prompt(request, None)
    }

    pub fn for_completion_with_system_prompt(
        request: &AutocompleteRequest,
        system_prompt: Option<String>,
    ) -> Result<Self, ProviderError> {
        let request_json = serde_json::to_string(&json!({
            "task": "inline_autocomplete",
            "contract": {
                "return": "JSON only",
                "schema": {
                    "insert_text": "string containing only text to insert at the cursor",
                    "confidence": "number between 0 and 1",
                    "reason": "optional short internal diagnostic"
                },
                "rules": [
                    "Do not return markdown fences.",
                    "Do not return commentary or explanations.",
                    "Do not repeat text that is already before the cursor.",
                    "Return an empty insert_text when no safe completion is available."
                ]
            },
            "request": request,
        }))
        .map_err(|error| {
            ProviderError::failed(
                format!("failed to render pi request JSON: {error}"),
                ProviderDiagnostics::default(),
            )
        })?;

        Ok(Self {
            system_prompt: system_prompt.unwrap_or_else(completion_system_prompt),
            request_json,
        })
    }

    pub fn for_repair(&self, parse_error: &str) -> Self {
        let request_json = serde_json::to_string(&json!({
            "task": "repair_autocomplete_response_json",
            "parse_error": parse_error,
            "original_request_json": self.request_json,
            "required_schema": {
                "insert_text": "string",
                "confidence": "number between 0 and 1",
                "reason": "optional string"
            }
        }))
        .unwrap_or_else(|_| "{\"task\":\"repair_autocomplete_response_json\"}".to_owned());

        Self {
            system_prompt: repair_system_prompt(),
            request_json,
        }
    }
}

fn completion_system_prompt() -> String {
    "You are an inline autocomplete provider. Return only valid JSON with keys insert_text, confidence, and optional reason. The insert_text must be exactly the text to insert at the cursor: no markdown fences, no prose, no explanations, and no repeated prefix.".to_owned()
}

fn repair_system_prompt() -> String {
    "Repair the previous malformed inline-autocomplete provider output. Return only valid JSON matching {\"insert_text\": string, \"confidence\": number, \"reason\": optional string}. No markdown or commentary.".to_owned()
}
