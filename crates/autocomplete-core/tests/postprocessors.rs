use autocomplete_core::PostprocessorPipeline;
use autocomplete_protocol::AutocompleteRequest;

fn fixture_request() -> AutocompleteRequest {
    serde_json::from_str(include_str!(
        "../../../examples/fixtures/autocomplete-request.v1.json"
    ))
    .expect("fixture request")
}

#[test]
fn postprocessors_strip_markdown_fences() {
    let request = fixture_request();
    let text = PostprocessorPipeline::default()
        .process(&request, "```typescript\n\"Ada\")\n```")
        .expect("postprocessed text");

    assert_eq!(text, "\"Ada\")");
}

#[test]
fn postprocessors_normalize_newlines_before_returning_insertable_text() {
    let request = fixture_request();
    let text = PostprocessorPipeline::default()
        .process(&request, "\"Ada\")\r\n  .toString()\r")
        .expect("postprocessed text");

    assert_eq!(text, "\"Ada\")\n  .toString()\n");
}

#[test]
fn postprocessors_truncate_overlong_output_to_request_max_chars() {
    let mut request = fixture_request();
    request.options.max_chars = 5;

    let text = PostprocessorPipeline::default()
        .process(&request, "\"Ada\") and more")
        .expect("postprocessed text");

    assert_eq!(text, "\"Ada\"");
}

#[test]
fn postprocessors_reject_explanations_instead_of_returning_prose() {
    let request = fixture_request();
    let error = PostprocessorPipeline::default()
        .process(&request, "Here is the completion: \"Ada\")")
        .expect_err("explanatory output should be rejected");

    assert!(error.to_string().contains("prose"));
}
