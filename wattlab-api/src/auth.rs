//! Service credential of the local trainer agent. Member login tokens are domain types.

use std::{fmt, sync::Arc};
use subtle::ConstantTimeEq;

/// Service credential of the local trainer agent (`WATTLAB_AGENT_TOKEN`).
#[derive(Clone)]
pub struct ApiToken(Arc<str>);

#[derive(Debug)]
pub struct InvalidToken;

impl ApiToken {
    pub fn parse(value: &str) -> Result<Self, InvalidToken> {
        if !(32..=64).contains(&value.len())
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(InvalidToken);
        }
        Ok(Self(Arc::from(format!("Bearer {value}"))))
    }

    pub(crate) fn accepts(&self, authorization: &[u8]) -> bool {
        bool::from(self.0.as_bytes().ct_eq(authorization))
    }
}

impl fmt::Debug for ApiToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ApiToken([REDACTED])")
    }
}
