use super::session_repository::RepositoryFuture;
use crate::domain::member::{Member, TokenHash, Username};
use chrono::{DateTime, Utc};
use uuid::Uuid;

pub struct StoredCredentials {
    pub member: Member,
    pub password_hash: String,
}

/// Accounts and their login sessions. Expiry is evaluated with the storage clock.
pub trait MemberRepository: Send + Sync {
    /// `Conflict` when the username is already taken.
    fn create_member(
        &self,
        id: Uuid,
        username: Username,
        password_hash: String,
    ) -> RepositoryFuture<'_, Member>;
    fn credentials(&self, username: Username) -> RepositoryFuture<'_, Option<StoredCredentials>>;
    /// Returns the expiry of the new login session.
    fn create_auth_session(
        &self,
        member_id: Uuid,
        token: TokenHash,
        ttl_secs: i64,
    ) -> RepositoryFuture<'_, DateTime<Utc>>;
    /// Extends an unexpired login session by `ttl_secs`; expired or unknown yields `None`.
    fn touch_auth_session(
        &self,
        token: TokenHash,
        ttl_secs: i64,
    ) -> RepositoryFuture<'_, Option<Member>>;
    fn delete_auth_session(&self, token: TokenHash) -> RepositoryFuture<'_, ()>;
}
