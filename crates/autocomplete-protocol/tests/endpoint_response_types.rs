use autocomplete_protocol::{
    CancelResponse, CancelStatus, HealthResponse, HealthStatus, PROTOCOL_VERSION, ProviderHealth,
    ProviderStatus, ReloadResponse, ReloadStatus,
};
use pretty_assertions::assert_eq;
use serde_json::json;
use uuid::Uuid;

#[test]
fn endpoint_response_types_use_stable_snake_case_wire_names() {
    let health = HealthResponse {
        protocol_version: PROTOCOL_VERSION,
        status: HealthStatus::Ok,
        provider: ProviderHealth {
            name: "mock".to_owned(),
            status: ProviderStatus::Available,
        },
    };
    assert_eq!(
        serde_json::to_value(health).expect("serializes"),
        json!({
            "protocol_version": 1,
            "status": "ok",
            "provider": {"name": "mock", "status": "available"}
        })
    );

    let request_id = Uuid::parse_str("018f160e-7152-7b43-9d9a-6083e0bd3cc8").unwrap();
    let cancel = CancelResponse {
        protocol_version: PROTOCOL_VERSION,
        request_id,
        status: CancelStatus::Cancelled,
    };
    assert_eq!(
        serde_json::to_value(cancel).expect("serializes"),
        json!({
            "protocol_version": 1,
            "request_id": "018f160e-7152-7b43-9d9a-6083e0bd3cc8",
            "status": "cancelled"
        })
    );

    let reload = ReloadResponse {
        protocol_version: PROTOCOL_VERSION,
        status: ReloadStatus::Reloaded,
    };
    assert_eq!(
        serde_json::to_value(reload).expect("serializes"),
        json!({"protocol_version": 1, "status": "reloaded"})
    );
}
