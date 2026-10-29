mod support;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{Duration, TimeZone, Utc};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;
use wattlab_api::{
    auth::ApiToken,
    build_router,
    domain::{
        live::{LiveMeasurement, LiveSnapshot},
        session::Session,
    },
};
fn snapshot() -> LiveSnapshot {
    let now = Utc.timestamp_opt(1000, 0).unwrap();
    LiveSnapshot {
        session: Session::restore(Uuid::new_v4(), Uuid::new_v4(), now, None).unwrap(),
        captured_at: now,
        connection_state: Some("streaming".into()),
        ftms: Some(LiveMeasurement {
            received_at: now,
            speed_kmh: Some(25.0),
            cadence_rpm: Some(90.0),
            power_w: Some(100),
        }),
        cycling_power: Some(LiveMeasurement {
            received_at: now,
            speed_kmh: None,
            cadence_rpm: None,
            power_w: Some(110),
        }),
    }
}
#[test]
fn live_power_prefers_fresh_cycling_and_falls_back_to_fresh_ftms() {
    let mut s = snapshot();
    assert_eq!(s.metrics().power_w, Some(110));
    assert_eq!(s.metrics().power_source, Some("cycling_power"));
    s.cycling_power.as_mut().unwrap().received_at -= Duration::seconds(6);
    assert_eq!(s.metrics().power_w, Some(100));
    assert_eq!(s.metrics().power_source, Some("ftms"));
    s.ftms.as_mut().unwrap().received_at -= Duration::seconds(6);
    assert!(!s.metrics().fresh);
    assert_eq!(s.metrics().power_w, None);
    assert_eq!(s.metrics().speed_kmh, None);
}
#[test]
fn live_missing_is_not_zero_and_sources_have_independent_freshness() {
    let mut s = snapshot();
    let f = s.ftms.as_mut().unwrap();
    f.speed_kmh = Some(0.0);
    f.cadence_rpm = None;
    f.power_w = Some(0);
    s.cycling_power = None;
    assert_eq!(s.metrics().speed_kmh, Some(0.0));
    assert_eq!(s.metrics().cadence_rpm, None);
    assert_eq!(s.metrics().power_w, Some(0));
    s.cycling_power = snapshot().cycling_power;
    s.ftms.as_mut().unwrap().received_at -= Duration::seconds(6);
    assert_eq!(s.metrics().speed_kmh, None);
    assert_eq!(s.metrics().power_w, Some(110));
}
#[test]
fn stopped_reconnecting_or_finished_never_show_old_metrics() {
    for state in [
        None,
        Some("connecting"),
        Some("retry_wait"),
        Some("stopped"),
    ] {
        let mut s = snapshot();
        s.connection_state = state.map(str::to_owned);
        assert!(!s.metrics().fresh);
        assert_eq!(s.metrics().power_w, None);
    }
    let mut s = snapshot();
    s.session = Session::restore(
        s.session.id(),
        s.session.request_id(),
        s.session.started_at(),
        Some(s.captured_at),
    )
    .unwrap();
    assert!(!s.metrics().fresh);
}
#[test]
fn future_receipts_and_the_freshness_boundary_are_explicit() {
    let mut s = snapshot();
    s.cycling_power = None;
    s.ftms.as_mut().unwrap().received_at = s.captured_at + Duration::seconds(1);
    assert!(!s.metrics().fresh);
    s.ftms.as_mut().unwrap().received_at = s.captured_at - Duration::seconds(5);
    assert!(s.metrics().fresh);
    s.captured_at += Duration::milliseconds(1);
    assert!(!s.metrics().fresh);
}
#[tokio::test]
async fn web_assets_are_local_public_and_never_embed_api_credentials() {
    for (path, mime) in [
        ("/", "text/html"),
        ("/app.js", "text/javascript"),
        ("/styles.css", "text/css"),
    ] {
        let app = build_router(
            ApiToken::parse("test-only-token-for-integration-tests").unwrap(),
            support::unavailable_repository(),
        );
        let response = app
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            response.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with(mime)
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(response.headers().contains_key("content-security-policy"));
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(!text.contains("test-only-token-for-integration-tests"));
        assert!(!text.contains("https://cdn"));
    }
}
#[tokio::test]
async fn live_api_requires_bearer_and_empty_storage_is_an_explicit_empty_view() {
    let app = build_router(
        ApiToken::parse("test-only-token-for-integration-tests").unwrap(),
        Arc::new(support::StubRepository::default()),
    );
    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/live")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/live")
                .header(
                    "authorization",
                    "Bearer test-only-token-for-integration-tests",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(value["session"].is_null());
    assert!(value["power_w"].is_null());
    assert_eq!(value["fresh"], false);
}
