use autocomplete_protocol::{
    AutocompleteRequest, AutocompleteResponse, CancelResponse, CancelStatus, ErrorCode,
    HealthResponse, HealthStatus, ProviderStatus, ReloadResponse, ReloadStatus,
};
use autocomplete_server::{AppState, BrokerConfig, ProviderKind, app};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;
use std::fs;
use std::time::Duration;
use tower::ServiceExt;

fn fixture_request() -> AutocompleteRequest {
    serde_json::from_str(include_str!(
        "../../../examples/fixtures/autocomplete-request.v1.json"
    ))
    .expect("fixture request")
}

async fn read_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    serde_json::from_slice(&bytes).expect("json body")
}

#[tokio::test]
async fn health_reports_configured_mock_provider() {
    let mut config = BrokerConfig::default();
    config.provider.kind = ProviderKind::Mock;
    let app = app(AppState::from_config(config).expect("state"));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: HealthResponse = read_json(response).await;
    assert_eq!(body.status, HealthStatus::Ok);
    assert_eq!(body.provider.name, "mock");
}

#[tokio::test]
async fn autocomplete_uses_mock_provider_and_postprocessing_pipeline() {
    let mut config = BrokerConfig::default();
    config.provider.kind = ProviderKind::Mock;
    config.mock.insert_text = "greet(\"Ada\")".to_owned();
    let app = app(AppState::from_config(config).expect("state"));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/v1/autocomplete")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&fixture_request()).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body: AutocompleteResponse = read_json(response).await;
    let AutocompleteResponse::Ok {
        insert_text,
        source,
        metadata,
        ..
    } = body
    else {
        panic!("expected ok response");
    };
    assert_eq!(insert_text, "\"Ada\")");
    assert_eq!(source, "mock");
    assert!(metadata.expect("metadata").postprocessed);
}

#[tokio::test]
async fn autocomplete_rejects_semantically_invalid_requests() {
    let app = app(AppState::from_config(BrokerConfig::default()).expect("state"));
    let mut request = fixture_request();
    request.client.name.clear();

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/v1/autocomplete")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&request).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: AutocompleteResponse = read_json(response).await;
    let AutocompleteResponse::Error {
        request_id, error, ..
    } = body
    else {
        panic!("expected protocol error response");
    };
    assert_eq!(request_id, request.request_id);
    assert_eq!(error.code, ErrorCode::InvalidRequest);
}

