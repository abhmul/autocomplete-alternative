use autocomplete_protocol::{AutocompleteRequest, Validate, schema};
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
fn autocomplete_request_fixture_round_trips_validates_and_matches_schema() {
    let json = fixture("autocomplete-request.v1.json");

    let request: AutocompleteRequest =
        serde_json::from_value(json.clone()).expect("fixture deserializes");
    request.validate().expect("fixture is semantically valid");

    let schema_json = schema::autocomplete_request_schema();
    let validator = jsonschema::validator_for(&schema_json).expect("request schema compiles");
    assert!(
        validator.is_valid(&json),
        "fixture validates against generated schema"
    );

    let mut invalid = json;
    invalid
        .as_object_mut()
        .expect("fixture is an object")
        .remove("request_id");
    assert!(
        !validator.is_valid(&invalid),
        "generated schema rejects missing required request_id"
    );
}
