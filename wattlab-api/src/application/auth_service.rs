use super::{
    error::ApplicationError,
    ports::{
        member_repository::MemberRepository, password_hasher::PasswordHasher,
        repository_error::RepositoryError, session_repository::SessionRepository,
    },
};
use crate::domain::member::{CredentialError, Member, Password, SessionToken, TokenHash, Username};
use chrono::{DateTime, Utc};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use uuid::Uuid;

/// Login sessions slide: each authenticated request extends them by this much.
pub const SESSION_TTL_SECS: i64 = 30 * 24 * 60 * 60;
const MAX_FAILURES: u32 = 5;
const LOCKOUT: Duration = Duration::from_secs(30);
const MAX_TRACKED_USERNAMES: usize = 1024;

pub struct Authenticated {
    pub member: Member,
    pub token: SessionToken,
    pub expires_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct AuthService {
    members: Arc<dyn MemberRepository>,
    sessions: Arc<dyn SessionRepository>,
    hasher: Arc<dyn PasswordHasher>,
    failures: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    /// Verified when the username is unknown so both paths cost the same.
    decoy_hash: Arc<OnceLock<String>>,
}

impl AuthService {
    pub fn new(
        members: Arc<dyn MemberRepository>,
        sessions: Arc<dyn SessionRepository>,
        hasher: Arc<dyn PasswordHasher>,
    ) -> Self {
        Self {
            members,
            sessions,
            hasher,
            failures: Arc::default(),
            decoy_hash: Arc::default(),
        }
    }

    pub async fn register(
        &self,
        username: &str,
        password: String,
    ) -> Result<Authenticated, ApplicationError> {
        let username = Username::parse(username).map_err(credential_error)?;
        let password = Password::parse(password).map_err(credential_error)?;
        let hash = self
            .hasher
            .hash(password)
            .await
            .map_err(|_| ApplicationError::StorageFailure)?;
        let member = self
            .members
            .create_member(Uuid::new_v4(), username, hash)
            .await
            .map_err(|error| match error {
                RepositoryError::Conflict => ApplicationError::UsernameTaken,
                other => other.into(),
            })?;
        self.issue(member).await
    }

    pub async fn login(
        &self,
        username: &str,
        password: String,
    ) -> Result<Authenticated, ApplicationError> {
        // Malformed input cannot match any account; it is reported like a wrong password.
        let (Ok(username), Ok(password)) = (Username::parse(username), Password::parse(password))
        else {
            return Err(ApplicationError::InvalidCredentials);
        };
        if self.locked(&username) {
            return Err(ApplicationError::TooManyAttempts);
        }
        let stored = self.members.credentials(username.clone()).await?;
        let verified = match stored {
            Some(stored) => self
                .hasher
                .verify(password, stored.password_hash)
                .await
                .then_some(stored.member),
            None => {
                let _ = self.hasher.verify(password, self.decoy().await).await;
                None
            }
        };
        match verified {
            Some(member) => {
                self.clear_failures(&username);
                self.issue(member).await
            }
            None => {
                self.record_failure(&username);
                Err(ApplicationError::InvalidCredentials)
            }
        }
    }

    /// `None` for unknown, expired or logged-out tokens.
    pub async fn authenticate(
        &self,
        token: &SessionToken,
    ) -> Result<Option<Member>, ApplicationError> {
        Ok(self
            .members
            .touch_auth_session(token.hash(), SESSION_TTL_SECS)
            .await?)
    }

    /// Refused while the member's own session is active, so a ride is never orphaned.
    pub async fn logout(&self, member: Uuid, token: TokenHash) -> Result<(), ApplicationError> {
        if self.sessions.active_owner().await? == Some(member) {
            return Err(ApplicationError::ActiveSession);
        }
        Ok(self.members.delete_auth_session(token).await?)
    }

    async fn issue(&self, member: Member) -> Result<Authenticated, ApplicationError> {
        let token = SessionToken::generate();
        let expires_at = self
            .members
            .create_auth_session(member.id, token.hash(), SESSION_TTL_SECS)
            .await?;
        Ok(Authenticated {
            member,
            token,
            expires_at,
        })
    }

    async fn decoy(&self) -> String {
        if let Some(hash) = self.decoy_hash.get() {
            return hash.clone();
        }
        let decoy = Password::parse("decoy-password-for-unknown-users".into());
        let hash = match decoy {
            Ok(password) => self.hasher.hash(password).await.unwrap_or_default(),
            Err(_) => String::new(),
        };
        self.decoy_hash.get_or_init(|| hash).clone()
    }

    fn locked(&self, username: &Username) -> bool {
        let Ok(failures) = self.failures.lock() else {
            return false;
        };
        failures
            .get(username.as_str())
            .is_some_and(|(count, last)| *count >= MAX_FAILURES && last.elapsed() < LOCKOUT)
    }

    fn record_failure(&self, username: &Username) {
        let Ok(mut failures) = self.failures.lock() else {
            return;
        };
        if failures.len() >= MAX_TRACKED_USERNAMES {
            failures.retain(|_, (_, last)| last.elapsed() < LOCKOUT);
        }
        let entry = failures
            .entry(username.as_str().to_owned())
            .or_insert((0, Instant::now()));
        if entry.1.elapsed() >= LOCKOUT {
            entry.0 = 0;
        }
        *entry = (entry.0.saturating_add(1), Instant::now());
    }

    fn clear_failures(&self, username: &Username) {
        if let Ok(mut failures) = self.failures.lock() {
            failures.remove(username.as_str());
        }
    }
}

fn credential_error(error: CredentialError) -> ApplicationError {
    match error {
        CredentialError::InvalidUsername => ApplicationError::InvalidUsername,
        CredentialError::InvalidPassword => ApplicationError::InvalidPassword,
    }
}
