mod support;

use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;
use wattlab_api::{auth::ApiToken, build_router};

const AGENT: &str = "test-only-token-for-integration-tests";

fn app() -> Router {
    build_router(
        ApiToken::parse(AGENT).unwrap(),
        Arc::new(support::StubRepository::default()),
    )
}

async fn call(
    app: &Router,
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
    let response = app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn credentials(username: &str, password: &str) -> Option<Value> {
    Some(json!({"username": username, "password": password}))
}

#[tokio::test]
async fn register_login_me_and_logout_use_a_per_member_token() {
    let app = app();
    let (status, registered) = call(
        &app,
        "POST",
        "/api/v1/auth/register",
        None,
        credentials("Holy", "correct horse"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(registered["member"]["username"], "holy");
    let token = registered["token"].as_str().unwrap().to_owned();
    assert_eq!(token.len(), 64);

    let (status, me) = call(&app, "GET", "/api/v1/auth/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["username"], "holy");

    let (status, login) = call(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        credentials("HOLY", "correct horse"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_ne!(login["token"], registered["token"]);

    let (status, _) = call(&app, "POST", "/api/v1/auth/logout", Some(&token), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&app, "GET", "/api/v1/auth/me", Some(&token), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let other = login["token"].as_str().unwrap();
    assert_eq!(
        call(&app, "GET", "/api/v1/auth/me", Some(other), None)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn registration_validates_input_and_rejects_taken_usernames() {
    let app = app();
    let register = |u: &'static str, p: &'static str| {
        let app = app.clone();
        async move {
            call(
                &app,
                "POST",
                "/api/v1/auth/register",
                None,
                credentials(u, p),
            )
            .await
        }
    };
    assert_eq!(
        register("ab", "correct horse").await.1["error"],
        "invalid_username"
    );
    assert_eq!(
        register("holy", "short").await.1["error"],
        "invalid_password"
    );
    assert_eq!(
        register("holy", "correct horse").await.0,
        StatusCode::CREATED
    );
    let (status, body) = register("HoLy", "another pass").await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::CONFLICT, Some("username_taken"))
    );
    let (status, _) = call(
        &app,
        "POST",
        "/api/v1/auth/register",
        None,
        Some(json!({"username": "x"})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn wrong_passwords_and_unknown_users_get_the_same_answer_then_throttle() {
    let app = app();
    call(
        &app,
        "POST",
        "/api/v1/auth/register",
        None,
        credentials("holy", "correct horse"),
    )
    .await;
    let unknown = call(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        credentials("nobody", "correct horse"),
    )
    .await;
    let wrong = call(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        credentials("holy", "wrong horse"),
    )
    .await;
    assert_eq!(unknown, wrong);
    assert_eq!(wrong.0, StatusCode::UNAUTHORIZED);
    assert_eq!(wrong.1["error"], "invalid_credentials");
    for _ in 0..4 {
        call(
            &app,
            "POST",
            "/api/v1/auth/login",
            None,
            credentials("holy", "wrong horse"),
        )
        .await;
    }
    let (status, body) = call(
        &app,
        "POST",
        "/api/v1/auth/login",
        None,
        credentials("holy", "correct horse"),
    )
    .await;
    assert_eq!(
        (status, body["error"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("too_many_attempts"))
    );
}

#[tokio::test]
async fn forged_tokens_are_refused_and_the_agent_is_not_a_member() {
    let app = app();
    let forged = "0".repeat(64);
    assert_eq!(
        call(&app, "GET", "/api/v1/auth/me", Some(&forged), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/api/v1/auth/me", None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, "GET", "/api/v1/auth/me", Some(AGENT), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&app, "POST", "/api/v1/auth/logout", Some(AGENT), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn members_cannot_start_sessions_and_the_agent_cannot_read_settings() {
    let app = app();
    let member = support::member_token(&app).await;
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/sessions",
            Some(&member),
            Some(json!({"request_id": uuid::Uuid::new_v4(), "command_id": uuid::Uuid::new_v4()})),
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(&app, "GET", "/api/v1/settings", Some(AGENT), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn logout_and_trainer_commands_respect_an_active_session() {
    let repo = std::sync::Arc::new(support::StubRepository::default());
    let app = wattlab_api::build_router(
        wattlab_api::auth::ApiToken::parse(AGENT).unwrap(),
        repo.clone(),
    );
    let token = support::member_token(&app).await;
    let (_, me) = call(&app, "GET", "/api/v1/auth/me", Some(&token), None).await;
    let id = uuid::Uuid::parse_str(me["id"].as_str().unwrap()).unwrap();
    *repo.active.lock().unwrap() = Some(id);
    assert_eq!(
        call(&app, "POST", "/api/v1/auth/logout", Some(&token), None)
            .await
            .1["error"],
        "active_session"
    );
    let other = support::member_token(&app).await;
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/agent/commands",
            Some(&other),
            Some(json!({"kind": "connect", "address": "AA:BB:CC:DD:EE:FF"})),
        )
        .await
        .1["error"],
        "trainer_in_use"
    );
    assert_eq!(
        call(
            &app,
            "POST",
            "/api/v1/agent/commands",
            Some(&token),
            Some(json!({"kind": "connect", "address": "AA:BB:CC:DD:EE:FF"})),
        )
        .await
        .0,
        StatusCode::ACCEPTED
    );
}
