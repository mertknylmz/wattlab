//! Pool setup and embedded migrations. Never resets or replays the Java schema.

use super::{DatabaseConfig, startup_error::DatabaseError};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;

pub async fn connect_and_migrate(config: &DatabaseConfig) -> Result<PgPool, DatabaseError> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(3))
        .after_connect(|connection, _| {
            Box::pin(async move {
                sqlx::query("SET statement_timeout = '5s'")
                    .execute(&mut *connection)
                    .await?;
                sqlx::query("SET lock_timeout = '3s'")
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect_with(config.options())
        .await
        .map_err(|_| DatabaseError::Unavailable)?;
    let legacy: bool =
        sqlx::query_scalar("SELECT to_regclass('public.flyway_schema_history') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .map_err(|_| DatabaseError::Unavailable)?;
    if legacy {
        pool.close().await;
        return Err(DatabaseError::LegacySchema);
    }
    if sqlx::migrate!("./migrations").run(&pool).await.is_err() {
        pool.close().await;
        return Err(DatabaseError::Migration);
    }
    Ok(pool)
}
