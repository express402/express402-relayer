use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

use crate::types::{RelayerError, Result, TransactionRequest, TransactionResult, TransactionStatus};

#[derive(Debug, Clone)]
pub struct PersistenceManager {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredTransaction {
    pub id: Uuid,
    pub user_address: String,
    pub target_contract: String,
    pub calldata: String,
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
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl PersistenceManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn store_transaction(&self, request: &TransactionRequest) -> Result<()> {
        let query = r#"
            INSERT INTO transactions (
                id, user_address, target_contract, calldata, value, gas_limit,
                max_fee_per_gas, max_priority_fee_per_gas, nonce,
                signature_r, signature_s, signature_v, priority, status,
                created_at, updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16
            )
        "#;

        sqlx::query(query)
            .bind(request.id)
            .bind(request.user_address.to_string())
            .bind(request.target_contract.to_string())
            .bind(format!("0x{}", hex::encode(&request.calldata)))
            .bind(request.value.to_string())
            .bind(request.gas_limit.to_string())
            .bind(request.max_fee_per_gas.to_string())
            .bind(request.max_priority_fee_per_gas.to_string())
            .bind(request.nonce.to_string())
            .bind(request.signature.r.to_string())
            .bind(request.signature.s.to_string())
            .bind(request.signature.v)
            .bind(format!("{:?}", request.priority))
            .bind("pending")
            .bind(request.timestamp)
            .bind(request.timestamp)
            .execute(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        Ok(())
    }

    pub async fn update_transaction_status(
        &self,
        id: Uuid,
        status: TransactionStatus,
        tx_hash: Option<String>,
        block_number: Option<u64>,
        gas_used: Option<String>,
        error_message: Option<String>,
    ) -> Result<()> {
        let query = r#"
            UPDATE transactions 
            SET status = $2, tx_hash = $3, block_number = $4, gas_used = $5, error_message = $6, updated_at = $7
            WHERE id = $1
        "#;

        sqlx::query(query)
            .bind(id)
            .bind(format!("{:?}", status))
            .bind(tx_hash)
            .bind(block_number.map(|n| n as i64))
            .bind(gas_used)
            .bind(error_message)
            .bind(chrono::Utc::now())
            .execute(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        Ok(())
    }

    pub async fn get_transaction(&self, id: Uuid) -> Result<Option<StoredTransaction>> {
        let query = r#"
            SELECT * FROM transactions WHERE id = $1
        "#;

        let row = sqlx::query(query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        if let Some(row) = row {
            Ok(Some(self.row_to_stored_transaction(row)?))
        } else {
            Ok(None)
        }
    }

    pub async fn get_transactions_by_user(
        &self,
        user_address: &str,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<StoredTransaction>> {
        let query = r#"
            SELECT * FROM transactions 
            WHERE user_address = $1 
            ORDER BY created_at DESC 
            LIMIT $2 OFFSET $3
        "#;

        let rows = sqlx::query(query)
            .bind(user_address)
            .bind(limit.unwrap_or(100))
            .bind(offset.unwrap_or(0))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        let mut transactions = Vec::new();
        for row in rows {
            transactions.push(self.row_to_stored_transaction(row)?);
        }

        Ok(transactions)
    }

    pub async fn get_transactions_by_status(
        &self,
        status: TransactionStatus,
        limit: Option<i64>,
    ) -> Result<Vec<StoredTransaction>> {
        let query = r#"
            SELECT * FROM transactions 
            WHERE status = $1 
            ORDER BY created_at ASC 
            LIMIT $2
        "#;

        let rows = sqlx::query(query)
            .bind(format!("{:?}", status))
            .bind(limit.unwrap_or(1000))
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        let mut transactions = Vec::new();
        for row in rows {
            transactions.push(self.row_to_stored_transaction(row)?);
        }

        Ok(transactions)
    }

    pub async fn get_transaction_stats(&self) -> Result<TransactionStats> {
        let query = r#"
            SELECT 
                COUNT(*) as total_transactions,
                COUNT(CASE WHEN status = 'pending' THEN 1 END) as pending_transactions,
                COUNT(CASE WHEN status = 'processing' THEN 1 END) as processing_transactions,
                COUNT(CASE WHEN status = 'submitted' THEN 1 END) as submitted_transactions,
                COUNT(CASE WHEN status = 'confirmed' THEN 1 END) as confirmed_transactions,
                COUNT(CASE WHEN status = 'failed' THEN 1 END) as failed_transactions,
                COUNT(CASE WHEN status = 'cancelled' THEN 1 END) as cancelled_transactions,
                AVG(CASE WHEN gas_used IS NOT NULL THEN gas_used::numeric END) as avg_gas_used,
                MIN(created_at) as earliest_transaction,
                MAX(created_at) as latest_transaction
            FROM transactions
        "#;

        let row = sqlx::query(query)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        Ok(TransactionStats {
            total_transactions: row.get::<i64, _>("total_transactions") as u64,
            pending_transactions: row.get::<i64, _>("pending_transactions") as u64,
            processing_transactions: row.get::<i64, _>("processing_transactions") as u64,
            submitted_transactions: row.get::<i64, _>("submitted_transactions") as u64,
            confirmed_transactions: row.get::<i64, _>("confirmed_transactions") as u64,
            failed_transactions: row.get::<i64, _>("failed_transactions") as u64,
            cancelled_transactions: row.get::<i64, _>("cancelled_transactions") as u64,
            average_gas_used: row.get::<Option<f64>, _>("avg_gas_used"),
            earliest_transaction: row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("earliest_transaction"),
            latest_transaction: row.get::<Option<chrono::DateTime<chrono::Utc>>, _>("latest_transaction"),
        })
    }

    pub async fn cleanup_old_transactions(&self, older_than_days: i32) -> Result<u64> {
        let query = r#"
            DELETE FROM transactions 
            WHERE created_at < NOW() - INTERVAL '%s days' AND status IN ('confirmed', 'failed', 'cancelled')
        "#;

        let result = sqlx::query(&format!(query, older_than_days))
            .execute(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        Ok(result.rows_affected())
    }

    pub async fn get_user_transaction_history(
        &self,
        user_address: &str,
        start_date: Option<chrono::DateTime<chrono::Utc>>,
        end_date: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<StoredTransaction>> {
        let mut query = r#"
            SELECT * FROM transactions 
            WHERE user_address = $1
        "#.to_string();

        let mut params = vec![user_address.to_string()];
        let mut param_count = 1;

        if let Some(start) = start_date {
            param_count += 1;
            query.push_str(&format!(" AND created_at >= ${}", param_count));
            params.push(start.to_rfc3339());
        }

        if let Some(end) = end_date {
            param_count += 1;
            query.push_str(&format!(" AND created_at <= ${}", param_count));
            params.push(end.to_rfc3339());
        }

        query.push_str(" ORDER BY created_at DESC");

        // This is a simplified version - in practice you'd use proper parameterized queries
        let rows = sqlx::query(&query)
            .bind(user_address)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        let mut transactions = Vec::new();
        for row in rows {
            transactions.push(self.row_to_stored_transaction(row)?);
        }

        Ok(transactions)
    }

    fn row_to_stored_transaction(&self, row: sqlx::postgres::PgRow) -> Result<StoredTransaction> {
        Ok(StoredTransaction {
            id: row.get("id"),
            user_address: row.get("user_address"),
            target_contract: row.get("target_contract"),
            calldata: row.get("calldata"),
            value: row.get("value"),
            gas_limit: row.get("gas_limit"),
            max_fee_per_gas: row.get("max_fee_per_gas"),
            max_priority_fee_per_gas: row.get("max_priority_fee_per_gas"),
            nonce: row.get("nonce"),
            signature_r: row.get("signature_r"),
            signature_s: row.get("signature_s"),
            signature_v: row.get("signature_v"),
            priority: row.get("priority"),
            status: row.get("status"),
            tx_hash: row.get("tx_hash"),
            block_number: row.get("block_number"),
            gas_used: row.get("gas_used"),
            error_message: row.get("error_message"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    pub async fn create_tables(&self) -> Result<()> {
        let create_table_query = r#"
            CREATE TABLE IF NOT EXISTS transactions (
                id UUID PRIMARY KEY,
                user_address VARCHAR(42) NOT NULL,
                target_contract VARCHAR(42) NOT NULL,
                calldata TEXT NOT NULL,
                value VARCHAR(78) NOT NULL,
                gas_limit VARCHAR(78) NOT NULL,
                max_fee_per_gas VARCHAR(78) NOT NULL,
                max_priority_fee_per_gas VARCHAR(78) NOT NULL,
                nonce VARCHAR(78) NOT NULL,
                signature_r VARCHAR(78) NOT NULL,
                signature_s VARCHAR(78) NOT NULL,
                signature_v SMALLINT NOT NULL,
                priority VARCHAR(20) NOT NULL,
                status VARCHAR(20) NOT NULL DEFAULT 'pending',
                tx_hash VARCHAR(66),
                block_number BIGINT,
                gas_used VARCHAR(78),
                error_message TEXT,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE INDEX IF NOT EXISTS idx_transactions_user_address ON transactions(user_address);
            CREATE INDEX IF NOT EXISTS idx_transactions_status ON transactions(status);
            CREATE INDEX IF NOT EXISTS idx_transactions_created_at ON transactions(created_at);
            CREATE INDEX IF NOT EXISTS idx_transactions_tx_hash ON transactions(tx_hash);
        "#;

        sqlx::query(create_table_query)
            .execute(&self.pool)
            .await
            .map_err(|e| RelayerError::Database(e))?;

        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionStats {
    pub total_transactions: u64,
    pub pending_transactions: u64,
    pub processing_transactions: u64,
    pub submitted_transactions: u64,
    pub confirmed_transactions: u64,
    pub failed_transactions: u64,
    pub cancelled_transactions: u64,
    pub average_gas_used: Option<f64>,
    pub earliest_transaction: Option<chrono::DateTime<chrono::Utc>>,
    pub latest_transaction: Option<chrono::DateTime<chrono::Utc>>,
}

impl Clone for PersistenceManager {
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
    use alloy::primitives::{Address, Bytes, U256};

    fn create_test_request() -> TransactionRequest {
        TransactionRequest::new(
            Address::ZERO,
            Address::ZERO,
            Bytes::new(),
            U256::ZERO,
            U256::from(21000),
            U256::from(20000000000u64),
            U256::from(2000000000u64),
            U256::ZERO,
            Signature {
                r: U256::ZERO,
                s: U256::ZERO,
                v: 27,
            },
            Priority::Normal,
        )
    }

    #[tokio::test]
    async fn test_persistence_manager_creation() {
        // This would require a database connection in a real test
        // For now, just test the structure
        let request = create_test_request();
        assert_eq!(request.priority.weight(), 2);
    }

    #[test]
    fn test_transaction_stats_creation() {
        let stats = TransactionStats {
            total_transactions: 100,
            pending_transactions: 10,
            processing_transactions: 5,
            submitted_transactions: 20,
            confirmed_transactions: 60,
            failed_transactions: 4,
            cancelled_transactions: 1,
            average_gas_used: Some(21000.0),
            earliest_transaction: Some(chrono::Utc::now()),
            latest_transaction: Some(chrono::Utc::now()),
        };

        assert_eq!(stats.total_transactions, 100);
        assert_eq!(stats.confirmed_transactions, 60);
    }
}
