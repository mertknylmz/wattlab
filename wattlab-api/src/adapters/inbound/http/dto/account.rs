use crate::{application::auth_service::Authenticated, domain::member::Member};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// No Debug: the password must never reach logs.
#[derive(Deserialize)]
pub(crate) struct CredentialsRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub(crate) struct MemberResponse {
    id: Uuid,
    username: String,
}
impl From<Member> for MemberResponse {
    fn from(member: Member) -> Self {
        Self {
            id: member.id,
            username: member.username.as_str().to_owned(),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct AuthResponse {
    member: MemberResponse,
    token: String,
    expires_at: DateTime<Utc>,
}
impl From<Authenticated> for AuthResponse {
    fn from(session: Authenticated) -> Self {
        Self {
            token: session.token.expose().to_owned(),
            member: session.member.into(),
            expires_at: session.expires_at,
        }
    }
}
