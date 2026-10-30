use std::{
    collections::HashMap,
    future::ready,
    sync::{Arc, Mutex},
};
use uuid::Uuid;
use wattlab_api::{
    application::ports::{
        history_repository::HistoryRepository,
        member_repository::{MemberRepository, StoredCredentials},
        repository_error::RepositoryError,
        session_repository::{RepositoryFuture, SessionRepository},
        settings_repository::SettingsRepository,
    },
    domain::{
        history::{ActivityEntry, SeriesPoint, SessionRecord},
        member::{Member, TokenHash, Username},
        session::Session,
        settings::{AthleteSettings, Ftp},
    },
};

/// Members with password hashes, and login token hashes with their owner.
type Accounts = (Vec<(Member, String)>, Vec<([u8; 32], Uuid)>);

#[derive(Default)]
pub struct StubRepository {
    pub session: Option<Session>,
    pub failure: Option<RepositoryError>,
    pub records: Vec<SessionRecord>,
    pub points: Vec<SeriesPoint>,
    pub accounts: Mutex<Accounts>,
    /// Owner of an unfinished session, for trainer-in-use and logout checks.
    pub active: Mutex<Option<Uuid>>,
}

impl StubRepository {
    fn result<T>(&self, value: T) -> RepositoryFuture<'_, T>
    where
        T: Send + 'static,
    {
        Box::pin(ready(self.failure.map_or(Ok(value), Err)))
    }
}

impl SessionRepository for StubRepository {
    fn start_or_get(&self, _: Uuid, _: Uuid) -> RepositoryFuture<'_, Session> {
        let result = match self.failure {
            Some(error) => Err(error),
            None => self.session.clone().ok_or(RepositoryError::Unexpected),
        };
        Box::pin(ready(result))
    }
    fn find(&self, _: Uuid, _: Option<Uuid>) -> RepositoryFuture<'_, Option<Session>> {
        self.result(self.session.clone())
    }
    fn finish_once(&self, id: Uuid, owner: Option<Uuid>) -> RepositoryFuture<'_, Option<Session>> {
        self.find(id, owner)
    }
    fn active_owner(&self) -> RepositoryFuture<'_, Option<Uuid>> {
        self.result(*self.active.lock().unwrap())
    }
    fn ingest_measurements(
        &self,
        _: Uuid,
        batch: wattlab_api::domain::events::EventBatch<wattlab_api::domain::events::Measurement>,
    ) -> RepositoryFuture<'_, usize> {
        self.result(batch.events().len())
    }
    fn ingest_connections(
        &self,
        _: Uuid,
        batch: wattlab_api::domain::events::EventBatch<
            wattlab_api::domain::events::ConnectionEvent,
        >,
    ) -> RepositoryFuture<'_, usize> {
        self.result(batch.events().len())
    }
    fn live_snapshot(
        &self,
        _owner: Uuid,
        _session_id: Option<Uuid>,
    ) -> RepositoryFuture<'_, Option<wattlab_api::domain::live::LiveSnapshot>> {
        self.result(None)
    }
}

impl HistoryRepository for StubRepository {
    fn sessions_page(
        &self,
        _: Uuid,
        _: Option<Uuid>,
        limit: u32,
    ) -> RepositoryFuture<'_, Vec<SessionRecord>> {
        self.result(self.records.iter().take(limit as usize).cloned().collect())
    }
    fn session_record(&self, _: Uuid, id: Uuid) -> RepositoryFuture<'_, Option<SessionRecord>> {
        self.result(self.records.iter().find(|r| r.session.id() == id).cloned())
    }
    fn series(&self, ids: Vec<Uuid>) -> RepositoryFuture<'_, HashMap<Uuid, Vec<SeriesPoint>>> {
        // Every requested session receives the same fixture points.
        self.result(
            ids.into_iter()
                .map(|id| (id, self.points.clone()))
                .collect(),
        )
    }
    fn activity(
        &self,
        _: Uuid,
        from: chrono::DateTime<chrono::Utc>,
        to: chrono::DateTime<chrono::Utc>,
    ) -> RepositoryFuture<'_, Vec<ActivityEntry>> {
        self.result(
            self.records
                .iter()
                .filter(|record| {
                    let start = record.session.started_at();
                    start >= from && start < to
                })
                .map(|record| ActivityEntry {
                    started_at: record.session.started_at(),
                    duration_seconds: record
                        .session
                        .finished_at()
                        .map(|finish| (finish - record.session.started_at()).num_seconds())
                        .unwrap_or(0)
                        .max(0),
                })
                .collect(),
        )
    }
}

