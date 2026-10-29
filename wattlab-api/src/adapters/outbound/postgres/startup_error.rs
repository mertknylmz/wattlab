use std::{error::Error, fmt};

#[derive(Debug)]
pub enum DatabaseError {
    Unavailable,
    LegacySchema,
    Migration,
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unavailable => {
                "PostgreSQL connection unavailable; check local database setup and credentials"
            }
            Self::LegacySchema => "Flyway schema detected; use a separate Rust database",
            Self::Migration => "Rust database migration failed; existing data was not reset",
        })
    }
}
impl Error for DatabaseError {}
