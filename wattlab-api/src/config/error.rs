use std::{error::Error, fmt};

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    EnvironmentFile,
    InvalidPort,
    InvalidToken,
    InvalidDatabase,
    UnsafeTestDatabase,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EnvironmentFile => "Cannot read or parse the configured environment file",
            Self::InvalidPort => "WATTLAB_API_PORT must be an integer from 1 to 65535",
            Self::InvalidToken => "WATTLAB_API_TOKEN must contain 32 to 64 URL-safe characters",
            Self::InvalidDatabase => "Valid PostgreSQL URL, user and password settings are required; run scripts/local_db.py start",
            Self::UnsafeTestDatabase => "Integration tests require a dedicated database ending in _rust_test",
        })
    }
}

impl Error for ConfigError {}
