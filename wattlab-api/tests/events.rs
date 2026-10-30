mod support;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use chrono::{TimeZone, Utc};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;
use wattlab_api::{
    application::{
        error::ApplicationError, ports::repository_error::RepositoryError,
        session_service::SessionService,
    },
    auth::ApiToken,
    build_router,
    domain::{
        events::{Envelope, EventBatch, Measurement},
        session::Session,
        source::Source,
    },
};
const TOKEN: &str = "test-only-token-for-integration-tests";
fn event() -> Value {
    json!({"schema_version":1,"stream_id":Uuid::new_v4(),"connection_id":Uuid::new_v4(),"sequence":0,"observed_at":"2026-10-29T09:00:00.123456789Z","source":"ftms","speed_kmh":25.5,"cadence_rpm":null,"power_w":-20})
}
fn envelope() -> Envelope {
    Envelope::new(
        1,
        Uuid::new_v4(),
        Uuid::new_v4(),
        0,
        Utc.timestamp_opt(1000, 123456789).unwrap(),
    )
    .unwrap()
}
async fn call(
    kind: &str,
    payload: Value,
    failure: Option<RepositoryError>,
    authenticated: bool,
) -> (StatusCode, Value) {
    let app = build_router(
        ApiToken::parse(TOKEN).unwrap(),
        Arc::new(support::StubRepository {
            session: None,
            failure,
            ..Default::default()
        }),
    );
    let mut request = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/sessions/{}/{kind}", Uuid::new_v4()))
        .header("content-type", "application/json");
    if authenticated {
        request = request.header("authorization", format!("Bearer {TOKEN}"));
    }
    let response = app
        .oneshot(request.body(Body::from(payload.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap())
}
#[test]
fn measurements_preserve_null_zero_signed_power_and_enforce_ranges() {
    assert!(Measurement::new(envelope(), Source::Ftms, Some(0.0), None, Some(-32768)).is_ok());
    for (speed, cadence, power, source) in [
        (None, None, None, Source::Ftms),
        (Some(f64::NAN), None, None, Source::Ftms),
        (None, Some(f64::INFINITY), None, Source::Ftms),
        (Some(-0.1), None, None, Source::Ftms),
        (Some(655.36), None, None, Source::Ftms),
        (None, Some(32767.6), None, Source::Ftms),
        (None, None, Some(32768), Source::Ftms),
        (None, None, Some(-32769), Source::Ftms),
        (Some(0.0), None, Some(1), Source::CyclingPower),
    ] {
        assert!(Measurement::new(envelope(), source, speed, cadence, power).is_err());
    }
    assert!(
        Measurement::new(
            envelope(),
            Source::Ftms,
            Some(655.35),
            Some(32767.5),
            Some(32767)
        )
        .is_ok()
    );
}
#[test]
fn envelopes_batches_and_post_finish_policy_are_domain_rules() {
    let e = envelope();
    assert_eq!(e.observed_at().timestamp_subsec_nanos(), 123456000);
    assert!(Envelope::new(2, e.stream_id(), e.connection_id(), 0, e.observed_at()).is_err());
    assert!(Envelope::new(1, e.stream_id(), e.connection_id(), -1, e.observed_at()).is_err());
    assert!(EventBatch::<u8>::new(vec![]).is_err());
    assert!(EventBatch::new(vec![0; 101]).is_err());
    assert!(EventBatch::new(vec![0; 100]).is_ok());
    for finished in [None, Some(e.observed_at())] {
        let session =
            Session::restore(Uuid::new_v4(), Uuid::new_v4(), e.observed_at(), finished).unwrap();
        assert_eq!(session.accept_event(Some(true)), Ok(false));
        assert!(session.accept_event(Some(false)).is_err());
        assert_eq!(session.accept_event(None).is_ok(), finished.is_none());
    }
}
#[tokio::test]
async fn http_validation_happens_before_the_storage_port() {
    let original = event();
    for (field, value) in [
        ("schema_version", json!(0)),
        ("schema_version", json!(2)),
        ("schema_version", json!(null)),
        ("sequence", json!(-1)),
        ("sequence", json!(9223372036854775808_u64)),
        ("stream_id", json!("invalid")),
        ("connection_id", json!(null)),
        ("observed_at", json!("yesterday")),
        ("observed_at", json!("2026-10-29T09:00:00")),
        ("source", json!("unknown")),
        ("speed_kmh", json!(-1)),
        ("speed_kmh", json!(655.36)),
        ("cadence_rpm", json!(32767.6)),
        ("power_w", json!(32768)),
        ("power_w", json!(1.5)),
    ] {
        let mut e = original.clone();
        e[field] = value;
        assert_eq!(
            call(
                "telemetry",
                json!({"events":[e]}),
                Some(RepositoryError::Unavailable),
                true
            )
            .await
            .0,
            StatusCode::BAD_REQUEST,
            "field {field}"
        );
    }
    for events in [
        json!([]),
        json!([null]),
        json!(vec![original.clone(); 101]),
        json!([{"source":"ftms"}]),
    ] {
        assert_eq!(
            call("telemetry", json!({"events":events}), None, true)
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    let mut empty = original.clone();
    empty["power_w"] = Value::Null;
    empty["speed_kmh"] = Value::Null;
    assert_eq!(
        call("telemetry", json!({"events":[empty]}), None, true)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let mut cycling = original.clone();
    cycling["source"] = json!("cycling_power");
    assert_eq!(
        call("telemetry", json!({"events":[cycling]}), None, true)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}
#[tokio::test]
async fn event_http_contract_auth_acceptance_and_error_mapping() {
    for kind in ["telemetry", "connection-events"] {
        let mut e = event();
        if kind == "connection-events" {
            e.as_object_mut().unwrap().remove("source");
            e["state"] = json!("streaming");
        }
        let payload = json!({"events":[e.clone(),e]});
        assert_eq!(
            call(kind, payload.clone(), None, false).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            call(kind, payload.clone(), None, true).await,
            (StatusCode::OK, json!({"accepted":2}))
        );
        for (failure, status, error) in [
            (
                RepositoryError::NotFound,
                StatusCode::NOT_FOUND,
                "not_found",
            ),
            (RepositoryError::Conflict, StatusCode::CONFLICT, "conflict"),
            (
                RepositoryError::Unavailable,
                StatusCode::SERVICE_UNAVAILABLE,
                "storage_unavailable",
            ),
            (
                RepositoryError::Unexpected,
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
            ),
        ] {
            assert_eq!(
                call(kind, payload.clone(), Some(failure), true).await,
                (status, json!({"error":error}))
            );
        }
    }
    for state in [
        "connecting",
        "subscribing",
        "streaming",
        "retry_wait",
        "stopped",
    ] {
        let mut e = event();
        e["state"] = json!(state);
        assert_eq!(
            call("connection-events", json!({"events":[e]}), None, true)
                .await
                .0,
            StatusCode::OK
        );
    }
    let mut e = event();
    e["state"] = json!("connected");
    assert_eq!(
        call("connection-events", json!({"events":[e]}), None, true)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
}
#[tokio::test]
async fn application_passes_validated_batches_and_maps_port_conflicts() {
    let unavailable = SessionService::new(support::unavailable_repository());
    let sample = Measurement::new(envelope(), Source::CyclingPower, None, None, Some(1)).unwrap();
    assert_eq!(
        unavailable
            .telemetry(Uuid::new_v4(), EventBatch::new(vec![sample]).unwrap())
            .await,
        Err(ApplicationError::StorageUnavailable)
    );
    let measurement =
        Measurement::new(envelope(), Source::CyclingPower, None, None, Some(-10)).unwrap();
    for (failure, expected) in [
        (None, Ok(1)),
        (
            Some(RepositoryError::NotFound),
            Err(ApplicationError::SessionNotFound),
        ),
        (
            Some(RepositoryError::Conflict),
            Err(ApplicationError::EventConflict),
        ),
    ] {
        let service = SessionService::new(Arc::new(support::StubRepository {
            session: None,
            failure,
            ..Default::default()
        }));
        assert_eq!(
            service
                .telemetry(
                    Uuid::new_v4(),
                    EventBatch::new(vec![measurement.clone()]).unwrap()
                )
                .await,
            expected
        );
    }
}
