//! Local accounts: username and password rules. Hashing lives behind a port.

use chrono::{DateTime, Utc};
use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialError {
    InvalidUsername,
    InvalidPassword,
}

/// Case-insensitive: stored and compared in lowercase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Username(String);

impl Username {
    pub fn parse(value: &str) -> Result<Self, CredentialError> {
        let normalized = value.to_ascii_lowercase();
        let valid = (3..=32).contains(&normalized.len())
            && normalized.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'_' | b'.' | b'-')
            });
        if !valid {
            return Err(CredentialError::InvalidUsername);
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Length-only policy (8–128 characters); composition rules are deliberately absent.
#[derive(Clone)]
pub struct Password(String);

impl Password {
    pub fn parse(value: String) -> Result<Self, CredentialError> {
        if !(8..=128).contains(&value.chars().count()) {
            return Err(CredentialError::InvalidPassword);
        }
        Ok(Self(value))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Password([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub id: Uuid,
    pub username: Username,
    pub created_at: DateTime<Utc>,
}

/// Opaque login token handed to a member once; only its hash is stored.
pub struct SessionToken(String);

/// SHA-256 of a login token: the only form that reaches storage.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct TokenHash(pub [u8; 32]);

impl SessionToken {
    pub fn generate() -> Self {
        let mut bytes = [0_u8; 32];
        OsRng.fill_bytes(&mut bytes);
        Self(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
    }

    pub fn parse(value: &str) -> Option<Self> {
        (value.len() == 64
            && value
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f')))
        .then(|| Self(value.to_owned()))
    }

    pub fn hash(&self) -> TokenHash {
        TokenHash(Sha256::digest(self.0.as_bytes()).into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionToken([REDACTED])")
    }
}

impl fmt::Debug for TokenHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TokenHash([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames_are_lowercased_and_restricted_to_a_safe_alphabet() {
        assert_eq!(
            Username::parse("Holy_Nekk.1-a").unwrap().as_str(),
            "holy_nekk.1-a"
        );
        for invalid in ["ab", "a b c", "çağrı", "user@home", &"x".repeat(33), ""] {
            assert_eq!(
                Username::parse(invalid),
                Err(CredentialError::InvalidUsername)
            );
        }
    }

    #[test]
    fn passwords_need_eight_to_128_characters_and_never_print() {
        assert!(Password::parse("1234567".into()).is_err());
        assert!(Password::parse("ğüşiöçĞÜ".into()).is_ok());
        assert!(Password::parse("x".repeat(129)).is_err());
        let password = Password::parse("correct horse".into()).unwrap();
        assert_eq!(format!("{password:?}"), "Password([REDACTED])");
    }

    #[test]
    fn login_tokens_are_random_hex_and_hash_deterministically() {
        let token = SessionToken::generate();
        assert_ne!(token.expose(), SessionToken::generate().expose());
        let parsed = SessionToken::parse(token.expose()).unwrap();
        assert!(parsed.hash() == token.hash());
        assert!(SessionToken::parse("ABC").is_none());
        assert!(SessionToken::parse(&"G".repeat(64)).is_none());
        assert!(!format!("{token:?}").contains(token.expose()));
    }
}
