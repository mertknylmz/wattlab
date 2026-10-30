use crate::{
    application::ports::password_hasher::{HashError, HashFuture, PasswordHasher},
    domain::member::Password,
};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier, SaltString},
};
use rand_core::OsRng;

/// Argon2id with the crate's recommended default parameters, stored as a PHC
/// string. Runs on Tokio's blocking pool.
#[derive(Default)]
pub struct Argon2Hasher;

fn hash_now(password: &Password) -> Result<String, HashError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.expose().as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| HashError)
}

fn verify_now(password: &Password, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|parsed| {
        Argon2::default()
            .verify_password(password.expose().as_bytes(), &parsed)
            .is_ok()
    })
}

impl PasswordHasher for Argon2Hasher {
    fn hash(&self, password: Password) -> HashFuture<Result<String, HashError>> {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || hash_now(&password))
                .await
                .unwrap_or(Err(HashError))
        })
    }

    fn verify(&self, password: Password, hash: String) -> HashFuture<bool> {
        Box::pin(async move {
            tokio::task::spawn_blocking(move || verify_now(&password, &hash))
                .await
                .unwrap_or(false)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hashes_are_salted_argon2id_and_verify_only_the_right_password() {
        let password = Password::parse("correct horse".into()).unwrap();
        let hasher = Argon2Hasher;
        let hash = hasher.hash(password.clone()).await.unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert_ne!(hash, hasher.hash(password.clone()).await.unwrap());
        assert!(hasher.verify(password.clone(), hash.clone()).await);
        let wrong = Password::parse("wrong horse".into()).unwrap();
        assert!(!hasher.verify(wrong, hash).await);
        assert!(!hasher.verify(password, "not a hash".into()).await);
    }
}
