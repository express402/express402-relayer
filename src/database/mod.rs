use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use tokio::fs;

use crate::types::{RelayerError, Result, Config};

pub struct DatabaseManager {
    pool: PgPool,
}

impl DatabaseManager {
    pub async fn new(config: &Config) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.database.max_connections)
            .min_connections(config.database.min_connections)
            .acquire_timeout(Duration::from_secs(config.database.connection_timeout))
            .idle_timeout(Duration::from_secs(config.database.idle_timeout))
            .connect(&config.database.url)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        tracing::info!("Running database migrations...");
        
        // Read migration files
        let migration_files = vec![
            "migrations/001_initial_schema.sql",
        ];

        for migration_file in migration_files {
            let migration_sql = fs::read_to_string(migration_file)
                .await
                .map_err(|e| RelayerError::Io(e))?;

            sqlx::query(&migration_sql)
                .execute(&self.pool)
                .await
                .map_err(|e| RelayerError::Database(e))?;

            tracing::info!("Applied migration: {}", migration_file);
        }

        tracing::info!("Database migrations completed successfully");
        Ok(())
    }

    pub async fn check_connection(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;
        
        Ok(())
    }

    pub fn get_pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn get_stats(&self) -> Result<DatabaseStats> {
        let connection_count = self.pool.size();
        let idle_connections = self.pool.num_idle();

        // Get table statistics
        let transaction_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM transactions")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        let wallet_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM wallets")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        Ok(DatabaseStats {
            connection_count,
            idle_connections,
            active_connections: connection_count - idle_connections,
            transaction_count: transaction_count.0 as u64,
            wallet_count: wallet_count.0 as u64,
        })
    }
}

#[derive(Debug, serde::Serialize)]
pub struct DatabaseStats {
    pub connection_count: u32,
    pub idle_connections: u32,
    pub active_connections: u32,
    pub transaction_count: u64,
    pub wallet_count: u64,
}

impl Clone for DatabaseManager {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}
