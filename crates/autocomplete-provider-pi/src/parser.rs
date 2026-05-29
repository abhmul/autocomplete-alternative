use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, Copy, Default)]
pub struct PiEventStreamParser;

impl PiEventStreamParser {
    pub fn final_assistant_text(&self, stdout: &str) -> Result<String, PiEventStreamError> {
        let mut last_text = None;
        let mut saw_nonempty_line = false;

        for line in stdout
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            saw_nonempty_line = true;
            let value = serde_json::from_str::<Value>(line).map_err(|error| {
                PiEventStreamError::InvalidJsonLine {
                    line: line.to_owned(),
                    message: error.to_string(),
                }
            })?;
            if let Some(text) = assistant_text_from_value(&value) {
                last_text = Some(text);
            }
        }

        if !saw_nonempty_line {
            return Err(PiEventStreamError::NoAssistantMessage);
        }

        last_text.ok_or(PiEventStreamError::NoAssistantMessage)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PiEventStreamError {
    #[error("pi JSON stream contained invalid JSON line {line:?}: {message}")]
    InvalidJsonLine { line: String, message: String },
    #[error("pi JSON stream did not contain an assistant message")]
    NoAssistantMessage,
}

fn assistant_text_from_value(value: &Value) -> Option<String> {
    if looks_like_assistant_message(value) {
        if let Some(text) = content_text(value.get("content")) {
            return Some(text);
        }
        if let Some(text) = content_text(value.get("text")) {
            return Some(text);
        }
        if let Some(text) = content_text(value.get("output")) {
            return Some(text);
        }
    }

    for key in ["message", "data", "payload", "result", "event"] {
        if let Some(nested) = value.get(key)
            && let Some(text) = assistant_text_from_value(nested)
        {
            return Some(text);
        }
    }

    None
}

fn looks_like_assistant_message(value: &Value) -> bool {
    role_is_assistant(value)
        || value
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind.contains("assistant"))
        || (value
            .get("type")
            .and_then(Value::as_str)
            .is_some_and(|kind| kind.contains("message"))
            && (value.get("content").is_some() || value.get("text").is_some()))
}

fn role_is_assistant(value: &Value) -> bool {
    value
        .get("role")
        .and_then(Value::as_str)
        .is_some_and(|role| role == "assistant")
}

fn content_text(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Array(items) => {
            let mut output = String::new();
            for item in items {
                if let Some(text) = item.get("text").and_then(Value::as_str) {
                    output.push_str(text);
                } else if let Some(text) = item.as_str() {
                    output.push_str(text);
                }
            }
            (!output.is_empty()).then_some(output)
        }
        Value::Object(object) => object
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::PiEventStreamParser;

    #[test]
    fn parser_extracts_last_assistant_content_from_nested_events() {
        let stdout = r#"
{"type":"session","id":"s"}
{"type":"message","message":{"role":"assistant","content":[{"type":"text","text":"first"}]}}
{"type":"message","message":{"role":"assistant","content":"second"}}
"#;

        assert_eq!(
            PiEventStreamParser::default()
                .final_assistant_text(stdout)
                .expect("assistant text"),
            "second"
        );
    }
}
