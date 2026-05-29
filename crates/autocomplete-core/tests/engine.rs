use autocomplete_core::{
    AutocompleteEngine, CompletionProvider, MockProvider, PostprocessorPipeline,
};
use autocomplete_protocol::{AutocompleteRequest, AutocompleteResponse, ErrorCode};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

fn fixture_request() -> AutocompleteRequest {
    serde_json::from_str(include_str!(
        "../../../examples/fixtures/autocomplete-request.v1.json"
    ))
    .expect("fixture request")
}

#[tokio::test]
async fn engine_returns_mock_completion_after_removing_repeated_prefix() {
    let request = fixture_request();
    let provider = MockProvider::new("greet(\"Ada\")");
    let engine = AutocompleteEngine::new(provider, PostprocessorPipeline::default());

    let response = engine.complete(request, CancellationToken::new()).await;

    let autocomplete_protocol::AutocompleteResponse::Ok {
        insert_text,
        source,
        metadata,
        ..
    } = response
    else {
        panic!("expected ok response, got {response:?}");
    };

    assert_eq!(insert_text, "\"Ada\")");
    assert_eq!(source, "mock");
    assert!(metadata.expect("metadata").postprocessed);
}

#[tokio::test]
async fn engine_can_use_provider_behind_dyn_trait_object() {
    let request = fixture_request();
    let provider: Arc<dyn CompletionProvider> = Arc::new(MockProvider::new("\"Ada\")"));
    let engine = AutocompleteEngine::from_provider_arc(provider, PostprocessorPipeline::default());

    let response = engine.complete(request, CancellationToken::new()).await;

    assert!(matches!(response, AutocompleteResponse::Ok { .. }));
}

#[tokio::test]
async fn mock_provider_returns_deterministic_completion_for_obsidian_style_requests() {
    let mut request = fixture_request();
    request.client.name = "obsidian".to_owned();
    request.document.uri = "file:///vault/notes/today.md".to_owned();
    request.document.language_id = "markdown".to_owned();
    request.context.prefix = "Today I learned".to_owned();
    request.context.suffix.clear();
    let engine = AutocompleteEngine::new(
        MockProvider::new(" that deterministic mocks are useful."),
        PostprocessorPipeline::default(),
    );

    let response = engine.complete(request, CancellationToken::new()).await;

    let AutocompleteResponse::Ok {
        insert_text,
        source,
        ..
    } = response
    else {
        panic!("expected ok response, got {response:?}");
    };
    assert_eq!(insert_text, " that deterministic mocks are useful.");
    assert_eq!(source, "mock");
}

#[tokio::test]
async fn engine_returns_provider_timeout_when_deadline_expires() {
    let mut request = fixture_request();
    request.options.deadline_ms = 10;
    let engine = AutocompleteEngine::new(
        MockProvider::new("\"Ada\")").with_delay(Duration::from_millis(200)),
        PostprocessorPipeline::default(),
    );

    let response = engine.complete(request, CancellationToken::new()).await;

    let AutocompleteResponse::Error { error, .. } = response else {
        panic!("expected timeout error, got {response:?}");
    };
    assert_eq!(error.code, ErrorCode::ProviderTimeout);
}

#[tokio::test]
async fn engine_returns_cancelled_when_token_is_cancelled_during_provider_call() {
    let request = fixture_request();
    let token = CancellationToken::new();
    let engine = AutocompleteEngine::new(
        MockProvider::new("\"Ada\")").with_delay(Duration::from_millis(200)),
        PostprocessorPipeline::default(),
    );

    let cancel = token.clone();
    let canceller = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        cancel.cancel();
    });

    let response = engine.complete(request, token).await;
    canceller.await.expect("canceller finished");

    assert!(matches!(response, AutocompleteResponse::Cancelled { .. }));
}
