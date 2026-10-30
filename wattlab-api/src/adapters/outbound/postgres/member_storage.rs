use super::{PgSessionRepository, error::repository_error};
use crate::{
    application::ports::{
        member_repository::{MemberRepository, StoredCredentials},
        repository_error::RepositoryError,
        session_repository::RepositoryFuture,
    },
    domain::member::{Member, TokenHash, Username},
};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct MemberRow {
    id: Uuid,
    username: String,
    created_at: DateTime<Utc>,
}
impl MemberRow {
    fn into_domain(self) -> Result<Member, RepositoryError> {
        Ok(Member {
            id: self.id,
            username: Username::parse(&self.username).map_err(|_| RepositoryError::InvalidData)?,
            created_at: self.created_at,
        })
    }
}

#[derive(sqlx::FromRow)]
struct CredentialsRow {
    #[sqlx(flatten)]
    member: MemberRow,
    password: String,
}

fn unique_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(|error| error.code())
        .is_some_and(|code| code == "23505")
}

impl MemberRepository for PgSessionRepository {
    fn create_member(
        &self,
        id: Uuid,
        username: Username,
        password_hash: String,
    ) -> RepositoryFuture<'_, Member> {
        Box::pin(async move {
            let row: MemberRow = sqlx::query_as(
                "INSERT INTO member (id, username, password) VALUES ($1, $2, $3) \
                RETURNING id, username, created_at",
            )
            .bind(id)
            .bind(username.as_str())
            .bind(password_hash)
            .fetch_one(&self.pool)
            .await
            .map_err(|error| {
                if unique_violation(&error) {
                    RepositoryError::Conflict
                } else {
                    repository_error(error)
                }
            })?;
            row.into_domain()
        })
    }

    fn credentials(&self, username: Username) -> RepositoryFuture<'_, Option<StoredCredentials>> {
        Box::pin(async move {
            let row: Option<CredentialsRow> = sqlx::query_as(
                "SELECT id, username, created_at, password FROM member WHERE username = $1",
            )
            .bind(username.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(repository_error)?;
            row.map(|row| {
                Ok(StoredCredentials {
                    member: row.member.into_domain()?,
                    password_hash: row.password,
                })
            })
            .transpose()
        })
    }

    fn create_auth_session(
        &self,
        member_id: Uuid,
        token: TokenHash,
        ttl_secs: i64,
    ) -> RepositoryFuture<'_, DateTime<Utc>> {
        Box::pin(async move {
            // Expired rows are pruned on the way; they can never authenticate anyway.
            sqlx::query("DELETE FROM auth_session WHERE expires_at <= clock_timestamp()")
                .execute(&self.pool)
                .await
                .map_err(repository_error)?;
            sqlx::query_scalar(
                "INSERT INTO auth_session (token_hash, member_id, expires_at) \
                VALUES ($1, $2, clock_timestamp() + make_interval(secs => $3)) \
                RETURNING expires_at",
            )
            .bind(token.0.as_slice())
            .bind(member_id)
            .bind(ttl_secs as f64)
            .fetch_one(&self.pool)
            .await
            .map_err(repository_error)
        })
    }

    fn touch_auth_session(
        &self,
        token: TokenHash,
        ttl_secs: i64,
    ) -> RepositoryFuture<'_, Option<Member>> {
        Box::pin(async move {
            let row: Option<MemberRow> = sqlx::query_as(
                "UPDATE auth_session AS a SET last_used_at = clock_timestamp(), \
                expires_at = clock_timestamp() + make_interval(secs => $2) \
                FROM member AS m \
                WHERE a.token_hash = $1 AND a.expires_at > clock_timestamp() AND m.id = a.member_id \
                RETURNING m.id, m.username, m.created_at",
            )
            .bind(token.0.as_slice())
            .bind(ttl_secs as f64)
            .fetch_optional(&self.pool)
            .await
            .map_err(repository_error)?;
            row.map(MemberRow::into_domain).transpose()
        })
    }

    fn delete_auth_session(&self, token: TokenHash) -> RepositoryFuture<'_, ()> {
        Box::pin(async move {
            sqlx::query("DELETE FROM auth_session WHERE token_hash = $1")
                .bind(token.0.as_slice())
                .execute(&self.pool)
                .await
                .map_err(repository_error)?;
            Ok(())
        })
    }
}