impl SettingsRepository for StubRepository {
    fn settings(&self, _: Uuid) -> RepositoryFuture<'_, AthleteSettings> {
        self.result(AthleteSettings {
            ftp: Ftp::default_reference(),
            ftp_is_default: true,
            updated_at: chrono::DateTime::UNIX_EPOCH,
        })
    }
    fn update_ftp(&self, _: Uuid, ftp: Option<Ftp>) -> RepositoryFuture<'_, AthleteSettings> {
        self.result(AthleteSettings {
            ftp: ftp.unwrap_or(Ftp::default_reference()),
            ftp_is_default: ftp.is_none(),
            updated_at: chrono::DateTime::UNIX_EPOCH,
        })
    }
}

#[allow(dead_code)] // Not every test binary exercises storage failures.
pub fn unavailable_repository() -> Arc<StubRepository> {
    Arc::new(StubRepository {
        failure: Some(RepositoryError::Unavailable),
        ..StubRepository::default()
    })
}

impl MemberRepository for StubRepository {
    fn create_member(
        &self,
        id: Uuid,
        username: Username,
        password_hash: String,
    ) -> RepositoryFuture<'_, Member> {
        let mut accounts = self.accounts.lock().unwrap();
        let result = if accounts.0.iter().any(|(m, _)| m.username == username) {
            Err(RepositoryError::Conflict)
        } else {
            let member = Member {
                id,
                username,
                created_at: chrono::DateTime::UNIX_EPOCH,
            };
            accounts.0.push((member.clone(), password_hash));
            Ok(member)
        };
        Box::pin(ready(result))
    }
    fn credentials(&self, username: Username) -> RepositoryFuture<'_, Option<StoredCredentials>> {
        let accounts = self.accounts.lock().unwrap();
        let found =
            accounts
                .0
                .iter()
                .find(|(m, _)| m.username == username)
                .map(|(member, hash)| StoredCredentials {
                    member: member.clone(),
                    password_hash: hash.clone(),
                });
        Box::pin(ready(Ok(found)))
    }
    fn create_auth_session(
        &self,
        member_id: Uuid,
        token: TokenHash,
        _: i64,
    ) -> RepositoryFuture<'_, chrono::DateTime<chrono::Utc>> {
        self.accounts.lock().unwrap().1.push((token.0, member_id));
        Box::pin(ready(Ok(chrono::DateTime::UNIX_EPOCH)))
    }
    fn touch_auth_session(&self, token: TokenHash, _: i64) -> RepositoryFuture<'_, Option<Member>> {
        let accounts = self.accounts.lock().unwrap();
        let member = accounts
            .1
            .iter()
            .find(|(hash, _)| *hash == token.0)
            .and_then(|(_, id)| accounts.0.iter().find(|(m, _)| m.id == *id))
            .map(|(member, _)| member.clone());
        Box::pin(ready(Ok(member)))
    }
    fn delete_auth_session(&self, token: TokenHash) -> RepositoryFuture<'_, ()> {
        self.accounts
            .lock()
            .unwrap()
            .1
            .retain(|(hash, _)| *hash != token.0);
        Box::pin(ready(Ok(())))
    }
}

/// Registers a fresh member through the router and returns their login token.
#[allow(dead_code)] // Not every test binary calls member endpoints.
pub async fn member_token(app: &axum::Router) -> String {
    use tower::ServiceExt;
    let username = format!("m{}", &Uuid::new_v4().simple().to_string()[..12]);
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/api/v1/auth/register")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            serde_json::json!({"username": username, "password": "correct horse"}).to_string(),
        ))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let bytes = http_body_util::BodyExt::collect(response.into_body())
        .await
        .unwrap()
        .to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    body["token"].as_str().unwrap().to_owned()
}
