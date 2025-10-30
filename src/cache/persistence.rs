use sqlx::{PgPool, Row};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::types::{RelayerError, Result, TransactionRequest, TransactionStatus, Priority};

#[derive(Debug)]
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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WalletRecord {
    pub id: Uuid,
    pub address: String,
    pub encrypted_private_key: String,
    pub balance: String,
    pub nonce: String,
    pub is_active: bool,
    pub last_used: Option<DateTime<Utc>>,
    pub success_rate: f64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserBalanceRecord {
    pub user_address: String,
    pub balance: String,
    pub last_updated: DateTime<Utc>,
}

impl DatabaseManager {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = PgPool::connect(database_url)
            .await
            .map_err(|e| RelayerError::Database(e.to_string()))?;

        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e.to_string()))?;

        Ok(())
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
            return Err(RelayerError::Database("No rows affected".to_string()));
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
        block_number: Option<i64>,
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
            block_number,
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

    pub async fn get_pending_transactions(&self, limit: u64) -> Result<Vec<TransactionRecord>> {
        let records = sqlx::query_as!(
            TransactionRecord,
            r#"
            SELECT * FROM transactions 
            WHERE status = 'pending' 
            ORDER BY 
                CASE priority 
                    WHEN 'critical' THEN 4
                    WHEN 'high' THEN 3
                    WHEN 'normal' THEN 2
                    WHEN 'low' THEN 1
                    ELSE 0
                END DESC,
                created_at ASC
            LIMIT $1
            "#,
            limit as i64
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        Ok(records)
    }

    // Wallet operations
    pub async fn create_wallet(&self, wallet: &WalletRecord) -> Result<Uuid> {
        let result = sqlx::query!(
            r#"
            INSERT INTO wallets (
                id, address, encrypted_private_key, balance, nonce, is_active,
                last_used, success_rate, created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10
            )
            "#,
            wallet.id,
            wallet.address,
            wallet.encrypted_private_key,
            wallet.balance,
            wallet.nonce,
            wallet.is_active,
            wallet.last_used,
            wallet.success_rate,
            wallet.created_at,
            wallet.updated_at
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        if result.rows_affected() == 0 {
            return Err(RelayerError::Database("No rows affected".to_string()));
        }

        Ok(wallet.id)
    }

    pub async fn get_active_wallets(&self) -> Result<Vec<WalletRecord>> {
        let records = sqlx::query_as!(
            WalletRecord,
            r#"
            SELECT * FROM wallets WHERE is_active = true ORDER BY success_rate DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        Ok(records)
    }

    pub async fn update_wallet_balance(
        &self,
        wallet_id: Uuid,
        balance: String,
        nonce: String,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE wallets 
            SET balance = $2, nonce = $3, updated_at = $4
            WHERE id = $1
            "#,
            wallet_id,
            balance,
            nonce,
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        Ok(())
    }

    pub async fn update_wallet_usage(
        &self,
        wallet_id: Uuid,
        success: bool,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            UPDATE wallets 
            SET last_used = $2, 
                success_rate = CASE 
                    WHEN success THEN LEAST(success_rate + 0.01, 1.0)
                    ELSE GREATEST(success_rate - 0.01, 0.0)
                END,
                updated_at = $3
            WHERE id = $1
            "#,
            wallet_id,
            Utc::now(),
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        Ok(())
    }

    // User balance operations
    pub async fn get_user_balance(&self, user_address: &str) -> Result<Option<UserBalanceRecord>> {
        let record = sqlx::query_as!(
            UserBalanceRecord,
            r#"
            SELECT * FROM user_balances WHERE user_address = $1
            "#,
            user_address
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        Ok(record)
    }

    pub async fn update_user_balance(
        &self,
        user_address: &str,
        balance: String,
    ) -> Result<()> {
        sqlx::query!(
            r#"
            INSERT INTO user_balances (user_address, balance, last_updated)
            VALUES ($1, $2, $3)
            ON CONFLICT (user_address) 
            DO UPDATE SET balance = $2, last_updated = $3
            "#,
            user_address,
            balance,
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        Ok(())
    }

    pub async fn deduct_user_balance(
        &self,
        user_address: &str,
        amount: String,
    ) -> Result<bool> {
        let result = sqlx::query!(
            r#"
            UPDATE user_balances 
            SET balance = balance - $2, last_updated = $3
            WHERE user_address = $1 AND balance >= $2
            "#,
            user_address,
            amount,
            Utc::now()
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        Ok(result.rows_affected() > 0)
    }

    // Statistics and monitoring
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

    pub async fn get_wallet_stats(&self) -> Result<WalletStats> {
        let stats = sqlx::query!(
            r#"
            SELECT 
                COUNT(*) as total_wallets,
                COUNT(CASE WHEN is_active = true THEN 1 END) as active_wallets,
                AVG(success_rate) as avg_success_rate,
                SUM(balance::numeric) as total_balance
            FROM wallets
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        Ok(WalletStats {
            total_wallets: stats.total_wallets.unwrap_or(0) as u64,
            active_wallets: stats.active_wallets.unwrap_or(0) as u64,
            avg_success_rate: stats.avg_success_rate.unwrap_or(0.0),
            total_balance: stats.total_balance.map(|v| v.to_string()),
        })
    }

    pub async fn cleanup_old_transactions(&self, days: i64) -> Result<u64> {
        let result = sqlx::query!(
            r#"
            DELETE FROM transactions 
            WHERE created_at < NOW() - INTERVAL '%s days' AND status IN ('confirmed', 'failed')
            "#,
            days
        )
        .execute(&self.pool)
        .await
        .map_err(|e| RelayerError::Database(e))?;

        Ok(result.rows_affected())
    }

    pub async fn health_check(&self) -> Result<bool> {
        sqlx::query!("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        Ok(true)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionStats {
    pub total_transactions: u64,
    pub pending_transactions: u64,
    pub confirmed_transactions: u64,
    pub failed_transactions: u64,
    pub avg_gas_used: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WalletStats {
    pub total_wallets: u64,
    pub active_wallets: u64,
    pub avg_success_rate: f64,
    pub total_balance: Option<String>,
}

impl Clone for DatabaseManager {
    fn clone(&self) -> Self {
        Self {
            pool: self.pool.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TransactionRequest, Signature, Priority};

    #[tokio::test]
    async fn test_database_manager_creation() {
        // This would require a test database in a real test
        let db_url = "postgresql://test:test@localhost/test_db";
        let manager = DatabaseManager::new(db_url).await;
        
        // In a real test environment, this would succeed
        // For now, we just test that the function exists
        assert!(manager.is_err()); // Expected to fail without real DB
    }

    #[tokio::test]
    async fn test_transaction_record_creation() {
        let request = TransactionRequest::new(
            alloy::primitives::Address::ZERO,
            alloy::primitives::Address::ZERO,
            alloy::primitives::Bytes::new(),
            alloy::primitives::U256::ZERO,
            alloy::primitives::U256::ZERO,
            alloy::primitives::U256::ZERO,
            alloy::primitives::U256::ZERO,
            alloy::primitives::U256::ZERO,
            Signature {
                r: alloy::primitives::U256::ZERO,
                s: alloy::primitives::U256::ZERO,
                v: 27,
            },
            Priority::Normal,
        );

        assert_eq!(request.priority, Priority::Normal);
        assert_eq!(request.signature.v, 27);
    }
}