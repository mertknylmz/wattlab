use super::dto::ErrorResponse;
use crate::auth::ApiToken;
use axum::{
    Json,
    extract::{Request, State},
    http::{
        StatusCode,
        header::{AUTHORIZATION, WWW_AUTHENTICATE},
    },
    middleware::Next,
    response::{IntoResponse, Response},
};

pub(super) async fn require_token(
    State(token): State<ApiToken>,
    request: Request,
    next: Next,
) -> Response {
    let mut values = request.headers().get_all(AUTHORIZATION).iter();
    let authorized = match values.next() {
        Some(value) if values.next().is_none() => token.accepts(value.as_bytes()),
        _ => false,
    };
    if !authorized {
        return (
            StatusCode::UNAUTHORIZED,
            [(WWW_AUTHENTICATE, "Bearer")],
            Json(ErrorResponse {
                error: "unauthorized",
            }),
        )
            .into_response();
    }
    next.run(request).await
}
