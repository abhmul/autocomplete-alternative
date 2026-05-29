use crate::types::{AutocompleteRequest, AutocompleteResponse};
use schemars::schema_for;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub fn autocomplete_request_schema() -> Value {
    serde_json::to_value(schema_for!(AutocompleteRequest)).expect("request schema serializes")
}

pub fn autocomplete_response_schema() -> Value {
    serde_json::to_value(schema_for!(AutocompleteResponse)).expect("response schema serializes")
}

pub fn all_schemas() -> BTreeMap<&'static str, Value> {
    BTreeMap::from([
        (
            "autocomplete-request.v1.schema.json",
            autocomplete_request_schema(),
        ),
        (
            "autocomplete-response.v1.schema.json",
            autocomplete_response_schema(),
        ),
    ])
}

pub fn export_schema_files(
    output_dir: impl AsRef<Path>,
) -> Result<Vec<PathBuf>, SchemaExportError> {
    let output_dir = output_dir.as_ref();
    std::fs::create_dir_all(output_dir)?;

    let mut written = Vec::new();
    for (file_name, schema) in all_schemas() {
        let path = output_dir.join(file_name);
        let json = serde_json::to_string_pretty(&schema)?;
        std::fs::write(&path, json)?;
        written.push(path);
    }
    Ok(written)
}

#[derive(Debug, Error)]
pub enum SchemaExportError {
    #[error("schema I/O failed")]
    Io(#[from] std::io::Error),
    #[error("schema serialization failed")]
    Json(#[from] serde_json::Error),
}
