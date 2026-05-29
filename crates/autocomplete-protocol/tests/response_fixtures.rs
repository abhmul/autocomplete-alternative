use autocomplete_protocol::{AutocompleteResponse, ErrorCode, Validate, schema};
use serde_json::Value;

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/../../examples/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(path).expect("fixture is readable");
    serde_json::from_str(&text).expect("fixture is valid json")
}

#[test]
fn response_fixtures_round_trip_validate_and_match_schema() {
    let schema_json = schema::autocomplete_response_schema();
    let validator = jsonschema::validator_for(&schema_json).expect("response schema compiles");

    for name in [
        "autocomplete-response-ok.v1.json",
        "autocomplete-response-error.v1.json",
    ] {
        let json = fixture(name);
        let response: AutocompleteResponse =
            serde_json::from_value(json.clone()).expect("fixture deserializes");
        response.validate().expect("fixture is semantically valid");
        assert!(
            validator.is_valid(&json),
            "{name} validates against generated schema"
        );
    }
}

#[test]
fn error_code_wire_names_are_stable_snake_case() {
    let code = serde_json::to_value(ErrorCode::ProviderTimeout).expect("serializes");
    assert_eq!(code, Value::String("provider_timeout".to_owned()));

    let parsed: ErrorCode =
        serde_json::from_value(Value::String("unsupported_protocol_version".to_owned()))
            .expect("deserializes");
    assert_eq!(parsed, ErrorCode::UnsupportedProtocolVersion);
}
