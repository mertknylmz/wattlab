use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct ErrorResponse {
    pub error: &'static str,
}
