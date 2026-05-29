use autocomplete_protocol::{AutocompleteRequest, AutocompleteResponse, Validate, schema};
use serde_json::Value;

fn plan_protocol_example(name: &str) -> Value {
    let path = format!(
        "{}/../../reports/autocomplete-engine-mvp-plan-2026-05-28.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(path).expect("MVP plan is readable");
    let plan: Value = serde_json::from_str(&text).expect("MVP plan is valid JSON");
    plan.pointer(&format!("/protocol/{name}"))
        .unwrap_or_else(|| panic!("MVP plan has protocol.{name}"))
        .clone()
}

fn assert_matches_schema(schema_json: &Value, instance: &Value, label: &str) {
    let validator = jsonschema::validator_for(schema_json).expect("generated schema compiles");
    assert!(
        validator.is_valid(instance),
        "{label} validates against generated JSON Schema"
    );
}

#[test]
fn mvp_plan_autocomplete_examples_validate_against_public_protocol_contract() {
    let request_json = plan_protocol_example("autocomplete_request_example");
    let request_schema = schema::autocomplete_request_schema();
    let request_label = "protocol.autocomplete_request_example";
    assert_matches_schema(&request_schema, &request_json, request_label);
    let request: AutocompleteRequest =
        serde_json::from_value(request_json).expect("plan request example deserializes");
    request
        .validate()
        .expect("plan request example is semantically valid");
    println!("{request_label}: schema_valid=true deserialize=ok semantic_valid=true");

    let response_schema = schema::autocomplete_response_schema();
    for name in ["autocomplete_response_example", "error_response_example"] {
        let response_json = plan_protocol_example(name);
        let response_label = format!("protocol.{name}");
        assert_matches_schema(&response_schema, &response_json, &response_label);
        let response: AutocompleteResponse =
            serde_json::from_value(response_json).expect("plan response example deserializes");
        response
            .validate()
            .expect("plan response example is semantically valid");
        println!("{response_label}: schema_valid=true deserialize=ok semantic_valid=true");
    }
}
