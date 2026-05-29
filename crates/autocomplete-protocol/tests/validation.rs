use autocomplete_protocol::{
    AutocompleteRequest, AutocompleteResponse, Validate, ValidationLimits,
};

fn valid_request() -> AutocompleteRequest {
    let path = format!(
        "{}/../../examples/fixtures/autocomplete-request.v1.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(path).expect("fixture is readable");
    serde_json::from_str(&text).expect("fixture deserializes")
}

fn valid_ok_response() -> AutocompleteResponse {
    let path = format!(
        "{}/../../examples/fixtures/autocomplete-response-ok.v1.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(path).expect("fixture is readable");
    serde_json::from_str(&text).expect("fixture deserializes")
}

#[test]
fn request_validation_reports_field_paths_for_semantic_errors() {
    let mut request = valid_request();
    request.protocol_version = 2;
    request.client.name.clear();
    request.options.max_chars = 0;
    request.options.deadline_ms = 30_001;

    let errors = request.validate().expect_err("request should be invalid");
    let fields: Vec<_> = errors.iter().map(|error| error.field.as_str()).collect();

    assert_eq!(
        fields,
        [
            "protocol_version",
            "client.name",
            "options.max_chars",
            "options.deadline_ms"
        ]
    );
}

#[test]
fn custom_validation_limits_reject_over_budget_context() {
    let request = valid_request();
    let limits = ValidationLimits {
        max_context_bytes: 4,
        ..ValidationLimits::default()
    };

    let errors = request
        .validate_with_limits(limits)
        .expect_err("context should be over budget");
    assert!(errors.iter().any(|error| error.field == "context"));
}

#[test]
fn ok_response_requires_insert_text_and_confidence_in_range() {
    let mut response = valid_ok_response();
    let AutocompleteResponse::Ok {
        insert_text,
        confidence,
        ..
    } = &mut response
    else {
        panic!("fixture is ok response");
    };
    insert_text.clear();
    *confidence = 1.1;

    let errors = response.validate().expect_err("response should be invalid");
    let fields: Vec<_> = errors.iter().map(|error| error.field.as_str()).collect();
    assert_eq!(fields, ["insert_text", "confidence"]);
}
