//! Opt-in tests use only WATTLAB_TEST_DB_* and never reset a database.
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::sync::Arc;
use tower::ServiceExt;
use uuid::Uuid;
use wattlab_api::{
    adapters::outbound::postgres::{DatabaseConfig, PgSessionRepository, connect_and_migrate},
    auth::ApiToken,
    build_router,
};

const TOKEN: &str = "test-only-token-for-integration-tests";

async fn setup() -> (Router, PgPool, String) {
    let config = DatabaseConfig::load_test()
        .expect("Configure the isolated Rust test database using scripts/local_db.py start");
    let pool = connect_and_migrate(&config)
        .await
        .expect("Rust test database must be available and migrations must pass");
    // One physical trainer: leftover unfinished rows from a crashed test block others.
    sqlx::query(
        "UPDATE training_session SET finished_at = clock_timestamp() WHERE finished_at IS NULL",
    )
    .execute(&pool)
    .await
    .expect("must be able to release leftover active sessions");
    let app = build_router(
        ApiToken::parse(TOKEN).unwrap(),
        Arc::new(PgSessionRepository::new(pool.clone())),
    );
    let member = register_member(&app).await;
    (app, pool, member)
}

async fn register_member(app: &Router) -> String {
    let username = format!("m{}", &Uuid::new_v4().simple().to_string()[..12]);
    let (status, body) = as_user(
        app.clone(),
        None,
        "POST",
        "/api/v1/auth/register",
        Some(json!({"username": username, "password": "correct horse"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    body["token"].as_str().unwrap().to_owned()
}

async fn as_user(
    app: Router,
    token: Option<&str>,
    method: &str,
    path: &str,
    payload: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let response = app
        .oneshot(
            request
                .body(match payload {
                    Some(value) => Body::from(serde_json::to_vec(&value).unwrap()),
                    None => Body::empty(),
                })
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

async fn call(
    app: Router,
    method: &str,
    path: &str,
    payload: Option<Value>,
) -> (StatusCode, Value) {
    as_user(app, Some(TOKEN), method, path, payload).await
}

async fn start(app: Router, member: &str, request_id: Uuid) -> Value {
    let (status, command) = as_user(
        app.clone(),
        Some(member),
        "POST",
        "/api/v1/agent/commands",
        Some(json!({"kind": "start_session"})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{command}");
    let (status, delivered) = call(
        app.clone(),
        "GET",
        "/api/v1/agent/commands/next?wait_seconds=0",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(delivered["id"], command["id"]);
    let (status, session) = call(
        app,
        "POST",
        "/api/v1/sessions",
        Some(json!({"request_id": request_id, "command_id": command["id"]})),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{session}");
    session
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL; run scripts/run_api.py --integration"]
async fn start_and_get_persist_one_session_per_request_id() {
    let (app, pool, member) = setup().await;
    let request = Uuid::new_v4();
    let first = start(app.clone(), &member, request).await;
    let second = start(app.clone(), &member, request).await;
    assert_eq!(first, second);
    assert_eq!(first["request_id"], request.to_string());
    assert!(first["finished_at"].is_null());
    assert!(chrono::DateTime::parse_from_rfc3339(first["started_at"].as_str().unwrap()).is_ok());
    let (status, fetched) = call(
        app,
        "GET",
        &format!("/api/v1/sessions/{}", first["id"].as_str().unwrap()),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched, first);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM training_session WHERE request_id = $1")
            .bind(request)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL; run scripts/run_api.py --integration"]
async fn finish_is_idempotent_and_start_retry_never_reopens_session() {
    let (app, pool, member) = setup().await;
    let request = Uuid::new_v4();
    let session = start(app.clone(), &member, request).await;
    let path = format!(
        "/api/v1/sessions/{}/finish",
        session["id"].as_str().unwrap()
    );
    let (status, first) = call(app.clone(), "POST", &path, None).await;
    let (second_status, second) = call(app.clone(), "POST", &path, None).await;
    assert_eq!((status, second_status), (StatusCode::OK, StatusCode::OK));
    assert_eq!(first, second);
    assert!(!first["finished_at"].is_null());
    assert_eq!(first["started_at"], session["started_at"]);
    assert_eq!(start(app, &member, request).await, first);
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL; run scripts/run_api.py --integration"]
async fn nonexistent_sessions_return_404_for_get_and_finish() {
    let (app, pool, _member) = setup().await;
    let path = format!("/api/v1/sessions/{}", Uuid::new_v4());
    for (method, path) in [("GET", path.clone()), ("POST", format!("{path}/finish"))] {
        let (status, response) = call(app.clone(), method, &path, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(response, json!({"error":"not_found"}));
    }
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL; run scripts/run_api.py --integration"]
async fn concurrent_start_retries_share_identity_and_start_time() {
    let (app, pool, member) = setup().await;
    let request = Uuid::new_v4();
    let (first, second) = tokio::join!(
        start(app.clone(), &member, request),
        start(app, &member, request)
    );
    assert_eq!(first, second);
    let count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM training_session WHERE request_id = $1")
            .bind(request)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(count, 1);
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL; run scripts/run_api.py --integration"]
async fn concurrent_finish_retries_preserve_the_first_finish_time() {
    let (app, pool, member) = setup().await;
    let session = start(app.clone(), &member, Uuid::new_v4()).await;
    let path = format!(
        "/api/v1/sessions/{}/finish",
        session["id"].as_str().unwrap()
    );
    let (first, second) = tokio::join!(
        call(app.clone(), "POST", &path, None),
        call(app, "POST", &path, None)
    );
    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(first, second);
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL; run scripts/run_api.py --integration"]
async fn racing_start_retry_and_finish_never_reset_lifecycle() {
    let (app, pool, member) = setup().await;
    let request = Uuid::new_v4();
    let session = start(app.clone(), &member, request).await;
    let path = format!(
        "/api/v1/sessions/{}/finish",
        session["id"].as_str().unwrap()
    );
    let (retry, finish) = tokio::join!(
        start(app.clone(), &member, request),
        call(app.clone(), "POST", &path, None)
    );
    assert_eq!(retry["id"], session["id"]);
    assert_eq!(finish.0, StatusCode::OK);
    assert_eq!(start(app, &member, request).await, finish.1);
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL; run scripts/run_api.py --integration"]
async fn migration_rejects_finish_before_start_without_mutating_the_row() {
    let (app, pool, member) = setup().await;
    let session = start(app, &member, Uuid::new_v4()).await;
    let id = Uuid::parse_str(session["id"].as_str().unwrap()).unwrap();
    let error = sqlx::query(
        "UPDATE training_session SET finished_at = started_at - INTERVAL '1 second' WHERE id = $1",
    )
    .bind(id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().code().as_deref(),
        Some("23514")
    );
    let finished: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("SELECT finished_at FROM training_session WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(finished.is_none());
    pool.close().await;
}

fn wire_event(kind: &str) -> Value {
    let mut event = json!({"schema_version":1,"stream_id":Uuid::new_v4(),"connection_id":Uuid::new_v4(),"sequence":0,"observed_at":"2026-10-29T09:00:00.123456789Z"});
    if kind == "telemetry" {
        event["source"] = json!("ftms");
        event["speed_kmh"] = json!(0.0);
        event["cadence_rpm"] = Value::Null;
        event["power_w"] = json!(-123);
    } else {
        event["state"] = json!("streaming");
    }
    event
}
async fn submit(app: Router, id: &str, kind: &str, events: Vec<Value>) -> (StatusCode, Value) {
    call(
        app,
        "POST",
        &format!("/api/v1/sessions/{id}/{kind}"),
        Some(json!({"events":events})),
    )
    .await
}
async fn event_count(pool: &PgPool, id: &str, kind: &str) -> i64 {
    let sql = if kind == "telemetry" {
        "SELECT count(*) FROM telemetry_event WHERE session_id=$1"
    } else {
        "SELECT count(*) FROM connection_event WHERE session_id=$1"
    };
    sqlx::query_scalar(sql)
        .bind(Uuid::parse_str(id).unwrap())
        .fetch_one(pool)
        .await
        .unwrap()
}
#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn events_persist_measurement_null_zero_signed_values_and_metadata() {
    let (app, pool, member) = setup().await;
    let session = start(app.clone(), &member, Uuid::new_v4()).await;
    let id = session["id"].as_str().unwrap();
    for kind in ["telemetry", "connection-events"] {
        let event = wire_event(kind);
        assert_eq!(
            submit(app.clone(), id, kind, vec![event.clone()]).await,
            (StatusCode::OK, json!({"accepted":1}))
        );
        assert_eq!(event_count(&pool, id, kind).await, 1);
    }
    let (speed, cadence, power, observed): (
        Option<f64>,
        Option<f64>,
        Option<i32>,
        chrono::DateTime<chrono::Utc>,
    ) = sqlx::query_as(
        "SELECT speed_kmh,cadence_rpm,power_w,observed_at FROM telemetry_event WHERE session_id=$1",
    )
    .bind(Uuid::parse_str(id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!((speed, cadence, power), (Some(0.0), None, Some(-123)));
    assert_eq!(observed.timestamp_subsec_nanos(), 123456000);
    // Boundaries: exactly 100 events are accepted atomically.
    let mut events = Vec::new();
    for sequence in 1..=100 {
        let mut e = wire_event("telemetry");
        e["sequence"] = json!(sequence);
        events.push(e);
    }
    assert_eq!(
        submit(app, id, "telemetry", events).await,
        (StatusCode::OK, json!({"accepted":100}))
    );
    assert_eq!(event_count(&pool, id, "telemetry").await, 101);
    pool.close().await;
}
#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn concurrent_and_within_batch_replays_acknowledge_without_duplicate_rows() {
    let (app, pool, member) = setup().await;
    let session = start(app.clone(), &member, Uuid::new_v4()).await;
    let id = session["id"].as_str().unwrap();
    for kind in ["telemetry", "connection-events"] {
        let event = wire_event(kind);
        let (a, b) = tokio::join!(
            submit(app.clone(), id, kind, vec![event.clone(), event.clone()]),
            submit(app.clone(), id, kind, vec![event])
        );
        assert_eq!(a, (StatusCode::OK, json!({"accepted":2})));
        assert_eq!(b, (StatusCode::OK, json!({"accepted":1})));
        assert_eq!(event_count(&pool, id, kind).await, 1);
    }
    pool.close().await;
}
#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn changed_event_contents_conflict_and_rollback_the_entire_batch() {
    let (app, pool, member) = setup().await;
    let session = start(app.clone(), &member, Uuid::new_v4()).await;
    let id = session["id"].as_str().unwrap();
    for kind in ["telemetry", "connection-events"] {
        let event = wire_event(kind);
        assert_eq!(
            submit(app.clone(), id, kind, vec![event.clone()]).await.0,
            StatusCode::OK
        );
        let mut changes = vec![
            ("connection_id", json!(Uuid::new_v4())),
            ("observed_at", json!("2026-10-29T09:00:01Z")),
        ];
        if kind == "telemetry" {
            changes.extend([
                ("source", json!("cycling_power")),
                ("speed_kmh", Value::Null),
                ("cadence_rpm", json!(0)),
                ("power_w", json!(123)),
            ]);
        } else {
            changes.push(("state", json!("retry_wait")));
        }
        for (field, value) in changes {
            let mut altered = event.clone();
            altered[field] = value;
            if field == "source" {
                altered["speed_kmh"] = Value::Null;
            }
            let new = wire_event(kind);
            assert_eq!(
                submit(app.clone(), id, kind, vec![new, altered]).await,
                (StatusCode::CONFLICT, json!({"error":"conflict"})),
                "field {field}"
            );
            assert_eq!(event_count(&pool, id, kind).await, 1);
        }
        // Validation fails for a later event before any write is possible.
        let mut invalid = wire_event(kind);
        invalid["sequence"] = json!(-1);
        assert_eq!(
            submit(app.clone(), id, kind, vec![wire_event(kind), invalid])
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(event_count(&pool, id, kind).await, 1);
    }
    pool.close().await;
}
#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn finished_session_accepts_only_exact_replays_and_missing_sessions_are_404() {
    let (app, pool, member) = setup().await;
    for kind in ["telemetry", "connection-events"] {
        let session = start(app.clone(), &member, Uuid::new_v4()).await;
        let id = session["id"].as_str().unwrap();
        let event = wire_event(kind);
        assert_eq!(
            submit(
                app.clone(),
                &Uuid::new_v4().to_string(),
                kind,
                vec![event.clone()]
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            submit(app.clone(), id, kind, vec![event.clone()]).await.0,
            StatusCode::OK
        );
        assert_eq!(
            call(
                app.clone(),
                "POST",
                &format!("/api/v1/sessions/{id}/finish"),
                None
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            submit(app.clone(), id, kind, vec![event.clone()]).await,
            (StatusCode::OK, json!({"accepted":1}))
        );
        assert_eq!(
            submit(app.clone(), id, kind, vec![event.clone(), wire_event(kind)])
                .await
                .0,
            StatusCode::CONFLICT
        );
        let mut altered = event;
        altered["connection_id"] = json!(Uuid::new_v4());
        assert_eq!(
            submit(app.clone(), id, kind, vec![altered]).await.0,
            StatusCode::CONFLICT
        );
        assert_eq!(event_count(&pool, id, kind).await, 1);
    }
    pool.close().await;
}
#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn competing_contents_for_one_key_have_exactly_one_winner() {
    let (app, pool, member) = setup().await;
    for kind in ["telemetry", "connection-events"] {
        let session = start(app.clone(), &member, Uuid::new_v4()).await;
        let id = session["id"].as_str().unwrap();
        let a = wire_event(kind);
        let mut b = a.clone();
        b["connection_id"] = json!(Uuid::new_v4());
        let (ra, rb) = tokio::join!(
            submit(app.clone(), id, kind, vec![a.clone()]),
            submit(app.clone(), id, kind, vec![b.clone()])
        );
        assert!(
            (ra.0 == StatusCode::OK && rb.0 == StatusCode::CONFLICT)
                || (rb.0 == StatusCode::OK && ra.0 == StatusCode::CONFLICT)
        );
        let winner = if ra.0 == StatusCode::OK { a } else { b };
        assert_eq!(
            submit(app.clone(), id, kind, vec![winner]).await.0,
            StatusCode::OK
        );
        assert_eq!(event_count(&pool, id, kind).await, 1);
    }
    pool.close().await;
}
#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn finish_and_ingestion_races_are_serialized_without_partial_batches() {
    let (app, pool, member) = setup().await;
    for kind in ["telemetry", "connection-events"] {
        for _ in 0..4 {
            let session = start(app.clone(), &member, Uuid::new_v4()).await;
            let id = session["id"].as_str().unwrap();
            let events = vec![wire_event(kind), wire_event(kind)];
            let finish_path = format!("/api/v1/sessions/{id}/finish");
            let (ingest, finish) = tokio::join!(
                submit(app.clone(), id, kind, events.clone()),
                call(app.clone(), "POST", &finish_path, None)
            );
            assert_eq!(finish.0, StatusCode::OK);
            assert!(ingest.0 == StatusCode::OK || ingest.0 == StatusCode::CONFLICT);
            assert_eq!(
                event_count(&pool, id, kind).await,
                if ingest.0 == StatusCode::OK { 2 } else { 0 }
            );
            assert_eq!(submit(app.clone(), id, kind, events).await.0, ingest.0);
            assert_eq!(
                submit(app.clone(), id, kind, vec![wire_event(kind)])
                    .await
                    .0,
                StatusCode::CONFLICT
            );
        }
    }
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn event_schema_constraints_reject_invalid_rows_without_changing_recorded_data() {
    let (app, pool, member) = setup().await;
    let session = start(app.clone(), &member, Uuid::new_v4()).await;
    let id = session["id"].as_str().unwrap();
    for kind in ["telemetry", "connection-events"] {
        assert_eq!(
            submit(app.clone(), id, kind, vec![wire_event(kind)])
                .await
                .0,
            StatusCode::OK
        );
    }
    for query in [
        "UPDATE telemetry_event SET sequence=-1 WHERE session_id=$1",
        "UPDATE telemetry_event SET schema_version=2 WHERE session_id=$1",
        "UPDATE telemetry_event SET speed_kmh='NaN'::float8 WHERE session_id=$1",
        "UPDATE telemetry_event SET cadence_rpm='Infinity'::float8 WHERE session_id=$1",
        "UPDATE telemetry_event SET power_w=32768 WHERE session_id=$1",
        "UPDATE telemetry_event SET speed_kmh=NULL,cadence_rpm=NULL,power_w=NULL WHERE session_id=$1",
        "UPDATE telemetry_event SET source='cycling_power' WHERE session_id=$1",
        "UPDATE connection_event SET state='unknown' WHERE session_id=$1",
    ] {
        let error = sqlx::query(query)
            .bind(Uuid::parse_str(id).unwrap())
            .execute(&pool)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    assert_eq!(event_count(&pool, id, "telemetry").await, 1);
    assert_eq!(event_count(&pool, id, "connection-events").await, 1);
    pool.close().await;
}

async fn live_view(app: Router, member: &str, id: &str) -> Value {
    let (status, value) = as_user(
        app,
        Some(member),
        "GET",
        &format!("/api/v1/sessions/{id}/live"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    value
}
#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn live_projection_selects_sources_expires_metrics_and_clears_finished_sessions() {
    let (app, pool, member) = setup().await;
    let session = start(app.clone(), &member, Uuid::new_v4()).await;
    let id = session["id"].as_str().unwrap();
    let uuid = Uuid::parse_str(id).unwrap();
    let mut connection = wire_event("connection-events");
    let mut ftms = wire_event("telemetry");
    ftms["stream_id"] = connection["stream_id"].clone();
    ftms["connection_id"] = connection["connection_id"].clone();
    ftms["sequence"] = json!(1);
    assert_eq!(
        submit(
            app.clone(),
            id,
            "connection-events",
            vec![connection.clone()]
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        submit(app.clone(), id, "telemetry", vec![ftms.clone()])
            .await
            .0,
        StatusCode::OK
    );
    let view = live_view(app.clone(), &member, id).await;
    assert_eq!(view["power_source"], "ftms");
    assert_eq!(view["power_w"], -123);
    assert_eq!(view["speed_kmh"].as_f64(), Some(0.0));
    assert!(view["cadence_rpm"].is_null());
    let mut cycling = ftms.clone();
    cycling["sequence"] = json!(2);
    cycling["source"] = json!("cycling_power");
    cycling["speed_kmh"] = Value::Null;
    cycling["power_w"] = json!(180);
    assert_eq!(
        submit(app.clone(), id, "telemetry", vec![cycling]).await.0,
        StatusCode::OK
    );
    let view = live_view(app.clone(), &member, id).await;
    assert_eq!(view["power_source"], "cycling_power");
    assert_eq!(view["power_w"], 180);
    sqlx::query("UPDATE telemetry_event SET received_at=clock_timestamp()-interval '6 seconds' WHERE session_id=$1 AND source='cycling_power'").bind(uuid).execute(&pool).await.unwrap();
    assert_eq!(
        live_view(app.clone(), &member, id).await["power_source"],
        "ftms"
    );
    // Replaying does not refresh server receipt time.
    sqlx::query("UPDATE telemetry_event SET received_at=clock_timestamp()-interval '6 seconds' WHERE session_id=$1").bind(uuid).execute(&pool).await.unwrap();
    assert_eq!(
        submit(app.clone(), id, "telemetry", vec![ftms]).await.0,
        StatusCode::OK
    );
    let view = live_view(app.clone(), &member, id).await;
    assert_eq!(view["fresh"], false);
    assert!(view["power_w"].is_null());
    connection["state"] = json!("stopped");
    connection["sequence"] = json!(3);
    assert_eq!(
        submit(app.clone(), id, "connection-events", vec![connection])
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        live_view(app.clone(), &member, id).await["connection_state"],
        "stopped"
    );
    assert_eq!(
        call(
            app.clone(),
            "POST",
            &format!("/api/v1/sessions/{id}/finish"),
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    let view = live_view(app.clone(), &member, id).await;
    assert!(!view["session"]["finished_at"].is_null());
    assert_eq!(view["fresh"], false);
    pool.close().await;
}
#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn live_projection_never_reuses_an_old_connection_or_reorders_by_arrival() {
    let (app, pool, member) = setup().await;
    let session = start(app.clone(), &member, Uuid::new_v4()).await;
    let id = session["id"].as_str().unwrap();
    let mut connection = wire_event("connection-events");
    let mut data = wire_event("telemetry");
    data["stream_id"] = connection["stream_id"].clone();
    data["connection_id"] = connection["connection_id"].clone();
    data["sequence"] = json!(1);
    assert_eq!(
        submit(
            app.clone(),
            id,
            "connection-events",
            vec![connection.clone()]
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        submit(app.clone(), id, "telemetry", vec![data.clone()])
            .await
            .0,
        StatusCode::OK
    );
    let old_connection = connection["connection_id"].clone();
    connection["connection_id"] = json!(Uuid::new_v4());
    connection["sequence"] = json!(10);
    assert_eq!(
        submit(
            app.clone(),
            id,
            "connection-events",
            vec![connection.clone()]
        )
        .await
        .0,
        StatusCode::OK
    );
    let view = live_view(app.clone(), &member, id).await;
    assert_eq!(view["fresh"], false);
    assert!(view["power_w"].is_null());
    data["connection_id"] = connection["connection_id"].clone();
    data["sequence"] = json!(11);
    data["power_w"] = json!(150);
    assert_eq!(
        submit(app.clone(), id, "telemetry", vec![data.clone()])
            .await
            .0,
        StatusCode::OK
    );
    data["sequence"] = json!(9);
    data["power_w"] = json!(50);
    assert_eq!(
        submit(app.clone(), id, "telemetry", vec![data]).await.0,
        StatusCode::OK
    );
    assert_eq!(live_view(app.clone(), &member, id).await["power_w"], 150);
    // A delayed earlier connection event cannot override the higher sequence.
    let mut delayed = connection.clone();
    delayed["connection_id"] = old_connection;
    delayed["sequence"] = json!(5);
    delayed["state"] = json!("retry_wait");
    assert_eq!(
        submit(app.clone(), id, "connection-events", vec![delayed])
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        live_view(app.clone(), &member, id).await["connection_state"],
        "streaming"
    );
    // A new stream starts low; an even later packet from the old stream must not win.
    let new_stream = wire_event("connection-events");
    assert_eq!(
        submit(app.clone(), id, "connection-events", vec![new_stream])
            .await
            .0,
        StatusCode::OK
    );
    connection["sequence"] = json!(20);
    assert_eq!(
        submit(app.clone(), id, "connection-events", vec![connection])
            .await
            .0,
        StatusCode::OK
    );
    let view = live_view(app.clone(), &member, id).await;
    assert_eq!(view["fresh"], false);
    assert!(view["power_w"].is_null());
    pool.close().await;
}
#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn live_projection_ignores_events_without_receipt_times_and_reports_missing_sessions() {
    let (app, pool, member) = setup().await;
    let session = start(app.clone(), &member, Uuid::new_v4()).await;
    let id = session["id"].as_str().unwrap();
    let uuid = Uuid::parse_str(id).unwrap();
    let connection = wire_event("connection-events");
    assert_eq!(
        submit(app.clone(), id, "connection-events", vec![connection])
            .await
            .0,
        StatusCode::OK
    );
    sqlx::query("UPDATE connection_event SET received_at=NULL WHERE session_id=$1")
        .bind(uuid)
        .execute(&pool)
        .await
        .unwrap();
    let view = live_view(app.clone(), &member, id).await;
    assert!(view["connection_state"].is_null());
    assert_eq!(view["fresh"], false);
    assert_eq!(
        as_user(
            app,
            Some(&member),
            "GET",
            &format!("/api/v1/sessions/{}/live", Uuid::new_v4()),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    pool.close().await;
}

fn measurement(
    stream: Uuid,
    sequence: i64,
    at: &str,
    source: &str,
    power: Value,
    cadence: Value,
) -> Value {
    json!({"schema_version":1,"stream_id":stream,"connection_id":stream,"sequence":sequence,
        "observed_at":at,"source":source,"power_w":power,"cadence_rpm":cadence,
        "speed_kmh": if source == "ftms" { json!(30.0) } else { Value::Null }})
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn history_buckets_seconds_prefers_cycling_power_and_summarizes() {
    let (app, pool, member) = setup().await;
    let session = start(app.clone(), &member, Uuid::new_v4()).await;
    let id = session["id"].as_str().unwrap();
    let stream = Uuid::new_v4();
    let events = vec![
        measurement(
            stream,
            0,
            "2026-10-30T10:00:00.100Z",
            "ftms",
            json!(100),
            json!(80.0),
        ),
        measurement(
            stream,
            1,
            "2026-10-30T10:00:00.600Z",
            "ftms",
            json!(120),
            json!(90.0),
        ),
        measurement(
            stream,
            2,
            "2026-10-30T10:00:00.700Z",
            "cycling_power",
            json!(130),
            Value::Null,
        ),
        measurement(
            stream,
            3,
            "2026-10-30T10:00:01.200Z",
            "ftms",
            json!(0),
            Value::Null,
        ),
    ];
    assert_eq!(
        submit(app.clone(), id, "telemetry", events).await.0,
        StatusCode::OK
    );
    call(
        app.clone(),
        "POST",
        &format!("/api/v1/sessions/{id}/finish"),
        None,
    )
    .await;
    let (status, series) = as_user(
        app.clone(),
        Some(&member),
        "GET",
        &format!("/api/v1/sessions/{id}/series"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let points = series["points"].as_array().unwrap();
    assert_eq!(points.len(), 2);
    assert_eq!(points[0]["power_w"], 130);
    assert_eq!(points[0]["power_source"], "cycling_power");
    assert_eq!(points[0]["cadence_rpm"], 85.0);
    assert_eq!(points[1]["power_w"], 0);
    assert!(points[1]["cadence_rpm"].is_null());
    let summary = &series["summary"];
    assert_eq!(summary["average_power_w"], 65.0);
    assert_eq!(summary["max_power_w"], 130);
    assert_eq!(summary["energy_kj"], 0.13);
    assert_eq!(summary["power_seconds"], 2);
    assert!(
        series["ftp_w"].is_i64(),
        "new sessions capture the current FTP"
    );
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn history_pages_newest_first_with_a_stable_cursor() {
    let (app, pool, member) = setup().await;
    let older = start(app.clone(), &member, Uuid::new_v4()).await;
    let newer = start(app.clone(), &member, Uuid::new_v4()).await;
    let (status, page) = as_user(
        app.clone(),
        Some(&member),
        "GET",
        "/api/v1/sessions?limit=100",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = page["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["session"]["id"].as_str().unwrap())
        .collect();
    let position = |v: &Value| {
        ids.iter()
            .position(|id| *id == v["id"].as_str().unwrap())
            .unwrap()
    };
    assert!(position(&newer) < position(&older));
    let empty = page["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["session"]["id"] == newer["id"])
        .unwrap();
    assert_eq!(empty["summary"]["recorded_seconds"], 0);
    assert!(empty["summary"]["average_power_w"].is_null());
    let (_, next) = as_user(
        app.clone(),
        Some(&member),
        "GET",
        &format!(
            "/api/v1/sessions?limit=100&before={}",
            newer["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    for s in next["sessions"].as_array().unwrap() {
        assert_ne!(s["session"]["id"], newer["id"]);
        assert!(
            s["session"]["started_at"].as_str().unwrap() <= newer["started_at"].as_str().unwrap()
        );
    }
    let (_, unknown) = as_user(
        app,
        Some(&member),
        "GET",
        &format!("/api/v1/sessions?before={}", Uuid::new_v4()),
        None,
    )
    .await;
    assert_eq!(unknown["sessions"], json!([]));
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn activity_lists_only_this_members_rides_in_the_window() {
    let (app, pool, alice) = setup().await;
    let bob = register_member(&app).await;
    let alice_ride = start(app.clone(), &alice, Uuid::new_v4()).await;
    call(
        app.clone(),
        "POST",
        &format!(
            "/api/v1/sessions/{}/finish",
            alice_ride["id"].as_str().unwrap()
        ),
        None,
    )
    .await;
    let _bob_ride = start(app.clone(), &bob, Uuid::new_v4()).await;
    let (status, body) = as_user(
        app.clone(),
        Some(&alice),
        "GET",
        "/api/v1/activity?from=2026-01-01T00:00:00Z&to=2026-12-31T00:00:00Z",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let entries = body["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["started_at"], alice_ride["started_at"]);
    assert!(entries[0]["duration_seconds"].as_i64().unwrap() >= 0);
    let (status, other) = as_user(
        app,
        Some(&bob),
        "GET",
        "/api/v1/activity?from=2026-01-01T00:00:00Z&to=2026-12-31T00:00:00Z",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(other["entries"].as_array().unwrap().len(), 1);
    assert_ne!(other["entries"][0]["started_at"], alice_ride["started_at"]);
    pool.close().await;
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn ftp_setting_persists_and_is_captured_only_by_later_sessions() {
    let (app, pool, member) = setup().await;
    let put = |body: Value| {
        as_user(
            app.clone(),
            Some(&member),
            "PUT",
            "/api/v1/settings",
            Some(body),
        )
    };
    let earlier = start(app.clone(), &member, Uuid::new_v4()).await;
    let (status, saved) = put(json!({"ftp_w": 275})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["ftp_is_default"], false);
    assert_eq!(
        as_user(app.clone(), Some(&member), "GET", "/api/v1/settings", None)
            .await
            .1["ftp_w"],
        275
    );
    let later = start(app.clone(), &member, Uuid::new_v4()).await;
    let summary = |id: &Value| {
        let path = format!("/api/v1/sessions/{}/summary", id.as_str().unwrap());
        let app = app.clone();
        let member = member.clone();
        async move { as_user(app, Some(&member), "GET", &path, None).await }
    };
    assert_eq!(summary(&later["id"]).await.1["ftp_w"], 275);
    assert_ne!(summary(&earlier["id"]).await.1["ftp_w"], 275);
    assert_eq!(put(json!({"ftp_w": 0})).await.0, StatusCode::BAD_REQUEST);
    let (_, reset) = put(json!({"ftp_w": null})).await;
    assert_eq!(
        (reset["ftp_w"].clone(), reset["ftp_is_default"].clone()),
        (json!(200), json!(true))
    );
    pool.close().await;
}

async fn account_call(
    app: Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    payload: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json");
    if let Some(token) = token {
        request = request.header("Authorization", format!("Bearer {token}"));
    }
    let body = payload.map_or_else(Body::empty, |value| Body::from(value.to_string()));
    let response = app.oneshot(request.body(body).unwrap()).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL; run scripts/run_api.py --integration"]
async fn accounts_store_only_hashes_and_tokens_expire_or_log_out() {
    let (app, pool, _member) = setup().await;
    let username = format!("it_{}", &Uuid::new_v4().simple().to_string()[..12]);
    let credentials = json!({"username": username.to_uppercase(), "password": "correct horse"});
    let (status, registered) = account_call(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        Some(credentials.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let token = registered["token"].as_str().unwrap().to_owned();
    let (status, _) = account_call(
        app.clone(),
        "POST",
        "/api/v1/auth/register",
        None,
        Some(credentials.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (stored_name, password): (String, String) =
        sqlx::query_as("SELECT username, password FROM member WHERE username = $1")
            .bind(&username)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stored_name, username);
    assert!(password.starts_with("$argon2id$"));
    assert!(!password.contains("correct horse"));
    let raw_tokens: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM auth_session WHERE encode(token_hash, 'hex') = $1",
    )
    .bind(&token)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(raw_tokens, 0, "the plain token must never be stored");

    let me = account_call(app.clone(), "GET", "/api/v1/auth/me", Some(&token), None).await;
    assert_eq!(
        (me.0, me.1["username"].as_str()),
        (StatusCode::OK, Some(username.as_str()))
    );

    let (status, login) = account_call(
        app.clone(),
        "POST",
        "/api/v1/auth/login",
        None,
        Some(credentials),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let second = login["token"].as_str().unwrap().to_owned();
    sqlx::query(
        "UPDATE auth_session SET expires_at = clock_timestamp() - interval '1 second' \
        WHERE member_id = (SELECT id FROM member WHERE username = $1) \
        AND token_hash = sha256(convert_to($2, 'UTF8'))",
    )
    .bind(&username)
    .bind(&second)
    .execute(&pool)
    .await
    .unwrap();
    let expired = account_call(app.clone(), "GET", "/api/v1/auth/me", Some(&second), None).await;
    assert_eq!(expired.0, StatusCode::UNAUTHORIZED);

    let (status, _) = account_call(
        app.clone(),
        "POST",
        "/api/v1/auth/logout",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let after = account_call(app, "GET", "/api/v1/auth/me", Some(&token), None).await;
    assert_eq!(after.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "Requires isolated PostgreSQL"]
async fn members_cannot_see_or_steer_another_members_session() {
    let (app, pool, alice) = setup().await;
    let bob = register_member(&app).await;
    let session = start(app.clone(), &alice, Uuid::new_v4()).await;
    let id = session["id"].as_str().unwrap();
    for path in [
        format!("/api/v1/sessions/{id}"),
        format!("/api/v1/sessions/{id}/summary"),
        format!("/api/v1/sessions/{id}/series"),
        format!("/api/v1/sessions/{id}/live"),
    ] {
        assert_eq!(
            as_user(app.clone(), Some(&bob), "GET", &path, None).await.0,
            StatusCode::NOT_FOUND,
            "{path}"
        );
    }
    let (status, page) = as_user(
        app.clone(),
        Some(&bob),
        "GET",
        "/api/v1/sessions?limit=20",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["sessions"], json!([]));

    let (status, body) = as_user(
        app.clone(),
        Some(&bob),
        "POST",
        "/api/v1/agent/commands",
        Some(json!({"kind": "connect", "address": "AA:BB:CC:DD:EE:FF"})),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("trainer_in_use"))
    );

    assert_eq!(
        as_user(
            app.clone(),
            Some(&alice),
            "POST",
            "/api/v1/auth/logout",
            None
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    call(
        app.clone(),
        "POST",
        &format!("/api/v1/sessions/{id}/finish"),
        None,
    )
    .await;
    assert_eq!(
        as_user(app, Some(&alice), "POST", "/api/v1/auth/logout", None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    pool.close().await;
}
