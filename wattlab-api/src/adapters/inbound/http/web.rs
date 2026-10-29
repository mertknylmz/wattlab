use axum::{
    http::{
        HeaderName,
        header::{CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE},
    },
    response::IntoResponse,
};
fn asset(mime: &'static str, body: &'static str) -> impl IntoResponse {
    (
        [
            (CONTENT_TYPE, mime),
            (CACHE_CONTROL, "no-store"),
            (
                CONTENT_SECURITY_POLICY,
                "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
            ),
            (HeaderName::from_static("x-content-type-options"), "nosniff"),
        ],
        body,
    )
}
pub(super) async fn index() -> impl IntoResponse {
    asset(
        "text/html; charset=utf-8",
        include_str!("../../../../web/index.html"),
    )
}
pub(super) async fn script() -> impl IntoResponse {
    asset(
        "text/javascript; charset=utf-8",
        include_str!("../../../../web/app.js"),
    )
}
pub(super) async fn style() -> impl IntoResponse {
    asset(
        "text/css; charset=utf-8",
        include_str!("../../../../web/styles.css"),
    )
}
