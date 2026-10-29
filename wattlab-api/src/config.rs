//! Load local settings without mutating the process environment or logging secrets.

use crate::auth::ApiToken;
mod error;
pub use error::ConfigError;

use std::{collections::HashMap, env, net::SocketAddr, path::PathBuf};

#[derive(Debug, Clone)]
pub struct Config {
    pub address: SocketAddr,
    pub api_token: ApiToken,
}

pub(crate) fn environment_file() -> Result<HashMap<String, String>, ConfigError> {
    let explicit_path = env::var_os("WATTLAB_ENV_FILE");
    let path = explicit_path
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.env"));
    match dotenvy::from_path_iter(&path) {
        Ok(entries) => entries
            .collect::<Result<HashMap<_, _>, _>>()
            .map_err(|_| ConfigError::EnvironmentFile),
        Err(dotenvy::Error::Io(error))
            if error.kind() == std::io::ErrorKind::NotFound && explicit_path.is_none() =>
        {
            Ok(HashMap::new())
        }
        Err(_) => Err(ConfigError::EnvironmentFile),
    }
}

pub(crate) fn setting(
    file: &HashMap<String, String>,
    key: &str,
) -> Result<Option<String>, ConfigError> {
    match env::var(key) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(file.get(key).cloned()),
        Err(_) => Err(ConfigError::InvalidDatabase),
    }
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let file = environment_file()?;
        let mut process = HashMap::new();
        for (key, error) in [
            ("WATTLAB_API_PORT", ConfigError::InvalidPort),
            ("WATTLAB_API_TOKEN", ConfigError::InvalidToken),
        ] {
            match env::var(key) {
                Ok(value) => {
                    process.insert(key.to_owned(), value);
                }
                Err(env::VarError::NotPresent) => {}
                Err(_) => return Err(error),
            }
        }
        Self::from_sources(&file, &process)
    }

    fn from_sources(
        file: &HashMap<String, String>,
        process: &HashMap<String, String>,
    ) -> Result<Self, ConfigError> {
        let resolve = |key: &str| {
            process
                .get(key)
                .or_else(|| file.get(key))
                .map(String::as_str)
        };
        Self::from_values(resolve("WATTLAB_API_PORT"), resolve("WATTLAB_API_TOKEN"))
    }

    pub fn from_values(port: Option<&str>, token: Option<&str>) -> Result<Self, ConfigError> {
        let port = port
            .unwrap_or("8080")
            .parse::<u16>()
            .map_err(|_| ConfigError::InvalidPort)?;
        if port == 0 {
            return Err(ConfigError::InvalidPort);
        }
        let api_token = ApiToken::parse(token.ok_or(ConfigError::InvalidToken)?)
            .map_err(|_| ConfigError::InvalidToken)?;
        Ok(Self {
            address: SocketAddr::from(([127, 0, 0, 1], port)),
            api_token,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, ConfigError};
    use std::collections::HashMap;

    const TOKEN: &str = "test-only-token-for-integration-tests";

    #[test]
    fn valid_settings_cannot_expose_the_server_outside_loopback() {
        let config = Config::from_values(Some("9091"), Some(TOKEN)).unwrap();
        assert!(config.address.ip().is_loopback());
        assert_eq!(config.address.port(), 9091);
    }

    #[test]
    fn invalid_ports_fail_instead_of_silently_using_a_default() {
        for port in ["", "0", "-1", "65536", "eight", "127.0.0.1:8081"] {
            assert_eq!(
                Config::from_values(Some(port), Some(TOKEN)).unwrap_err(),
                ConfigError::InvalidPort
            );
        }
    }

    #[test]
    fn missing_empty_or_invalid_tokens_prevent_startup() {
        for token in [
            None,
            Some(""),
            Some("short"),
            Some("has spaces but at least thirty two characters"),
            Some("éééééééééééééééééé"),
            Some("xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
        ] {
            assert_eq!(
                Config::from_values(None, token).unwrap_err(),
                ConfigError::InvalidToken
            );
        }
    }

    #[test]
    fn process_settings_override_file_settings_including_invalid_empty_values() {
        let file = HashMap::from([
            ("WATTLAB_API_TOKEN".into(), TOKEN.into()),
            ("WATTLAB_API_PORT".into(), "9001".into()),
        ]);
        let process = HashMap::from([("WATTLAB_API_PORT".into(), "9002".into())]);
        assert_eq!(
            Config::from_sources(&file, &process)
                .unwrap()
                .address
                .port(),
            9002
        );
        let process = HashMap::from([("WATTLAB_API_TOKEN".into(), "".into())]);
        assert_eq!(
            Config::from_sources(&file, &process).unwrap_err(),
            ConfigError::InvalidToken
        );
    }

    #[test]
    fn debug_output_never_contains_token() {
        let config = Config::from_values(None, Some(TOKEN)).unwrap();
        assert!(!format!("{config:?}").contains(TOKEN));
        assert_eq!(config.address.port(), 8080);
    }
}
