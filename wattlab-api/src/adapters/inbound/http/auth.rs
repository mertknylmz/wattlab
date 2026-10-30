use super::{dto::ErrorResponse, error::ApiError};
use crate::{
    application::auth_service::AuthService,
    auth::ApiToken,
    domain::member::{Member, SessionToken, TokenHash},
};
use axum::{
    Json,
    extract::{FromRequestParts, Request, State},
    http::{
        StatusCode,
        header::{AUTHORIZATION, WWW_AUTHENTICATE},
        request::Parts,
    },
    middleware::Next,
    response::{IntoResponse, Response},
};
use uuid::Uuid;

/// Who is calling: the local trainer agent or a logged-in member.
#[derive(Clone)]
pub(super) enum Principal {
    Agent,
    Member { member: Member, token: TokenHash },
}

#[derive(Clone)]
pub(super) struct Gate {
    pub agent: ApiToken,
    pub auth: AuthService,
}

pub(super) async fn require_principal(
    State(gate): State<Gate>,
    mut request: Request,
    next: Next,
) -> Response {
    let mut values = request.headers().get_all(AUTHORIZATION).iter();
    let header = match (values.next(), values.next()) {
        (Some(value), None) => value.as_bytes().to_vec(),
        _ => return unauthorized(),
    };
    let principal = if gate.agent.accepts(&header) {
        Principal::Agent
    } else {
        let Some(token) = member_token(&header) else {
            return unauthorized();
        };
        match gate.auth.authenticate(&token).await {
            Ok(Some(member)) => Principal::Member {
                member,
                token: token.hash(),
            },
            Ok(None) => return unauthorized(),
            Err(error) => return ApiError::from(error).into_response(),
        }
    };
    request.extensions_mut().insert(principal);
    next.run(request).await
}

/// Handlers for member screens: the agent has no member identity.
pub(super) struct MemberId(pub Uuid);

/// Handlers only the trainer agent may call.
pub(super) struct AgentOnly;

/// `Some(member)` restricts reads to that member; the agent sees every session.
pub(super) struct Scope(pub Option<Uuid>);

impl<S: Send + Sync> FromRequestParts<S> for MemberId {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        match parts.extensions.get::<Principal>() {
            Some(Principal::Member { member, .. }) => Ok(Self(member.id)),
            _ => Err(ApiError::Forbidden),
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for AgentOnly {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        match parts.extensions.get::<Principal>() {
            Some(Principal::Agent) => Ok(Self),
            _ => Err(ApiError::Forbidden),
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Scope {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, ApiError> {
        match parts.extensions.get::<Principal>() {
            Some(Principal::Agent) => Ok(Self(None)),
            Some(Principal::Member { member, .. }) => Ok(Self(Some(member.id))),
            None => Err(ApiError::Forbidden),
        }
    }
}

fn member_token(header: &[u8]) -> Option<SessionToken> {
    let value = std::str::from_utf8(header).ok()?;
    SessionToken::parse(value.strip_prefix("Bearer ")?)
}

fn unauthorized() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(WWW_AUTHENTICATE, "Bearer")],
        Json(ErrorResponse {
            error: "unauthorized",
        }),
    )
        .into_response()
}
