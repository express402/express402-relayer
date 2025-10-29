use sqlx::{PgPool, postgres::PgPoolOptions};
use std::time::Duration;
use tokio::fs;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::types::{RelayerError, Result, Config, TransactionRequest, TransactionStatus};

pub struct DatabaseManager {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TransactionRecord {
    pub id: Uuid,
    pub user_address: String,
    pub target_contract: String,
    pub calldata: Vec<u8>,
    pub value: String,
    pub gas_limit: String,
    pub max_fee_per_gas: String,
    pub max_priority_fee_per_gas: String,
    pub nonce: String,
    pub signature_r: String,
    pub signature_s: String,
    pub signature_v: u8,
    pub priority: String,
    pub status: String,
    pub tx_hash: Option<String>,
    pub block_number: Option<i64>,
    pub gas_used: Option<String>,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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

    // Transaction operations
    pub async fn create_transaction(&self, request: &TransactionRequest) -> Result<Uuid> {
        let result = sqlx::query!(
            r#"
            INSERT INTO transactions (
                id, user_address, target_contract, calldata, value, gas_limit,
                max_fee_per_gas, max_priority_fee_per_gas, nonce,
                signature_r, signature_s, signature_v, priority, status,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16
            )
            "#,
            request.id,
            request.user_address.to_string(),
            request.target_contract.to_string(),
            request.calldata.as_ref(),
            request.value.to_string(),
            request.gas_limit.to_string(),
            request.max_fee_per_gas.to_string(),
            request.max_priority_fee_per_gas.to_string(),
            request.nonce.to_string(),
            request.signature.r.to_string(),
            request.signature.s.to_string(),
            request.signature.v,
            request.priority.to_string(),
            "pending",
            request.timestamp,
            request.timestamp
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        if result.rows_affected() == 0 {
            return Err(RelayerError::Database(sqlx::Error::RowNotFound));
        }

        Ok(request.id)
    }

    pub async fn get_transaction(&self, transaction_id: Uuid) -> Result<Option<TransactionRecord>> {
        let record = sqlx::query_as!(
            TransactionRecord,
            r#"
            SELECT * FROM transactions WHERE id = $1
            "#,
            transaction_id
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        Ok(record)
    }

    pub async fn update_transaction_status(
        &self,
        transaction_id: Uuid,
        status: TransactionStatus,
        tx_hash: Option<String>,
        block_number: Option<u64>,
        gas_used: Option<String>,
        error_message: Option<String>,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE transactions 
            SET status = $2, tx_hash = $3, block_number = $4, gas_used = $5, 
                error_message = $6, updated_at = $7
            WHERE id = $1
            "#,
            transaction_id,
            status.to_string(),
            tx_hash,
            block_number.map(|n| n as i64),
            gas_used,
            error_message,
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        Ok(())
    }

    pub async fn get_user_transactions(
        &self,
        user_address: &str,
        page: u64,
        limit: u64,
    ) -> Result<(Vec<TransactionRecord>, u64)> {
        let offset = (page - 1) * limit;

        let records = sqlx::query_as!(
            TransactionRecord,
            r#"
            SELECT * FROM transactions 
            WHERE user_address = $1 
            ORDER BY created_at DESC 
            LIMIT $2 OFFSET $3
            "#,
            user_address,
            limit as i64,
            offset as i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        let total = sqlx::query!(
            r#"
            SELECT COUNT(*) as count FROM transactions WHERE user_address = $1
            "#,
            user_address
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?
        .count
        .unwrap_or(0) as u64;

        Ok((records, total))
    }

    pub async fn get_transaction_stats(&self) -> Result<TransactionStats> {
        let stats = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as total_transactions,
                COUNT(CASE WHEN status = 'pending' THEN 1 END) as pending_transactions,
                COUNT(CASE WHEN status = 'confirmed' THEN 1 END) as confirmed_transactions,
                COUNT(CASE WHEN status = 'failed' THEN 1 END) as failed_transactions,
                AVG(CASE WHEN gas_used IS NOT NULL THEN gas_used::numeric END) as avg_gas_used
            FROM transactions
            WHERE created_at >= NOW() - INTERVAL '24 hours'
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        Ok(TransactionStats {
            total_transactions: stats.total_transactions.unwrap_or(0) as u64,
            pending_transactions: stats.pending_transactions.unwrap_or(0) as u64,
            confirmed_transactions: stats.confirmed_transactions.unwrap_or(0) as u64,
            failed_transactions: stats.failed_transactions.unwrap_or(0) as u64,
            avg_gas_used: stats.avg_gas_used.map(|v| v.to_string()),
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

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionStats {
    pub total_transactions: u64,
    pub pending_transactions: u64,
    pub confirmed_transactions: u64,
    pub failed_transactions: u64,
    pub avg_gas_used: Option<String>,
}

impl Clone for DatabaseManager {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}
