mod support;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};
use tower::ServiceExt;
use uuid::Uuid;
use wattlab_api::{App, auth::ApiToken, build_app};

const TOKEN: &str = "test-only-token-for-integration-tests";

fn app() -> App {
    build_app(
        ApiToken::parse(TOKEN).unwrap(),
        Arc::new(support::StubRepository::default()),
    )
}

async fn call(
    app: &Router,
    token: &str,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("Authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
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
async fn command_lifecycle_runs_from_submission_to_agent_result() {
    let app = app().router;
    let member = support::member_token(&app).await;
    let (status, command) = call(
        &app,
        &member,
        "POST",
        "/api/v1/agent/commands",
        Some(json!({"kind":"set_target_power","target_power_w":180})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(command["status"], "pending");
    assert_eq!(command["target_power_w"], 180);
    let id = command["id"].as_str().unwrap();

    let (status, delivered) = call(
        &app,
        TOKEN,
        "GET",
        "/api/v1/agent/commands/next?wait_seconds=1",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (delivered["id"].as_str(), delivered["status"].as_str()),
        (Some(id), Some("delivered"))
    );
    assert_eq!(
        call(&app, TOKEN, "GET", "/api/v1/agent/commands/next", None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );

    let result = json!({"status":"rejected","message":"ERG not supported"});
    let path = format!("/api/v1/agent/commands/{id}/result");
    let (status, done) = call(&app, TOKEN, "PUT", &path, Some(result.clone())).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(done["status"], "rejected");
    assert_eq!(
        call(&app, TOKEN, "PUT", &path, Some(result)).await.0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &app,
            TOKEN,
            "PUT",
            &path,
            Some(json!({"status":"succeeded"}))
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, fetched) = call(
        &app,
        &member,
        "GET",
        &format!("/api/v1/agent/commands/{id}"),
        None,
    )
    .await;
    assert_eq!(fetched["message"], "ERG not supported");
}

#[tokio::test]
async fn invalid_commands_and_unknown_ids_are_rejected() {
    let app = app().router;
    let member = support::member_token(&app).await;
    for body in [
        json!({"kind":"explode"}),
        json!({"kind":"connect"}),
        json!({"kind":"connect","address":"trainer"}),
        json!({"kind":"set_target_power"}),
        json!({"kind":"set_target_power","target_power_w":2001}),
    ] {
        assert_eq!(
            call(
                &app,
                &member,
                "POST",
                "/api/v1/agent/commands",
                Some(body.clone())
            )
            .await
            .0,
            StatusCode::BAD_REQUEST,
            "{body}"
        );
    }
    assert_eq!(
        call(
            &app,
            TOKEN,
            "GET",
            "/api/v1/agent/commands/next?wait_seconds=21",
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let unknown = format!("/api/v1/agent/commands/{}/result", Uuid::new_v4());
    assert_eq!(
        call(
            &app,
            TOKEN,
            "PUT",
            &unknown,
            Some(json!({"status":"succeeded"}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn heartbeat_is_validated_and_reported_while_fresh() {
    let app = app().router;
    let (_, before) = call(&app, TOKEN, "GET", "/api/v1/agent/status", None).await;
    assert_eq!(before["online"], false);
    assert!(before["last_seen_at"].is_null());
    let beat = json!({"agent_id":Uuid::new_v4(),"connection_state":"streaming","session_id":null,
        "erg_supported":true,"power_range_w":[0,1500],"target_power_w":null});
    assert_eq!(
        call(&app, TOKEN, "PUT", "/api/v1/agent/status", Some(beat))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (_, after) = call(&app, TOKEN, "GET", "/api/v1/agent/status", None).await;
    assert_eq!(after["online"], true);
    assert_eq!(after["connection_state"], "streaming");
    assert_eq!(after["power_range_w"], json!([0, 1500]));
    assert_eq!(after["devices"], json!([]));
    assert!(after["trainer_address"].is_null());
    for invalid in [
        json!({"agent_id":Uuid::new_v4(),"connection_state":"dancing","session_id":null,"erg_supported":false,"power_range_w":null,"target_power_w":null}),
        json!({"agent_id":Uuid::new_v4(),"connection_state":"idle","session_id":null,"erg_supported":false,"power_range_w":[500,100],"target_power_w":null}),
    ] {
        assert_eq!(
            call(&app, TOKEN, "PUT", "/api/v1/agent/status", Some(invalid))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
}

#[tokio::test]
async fn connect_carries_the_selected_address_and_scan_results_ride_on_status() {
    let app = app().router;
    let member = support::member_token(&app).await;
    let (status, command) = call(
        &app,
        &member,
        "POST",
        "/api/v1/agent/commands",
        Some(json!({"kind":"connect","address":"aa-bb-cc-dd-ee-ff"})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(command["kind"], "connect");
    assert_eq!(command["address"], "AA:BB:CC:DD:EE:FF");
    let (status, scan) = call(
        &app,
        &member,
        "POST",
        "/api/v1/agent/commands",
        Some(json!({"kind":"scan"})),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(scan["kind"], "scan");
    let beat = json!({
        "agent_id": Uuid::new_v4(),
        "connection_state": "idle",
        "session_id": null,
        "erg_supported": false,
        "power_range_w": null,
        "target_power_w": null,
        "trainer_address": "aa:bb:cc:dd:ee:ff",
        "devices": [{"address":"aa:bb:cc:dd:ee:ff","name":"D500","rssi":-48}]
    });
    assert_eq!(
        call(&app, TOKEN, "PUT", "/api/v1/agent/status", Some(beat))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    let (_, after) = call(&app, TOKEN, "GET", "/api/v1/agent/status", None).await;
    assert_eq!(after["trainer_address"], "AA:BB:CC:DD:EE:FF");
    assert_eq!(after["devices"][0]["name"], "D500");
}

#[tokio::test]
async fn shutdown_releases_waiting_agents_and_ends_live_streams() {
    let App { router, shutdown } = app();
    let member = support::member_token(&router).await;
    let waiting = tokio::spawn({
        let router = router.clone();
        async move {
            call(
                &router,
                TOKEN,
                "GET",
                "/api/v1/agent/commands/next?wait_seconds=20",
                None,
            )
            .await
            .0
        }
    });
    let stream = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/live/stream")
                .header("Authorization", format!("Bearer {member}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stream.status(), StatusCode::OK);
    tokio::time::sleep(Duration::from_millis(50)).await;
    shutdown.trigger();
    let status = tokio::time::timeout(Duration::from_secs(2), waiting)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(status, StatusCode::NO_CONTENT);
    let body = tokio::time::timeout(Duration::from_secs(3), stream.into_body().collect())
        .await
        .expect("stream must end after shutdown")
        .unwrap()
        .to_bytes();
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(
        text.starts_with("event: snapshot\ndata: {\"live\":"),
        "{text}"
    );
    assert!(text.contains("\"agent\":{\"online\":false"));
}

#[tokio::test]
async fn members_steer_and_only_the_agent_executes() {
    let app = app().router;
    let member = support::member_token(&app).await;
    let other = support::member_token(&app).await;
    let connect = Some(json!({"kind":"connect","address":"AA:BB:CC:DD:EE:FF"}));
    let forbidden = call(
        &app,
        TOKEN,
        "POST",
        "/api/v1/agent/commands",
        connect.clone(),
    )
    .await;
    assert_eq!(forbidden.0, StatusCode::FORBIDDEN);
    let (_, command) = call(&app, &member, "POST", "/api/v1/agent/commands", connect).await;
    let path = format!("/api/v1/agent/commands/{}", command["id"].as_str().unwrap());
    assert_eq!(
        call(&app, &other, "GET", &path, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&app, &member, "GET", &path, None).await.0,
        StatusCode::OK
    );
    for (method, agent_path) in [
        ("GET", "/api/v1/agent/commands/next"),
        ("PUT", "/api/v1/agent/status"),
    ] {
        let status = call(&app, &member, method, agent_path, Some(json!({})))
            .await
            .0;
        assert_eq!(status, StatusCode::FORBIDDEN, "{agent_path}");
    }
    assert_eq!(
        call(&app, &member, "GET", "/api/v1/agent/status", None)
            .await
            .0,
        StatusCode::OK
    );
}
