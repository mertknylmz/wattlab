mod support;

use axum::{
    body::Body,
    http::{
        Request, StatusCode,
        header::{AUTHORIZATION, WWW_AUTHENTICATE},
    },
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use wattlab_api::{auth::ApiToken, build_router};

const TOKEN: &str = "test-only-token-for-integration-tests";

fn app() -> axum::Router {
    build_router(
        ApiToken::parse(TOKEN).unwrap(),
        support::unavailable_repository(),
    )
}

async fn body(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn health_is_public_process_liveness_without_database_or_trainer() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "application/json");
    assert_eq!(
        body(response).await,
        json!({"status": "ok", "service": "wattlab-api"})
    );
}

#[tokio::test]
async fn protected_status_requires_authentication() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/v1/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[WWW_AUTHENTICATE], "Bearer");
    assert_eq!(body(response).await, json!({"error": "unauthorized"}));
}

#[tokio::test]
async fn exact_bearer_token_grants_access() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/v1/status")
                .header(AUTHORIZATION, format!("Bearer {TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        body(response).await,
        json!({"status": "ok", "service": "wattlab-api"})
    );
}

#[tokio::test]
async fn wrong_token_scheme_and_partial_matches_are_rejected_without_echoing_credentials() {
    for authorization in [
        format!("Basic {TOKEN}"),
        "Bearer wrong".into(),
        "Bearer ".into(),
        format!("Bearer {}", &TOKEN[..TOKEN.len() - 1]),
        format!("Bearer {TOKEN}x"),
    ] {
        let response = app()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/status")
                    .header(AUTHORIZATION, &authorization)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(body(response).await, json!({"error": "unauthorized"}));
    }
}

#[tokio::test]
async fn token_in_query_or_cookie_does_not_grant_access() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/status?token={TOKEN}"))
                .header("cookie", format!("token={TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn duplicate_authorization_headers_are_rejected() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/v1/status")
                .header(AUTHORIZATION, format!("Bearer {TOKEN}"))
                .header(AUTHORIZATION, "Bearer other")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn api_fallback_is_protected_before_body_parsing() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("content-type", "application/json")
                .body(Body::from("invalid json"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn telemetry_route_rejects_invalid_path_and_payload() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions/00000000-0000-0000-0000-000000000001/telemetry")
                .header(AUTHORIZATION, format!("Bearer {TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(body(response).await, json!({"error": "invalid_request"}));
}

#[tokio::test]
async fn invalid_session_payloads_are_400_not_framework_default_422() {
    for payload in [
        "invalid json",
        "{}",
        "null",
        "{\"request_id\":null}",
        "{\"request_id\":\"invalid\"}",
    ] {
        let response = app()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header(AUTHORIZATION, format!("Bearer {TOKEN}"))
                    .header("content-type", "application/json")
                    .body(Body::from(payload))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body(response).await, json!({"error":"invalid_request"}));
    }
}

#[tokio::test]
async fn invalid_session_paths_are_rejected_before_storage() {
    for path in [
        "/api/v1/sessions/not-a-uuid",
        "/api/v1/sessions/not-a-uuid/finish",
    ] {
        let response = app()
            .oneshot(
                Request::builder()
                    .method(if path.ends_with("finish") {
                        "POST"
                    } else {
                        "GET"
                    })
                    .uri(path)
                    .header(AUTHORIZATION, format!("Bearer {TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
async fn storage_failure_is_sanitized_and_retryable() {
    let response = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header(AUTHORIZATION, format!("Bearer {TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&json!({"request_id":uuid::Uuid::new_v4()})).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body(response).await, json!({"error":"storage_unavailable"}));
}
