use crate::config::{ConfigError, environment_file, setting};
use sqlx::{ConnectOptions, postgres::PgConnectOptions};
use std::{fmt, str::FromStr};

#[derive(Clone)]
pub struct DatabaseConfig {
    options: PgConnectOptions,
}

impl fmt::Debug for DatabaseConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DatabaseConfig([REDACTED])")
    }
}

impl DatabaseConfig {
    pub fn load() -> Result<Self, ConfigError> {
        Self::load_prefix("WATTLAB_DB")
    }

    /// Dedicated opt-in integration-test configuration; never falls back to the dev database.
    pub fn load_test() -> Result<Self, ConfigError> {
        Self::load_prefix("WATTLAB_TEST_DB")?.ensure_test_database()
    }

    fn ensure_test_database(self) -> Result<Self, ConfigError> {
        if !self
            .options
            .get_database()
            .is_some_and(|name| name.ends_with("_rust_test"))
        {
            return Err(ConfigError::UnsafeTestDatabase);
        }
        Ok(self)
    }

    fn load_prefix(prefix: &str) -> Result<Self, ConfigError> {
        let file = environment_file()?;
        let required = |suffix: &str| -> Result<String, ConfigError> {
            setting(&file, &format!("{prefix}_{suffix}"))?
                .filter(|value| !value.is_empty())
                .ok_or(ConfigError::InvalidDatabase)
        };
        Self::new(
            &required("URL")?,
            &required("USER")?,
            &required("PASSWORD")?,
        )
    }

    fn new(url: &str, user: &str, password: &str) -> Result<Self, ConfigError> {
        if !(url.starts_with("postgresql://") || url.starts_with("postgres://"))
            || user.is_empty()
            || password.is_empty()
        {
            return Err(ConfigError::InvalidDatabase);
        }
        let options = PgConnectOptions::from_str(url)
            .map_err(|_| ConfigError::InvalidDatabase)?
            .username(user)
            .password(password)
            .disable_statement_logging();
        Ok(Self { options })
    }

    pub(crate) fn options(&self) -> PgConnectOptions {
        self.options.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_jdbc_urls_are_rejected_and_debug_output_is_redacted() {
        assert!(matches!(
            DatabaseConfig::new(
                "jdbc:postgresql://localhost/old",
                "user",
                "example-password"
            ),
            Err(ConfigError::InvalidDatabase)
        ));
        let config = DatabaseConfig::new(
            "postgresql://localhost/wattlab_rust_dev",
            "user",
            "example-password",
        )
        .unwrap();
        assert_eq!(format!("{config:?}"), "DatabaseConfig([REDACTED])");
    }

    #[test]
    fn integration_guard_rejects_dev_and_legacy_test_databases() {
        for name in ["wattlab_rust_dev", "wattlab_api_test", "production"] {
            let config = DatabaseConfig::new(
                &format!("postgresql://localhost/{name}"),
                "user",
                "example-password",
            )
            .unwrap();
            assert!(matches!(
                config.ensure_test_database(),
                Err(ConfigError::UnsafeTestDatabase)
            ));
        }
        let config = DatabaseConfig::new(
            "postgresql://localhost/wattlab_rust_test",
            "user",
            "example-password",
        )
        .unwrap();
        assert!(config.ensure_test_database().is_ok());
    }
}