#[tokio::test]
async fn cancel_endpoint_cancels_in_flight_autocomplete_and_marks_completion() {
    let mut config = BrokerConfig::default();
    config.mock.delay_ms = 200;
    config.mock.insert_text = "slow completion".to_owned();
    let app = app(AppState::from_config(config).expect("state"));
    let request = fixture_request();
    let request_id = request.request_id;

    let autocomplete_app = app.clone();
    let autocomplete_task = tokio::spawn(async move {
        autocomplete_app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/v1/autocomplete")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&request).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap()
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    let cancel_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/v1/cancel/{request_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let cancel_body: CancelResponse = read_json(cancel_response).await;
    assert_eq!(cancel_body.status, CancelStatus::Cancelled);

    let autocomplete_response = autocomplete_task.await.expect("autocomplete task");
    let autocomplete_body: AutocompleteResponse = read_json(autocomplete_response).await;
    assert!(matches!(
        autocomplete_body,
        AutocompleteResponse::Cancelled { .. }
    ));

    let completed_cancel = app
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri(format!("/v1/cancel/{request_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let completed_body: CancelResponse = read_json(completed_cancel).await;
    assert_eq!(completed_body.status, CancelStatus::AlreadyCompleted);
}

#[tokio::test]
async fn reload_reloads_config_and_changes_provider_behavior_without_client_changes() {
    let file = tempfile::NamedTempFile::new().expect("config file");
    fs::write(
        file.path(),
        r#"
bind_addr = "127.0.0.1:32145"

[provider]
kind = "mock"

[mock]
insert_text = "first completion"
"#,
    )
    .expect("write config");
    let app = app(AppState::from_config_path(file.path()).expect("state"));

    let first = post_autocomplete(app.clone(), fixture_request()).await;
    assert_ok_insert_text(first, "first completion").await;

    fs::write(
        file.path(),
        r#"
bind_addr = "127.0.0.1:32145"

[provider]
kind = "mock"

[mock]
insert_text = "second completion"
"#,
    )
    .expect("rewrite config");

    let reload_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/v1/reload")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reload_response.status(), StatusCode::OK);
    let reload_body: ReloadResponse = read_json(reload_response).await;
    assert_eq!(reload_body.status, ReloadStatus::Reloaded);

    let second = post_autocomplete(app, fixture_request()).await;
    assert_ok_insert_text(second, "second completion").await;
}

#[tokio::test]
async fn health_reflects_pi_provider_selection_from_config_without_calling_pi() {
    let mut config = BrokerConfig::default();
    config.provider.kind = ProviderKind::Pi;
    config.pi.model = "custom/model".to_owned();
    let app = app(AppState::from_config(config).expect("state"));

    let response = app
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let body: HealthResponse = read_json(response).await;
    assert_eq!(body.provider.name, "pi:custom/model");
    assert_eq!(body.provider.status, ProviderStatus::Unknown);
}

#[tokio::test]
async fn privacy_policy_rejects_excluded_paths_before_provider_runs() {
    let mut config = BrokerConfig::default();
    config.mock.insert_text = "would leak".to_owned();
    let app = app(AppState::from_config(config).expect("state"));
    let mut request = fixture_request();
    request.document.uri = "file:///repo/secrets/key.txt".to_owned();

    let response = post_autocomplete(app, request.clone()).await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body: AutocompleteResponse = read_json(response).await;
    let AutocompleteResponse::Error {
        request_id, error, ..
    } = body
    else {
        panic!("expected protocol error response");
    };
    assert_eq!(request_id, request.request_id);
    assert_eq!(error.code, ErrorCode::InvalidRequest);
}

#[tokio::test]
async fn privacy_policy_rejects_context_over_remote_byte_limit() {
    let mut config = BrokerConfig::default();
    config.privacy.remote_context_byte_limit = 4;
    let app = app(AppState::from_config(config).expect("state"));
    let mut request = fixture_request();
    request.context.prefix = "abcdef".to_owned();
    request.context.suffix.clear();

    let response = post_autocomplete(app, request).await;

    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let body: AutocompleteResponse = read_json(response).await;
    let AutocompleteResponse::Error { error, .. } = body else {
        panic!("expected protocol error response");
    };
    assert_eq!(error.code, ErrorCode::ContextTooLarge);
}

#[tokio::test]
async fn mock_provider_serves_obsidian_style_markdown_requests() {
    let mut config = BrokerConfig::default();
    config.mock.insert_text = " that deterministic broker tests are useful.".to_owned();
    let app = app(AppState::from_config(config).expect("state"));
    let mut request = fixture_request();
    request.client.name = "obsidian".to_owned();
    request.document.uri = "file:///vault/daily.md".to_owned();
    request.document.language_id = "markdown".to_owned();
    request.context.prefix = "Today I learned".to_owned();
    request.context.suffix.clear();

    let response = post_autocomplete(app, request).await;

    assert_ok_insert_text(response, " that deterministic broker tests are useful.").await;
}

async fn post_autocomplete(
    app: axum::Router,
    request: AutocompleteRequest,
) -> axum::response::Response {
    app.oneshot(
        Request::builder()
            .method(Method::POST)
            .uri("/v1/autocomplete")
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&request).unwrap()))
            .unwrap(),
    )
    .await
    .unwrap()
}

async fn assert_ok_insert_text(response: axum::response::Response, expected: &str) {
    assert_eq!(response.status(), StatusCode::OK);
    let body: AutocompleteResponse = read_json(response).await;
    let AutocompleteResponse::Ok { insert_text, .. } = body else {
        panic!("expected ok response");
    };
    assert_eq!(insert_text, expected);
}
