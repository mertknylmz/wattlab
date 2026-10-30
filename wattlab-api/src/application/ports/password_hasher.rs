use crate::domain::member::Password;
use std::{future::Future, pin::Pin};

#[derive(Debug)]
pub struct HashError;

pub type HashFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

/// Hashing is CPU-bound; adapters must keep it off the async executor.
pub trait PasswordHasher: Send + Sync {
    fn hash(&self, password: Password) -> HashFuture<Result<String, HashError>>;
    /// False for a wrong password and for an unreadable stored hash.
    fn verify(&self, password: Password, hash: String) -> HashFuture<bool>;
}
