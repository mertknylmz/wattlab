use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct HealthResponse {
    pub status: &'static str,
    pub service: &'static str,
}
