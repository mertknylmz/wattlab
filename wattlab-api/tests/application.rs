mod support;

use chrono::{TimeZone, Utc};
use std::sync::Arc;
use support::StubRepository;
use uuid::Uuid;
use wattlab_api::{
    application::{
        error::ApplicationError, ports::repository_error::RepositoryError,
        session_service::SessionService,
    },
    domain::session::Session,
};

#[tokio::test]
async fn missing_sessions_are_application_errors_without_http_or_database() {
    let service = SessionService::new(Arc::new(StubRepository::default()));
    assert_eq!(
        service.get(Uuid::new_v4(), None).await,
        Err(ApplicationError::SessionNotFound)
    );
    assert_eq!(
        service.finish(Uuid::new_v4(), None).await,
        Err(ApplicationError::SessionNotFound)
    );
}

#[tokio::test]
async fn repository_failures_are_mapped_without_driver_types() {
    let service = SessionService::new(support::unavailable_repository());
    assert_eq!(
        service.start(Uuid::new_v4(), Uuid::nil()).await,
        Err(ApplicationError::StorageUnavailable)
    );
    for error in [RepositoryError::Unexpected, RepositoryError::InvalidData] {
        let service = SessionService::new(Arc::new(StubRepository {
            session: None,
            failure: Some(error),
            ..Default::default()
        }));
        assert_eq!(
            service.get(Uuid::new_v4(), None).await,
            Err(ApplicationError::StorageFailure)
        );
    }
}

#[tokio::test]
async fn service_returns_domain_values_through_the_port() {
    let request = Uuid::new_v4();
    let session = Session::restore(
        Uuid::new_v4(),
        request,
        Utc.timestamp_opt(1_000, 0).unwrap(),
        None,
    )
    .unwrap();
    let service = SessionService::new(Arc::new(StubRepository {
        session: Some(session.clone()),
        failure: None,
        ..Default::default()
    }));
    assert_eq!(service.start(request, Uuid::nil()).await.unwrap(), session);
    assert_eq!(service.get(session.id(), None).await.unwrap(), session);
}
