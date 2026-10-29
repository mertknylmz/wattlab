use chrono::{TimeZone, Utc};
use uuid::Uuid;
use wattlab_api::domain::{error::DomainError, session::Session};

#[test]
fn session_rejects_finish_before_start() {
    let start = Utc.timestamp_opt(1_000, 0).unwrap();
    let earlier = Utc.timestamp_opt(999, 0).unwrap();
    assert_eq!(
        Session::restore(Uuid::new_v4(), Uuid::new_v4(), start, Some(earlier)),
        Err(DomainError::FinishBeforeStart)
    );
}

#[test]
fn open_and_finished_sessions_preserve_identity_and_times() {
    let id = Uuid::new_v4();
    let request = Uuid::new_v4();
    let start = Utc.timestamp_opt(1_000, 0).unwrap();
    let open = Session::restore(id, request, start, None).unwrap();
    assert_eq!(
        (
            open.id(),
            open.request_id(),
            open.started_at(),
            open.finished_at()
        ),
        (id, request, start, None)
    );
    // A zero-duration session is valid; finishing earlier is not.
    let finished = Session::restore(id, request, start, Some(start)).unwrap();
    assert_eq!(finished.finished_at(), Some(start));
}
