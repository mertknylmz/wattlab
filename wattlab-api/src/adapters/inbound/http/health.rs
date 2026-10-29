use super::dto::HealthResponse;
use axum::Json;

/// Process liveness only. Does not claim PostgreSQL or trainer readiness.
pub(super) async fn check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "wattlab-api",
    })
}
