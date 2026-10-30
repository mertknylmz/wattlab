mod support;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{TimeZone, Utc};
use http_body_util::BodyExt;
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;
use wattlab_api::{
    auth::ApiToken,
    build_router,
    domain::{
        history::{SeriesPoint, SessionRecord},
        session::Session,
    },
};

const TOKEN: &str = "test-only-token-for-integration-tests";

fn record(second: i64) -> SessionRecord {
    let start = Utc.timestamp_opt(second, 0).unwrap();
    SessionRecord {
        session: Session::restore(
            Uuid::new_v4(),
            Uuid::new_v4(),
            start,
            Some(start + chrono::Duration::seconds(60)),
        )
        .unwrap(),
        ftp_w: Some(250),
    }
}

fn stub(records: Vec<SessionRecord>) -> Arc<support::StubRepository> {
    let start = Utc.timestamp_opt(1000, 0).unwrap();
    Arc::new(support::StubRepository {
        records,
        points: vec![
            SeriesPoint {
                second: start,
                ftms_power_w: Some(100),
                cycling_power_w: Some(120),
                cadence_rpm: Some(85.0),
                speed_kmh: Some(30.0),
            },
            SeriesPoint {
                second: start + chrono::Duration::seconds(1),
                ftms_power_w: Some(0),
                cycling_power_w: None,
                cadence_rpm: None,
                speed_kmh: None,
            },
        ],
        ..Default::default()
    })
}

async fn get(repository: Arc<support::StubRepository>, path: &str) -> (StatusCode, Value) {
    let app = build_router(ApiToken::parse(TOKEN).unwrap(), repository);
    let token = support::member_token(&app).await;
    let response = app
        .oneshot(
            Request::builder()
                .uri(path)
                .header("Authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn list_summarizes_each_session_and_offers_cursor_only_for_full_pages() {
    let records = vec![record(2000), record(1000)];
    let last = records[1].session.id();
    let (status, body) = get(stub(records.clone()), "/api/v1/sessions?limit=2").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["next_before"], last.to_string());
    let first = &body["sessions"][0];
    assert_eq!(first["ftp_w"], 250);
    assert_eq!(first["summary"]["duration_seconds"], 60);
    assert_eq!(first["summary"]["average_power_w"], 60.0);
    assert_eq!(first["summary"]["max_power_w"], 120);
    let (_, body) = get(stub(records), "/api/v1/sessions").await;
    assert!(body["next_before"].is_null());
}

#[tokio::test]
async fn series_reports_selected_source_and_keeps_missing_values_null() {
    let records = vec![record(1000)];
    let id = records[0].session.id();
    let (status, body) = get(stub(records), &format!("/api/v1/sessions/{id}/series")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["session"]["id"], id.to_string());
    let points = body["points"].as_array().unwrap();
    assert_eq!(points[0]["power_w"], 120);
    assert_eq!(points[0]["power_source"], "cycling_power");
    assert_eq!(points[1]["power_w"], 0);
    assert_eq!(points[1]["power_source"], "ftms");
    assert!(points[1]["cadence_rpm"].is_null());
}

#[tokio::test]
async fn invalid_queries_unknown_sessions_and_storage_failures_are_mapped() {
    for path in [
        "/api/v1/sessions?limit=0",
        "/api/v1/sessions?limit=101",
        "/api/v1/sessions?limit=abc",
        "/api/v1/sessions?before=nope",
        "/api/v1/sessions/nope/summary",
        "/api/v1/activity",
        "/api/v1/activity?from=nope&to=2026-01-01T00:00:00Z",
        "/api/v1/activity?from=2026-01-01T00:00:00Z&to=2026-01-01T00:00:00Z",
    ] {
        assert_eq!(
            get(stub(vec![]), path).await.0,
            StatusCode::BAD_REQUEST,
            "{path}"
        );
    }
    let missing = format!("/api/v1/sessions/{}/summary", Uuid::new_v4());
    assert_eq!(get(stub(vec![]), &missing).await.0, StatusCode::NOT_FOUND);
    assert_eq!(
        get(support::unavailable_repository(), "/api/v1/sessions")
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn activity_lists_rides_in_the_requested_window() {
    let records = vec![record(1_700_000_000), record(1_800_000_000)];
    let (status, body) = get(
        stub(records),
        "/api/v1/activity?from=2023-01-01T00:00:00Z&to=2024-01-01T00:00:00Z",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let entries = body["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["duration_seconds"], 60);
}

async fn put_settings(body: &str) -> (StatusCode, Value) {
    let app = build_router(ApiToken::parse(TOKEN).unwrap(), stub(vec![]));
    let token = support::member_token(&app).await;
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/settings")
                .header("Authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn settings_validate_ftp_and_null_restores_the_default() {
    let (status, body) = get(stub(vec![]), "/api/v1/settings").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (body["ftp_w"].clone(), body["ftp_is_default"].clone()),
        (200.into(), true.into())
    );
    let (status, body) = put_settings(r#"{"ftp_w":275}"#).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["ftp_w"], 275);
    assert_eq!(body["ftp_is_default"], false);
    assert_eq!(
        put_settings(r#"{"ftp_w":null}"#).await.1["ftp_is_default"],
        true
    );
    for invalid in [
        r#"{}"#,
        r#"{"ftp_w":0}"#,
        r#"{"ftp_w":2001}"#,
        r#"{"ftp_w":12.5}"#,
        r#"{"ftp_w":"250"}"#,
    ] {
        assert_eq!(
            put_settings(invalid).await.0,
            StatusCode::BAD_REQUEST,
            "{invalid}"
        );
    }
}

#[tokio::test]
async fn sessions_without_captured_ftp_use_the_current_setting_for_zones() {
    let mut legacy = record(1000);
    legacy.ftp_w = None;
    let id = legacy.session.id();
    let (_, body) = get(
        stub(vec![legacy]),
        &format!("/api/v1/sessions/{id}/summary"),
    )
    .await;
    assert_eq!(body["zone_reference"], "current");
    assert_eq!(body["summary"]["zone_ftp_w"], 200);
    assert_eq!(
        body["summary"]["zone_seconds"],
        serde_json::json!([1, 0, 1, 0, 0, 0, 0])
    );
    let captured = record(1000);
    let id = captured.session.id();
    let (_, body) = get(
        stub(vec![captured]),
        &format!("/api/v1/sessions/{id}/summary"),
    )
    .await;
    assert_eq!(body["zone_reference"], "session");
    assert_eq!(body["summary"]["zone_ftp_w"], 250);
}
