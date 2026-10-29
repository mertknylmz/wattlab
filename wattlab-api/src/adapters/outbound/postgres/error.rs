use crate::application::ports::repository_error::RepositoryError;

pub(super) fn repository_error(error: sqlx::Error) -> RepositoryError {
    // Driver details stay in this adapter; log only the non-sensitive SQLSTATE code.
    tracing::warn!(code = ?error.as_database_error().and_then(|error| error.code()), "Session storage failure");
    match error {
        sqlx::Error::PoolClosed
        | sqlx::Error::PoolTimedOut
        | sqlx::Error::Io(_)
        | sqlx::Error::Tls(_) => RepositoryError::Unavailable,
        _ => RepositoryError::Unexpected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driver_errors_are_classified_without_leaking_into_the_port() {
        assert_eq!(
            repository_error(sqlx::Error::PoolClosed),
            RepositoryError::Unavailable
        );
        assert_eq!(
            repository_error(sqlx::Error::RowNotFound),
            RepositoryError::Unexpected
        );
    }
}
