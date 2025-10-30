use redis::{Client, Connection, RedisResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

use crate::types::{RelayerError, Result};

#[derive(Debug)]
pub struct RedisCache {
    client: Client,
    connection: Arc<RwLock<Option<Connection>>>,
    key_prefix: String,
    default_ttl: Duration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry<T> {
    pub value: T,
    pub created_at: Instant,
    pub expires_at: Option<Instant>,
    pub access_count: u64,
    pub last_accessed: Instant,
}

impl<T> CacheEntry<T> {
    pub fn new(value: T, ttl: Option<Duration>) -> Self {
        let now = Instant::now();
        Self {
            value,
            created_at: now,
            expires_at: ttl.map(|ttl| now + ttl),
            access_count: 0,
            last_accessed: now,
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            Instant::now() > expires_at
        } else {
            false
        }
    }

    pub fn access(&mut self) {
        self.access_count += 1;
        self.last_accessed = Instant::now();
    }
}

impl RedisCache {
    pub fn new(url: &str, key_prefix: String, default_ttl: Duration) -> Result<Self> {
        let client = Client::open(url)?;

        Ok(Self {
            client,
            connection: Arc::new(RwLock::new(None)),
            key_prefix,
            default_ttl,
        })
    }

    pub async fn connect(&self) -> Result<()> {
        let connection = self.client.get_connection()?;

        let mut conn = self.connection.write().await;
        *conn = Some(connection);

        Ok(())
    }

    pub async fn get<T>(&self, key: &str) -> Result<Option<T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut conn = self.connection.write().await;
        let conn = conn.as_mut()
            .ok_or_else(|| RelayerError::Cache("Not connected to Redis".to_string()))?;

        let full_key = format!("{}{}", self.key_prefix, key);
        let result: RedisResult<String> = redis::cmd("GET").arg(&full_key).query(conn);

        match result {
            Ok(data) => {
                let value: T = serde_json::from_str(&data)?;
                Ok(Some(value))
            }
            Err(_) => {
                // Key not found or other error
                Ok(None)
            }
        }
    }

    pub async fn set<T>(&self, key: &str, value: &T, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize,
    {
        let mut conn = self.connection.write().await;
        let conn = conn.as_mut()
            .ok_or_else(|| RelayerError::Cache("Not connected to Redis".to_string()))?;

        let full_key = format!("{}{}", self.key_prefix, key);
        let data = serde_json::to_string(value)?;

        let ttl_seconds = ttl.unwrap_or(self.default_ttl).as_secs() as i64;

        redis::cmd("SETEX")
            .arg(&full_key)
            .arg(ttl_seconds)
            .arg(&data)
            .execute(conn);

        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<bool> {
        let mut conn = self.connection.write().await;
        let conn = conn.as_mut()
            .ok_or_else(|| RelayerError::Cache("Not connected to Redis".to_string()))?;

        let full_key = format!("{}{}", self.key_prefix, key);
        let result: RedisResult<i32> = redis::cmd("DEL").arg(&full_key).query(conn);

        match result {
            Ok(count) => Ok(count > 0),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.connection.write().await;
        let conn = conn.as_mut()
            .ok_or_else(|| RelayerError::Cache("Not connected to Redis".to_string()))?;

        let full_key = format!("{}{}", self.key_prefix, key);
        let result: RedisResult<i32> = redis::cmd("EXISTS").arg(&full_key).query(conn);

        match result {
            Ok(count) => Ok(count > 0),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn expire(&self, key: &str, ttl: Duration) -> Result<bool> {
        let mut conn = self.connection.write().await;
        let conn = conn.as_mut()
            .ok_or_else(|| RelayerError::Cache("Not connected to Redis".to_string()))?;

        let full_key = format!("{}{}", self.key_prefix, key);
        let result: RedisResult<i32> = redis::cmd("EXPIRE")
            .arg(&full_key)
            .arg(ttl.as_secs() as i64)
            .query(conn);

        match result {
            Ok(count) => Ok(count > 0),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn get_ttl(&self, key: &str) -> Result<Option<Duration>> {
        let mut conn = self.connection.write().await;
        let conn = conn.as_mut()
            .ok_or_else(|| RelayerError::Cache("Not connected to Redis".to_string()))?;

        let full_key = format!("{}{}", self.key_prefix, key);
        let result: RedisResult<i64> = redis::cmd("TTL").arg(&full_key).query(conn);

        match result {
            Ok(ttl) => {
                if ttl == -1 {
                    Ok(None) // No expiration
                } else if ttl == -2 {
                    Ok(None) // Key doesn't exist
                } else {
                    Ok(Some(Duration::from_secs(ttl as u64)))
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    pub async fn get_multiple<T>(&self, keys: &[String]) -> Result<HashMap<String, T>>
    where
        T: for<'de> Deserialize<'de>,
    {
        let mut conn = self.connection.write().await;
        let conn = conn.as_mut()
            .ok_or_else(|| RelayerError::Cache("Not connected to Redis".to_string()))?;

        let full_keys: Vec<String> = keys.iter()
            .map(|key| format!("{}{}", self.key_prefix, key))
            .collect();

        let result: RedisResult<Vec<String>> = redis::cmd("MGET")
            .arg(&full_keys)
            .query(conn);

        match result {
            Ok(values) => {
                let mut map = HashMap::new();
                for (i, value) in values.into_iter().enumerate() {
                    if let Some(key) = keys.get(i) {
                        if !value.is_empty() {
                            let deserialized: T = serde_json::from_str(&value)
                                .map_err(|e| RelayerError::Serialization(e.to_string()))?;
                            map.insert(key.clone(), deserialized);
                        }
                    }
                }
                Ok(map)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub async fn set_multiple<T>(&self, items: HashMap<String, T>, ttl: Option<Duration>) -> Result<()>
    where
        T: Serialize,
    {
        let mut conn = self.connection.write().await;
        let conn = conn.as_mut()
            .ok_or_else(|| RelayerError::Cache("Not connected to Redis".to_string()))?;

        let mut pipe = redis::pipe();
        
        for (key, value) in items {
            let full_key = format!("{}{}", self.key_prefix, key);
            let data = serde_json::to_string(&value)
                .map_err(|e| RelayerError::Serialization(e.to_string()))?;

            if let Some(ttl) = ttl {
                pipe.cmd("SETEX").arg(&full_key).arg(ttl.as_secs() as i64).arg(&data);
            } else {
                pipe.set(&full_key, &data);
            }
        }

        pipe.execute(conn);
        Ok(())
    }

    pub async fn clear_pattern(&self, pattern: &str) -> Result<usize> {
        let mut conn = self.connection.write().await;
        let conn = conn.as_mut()
            .ok_or_else(|| RelayerError::Cache("Not connected to Redis".to_string()))?;

        let full_pattern = format!("{}{}", self.key_prefix, pattern);
        let result: RedisResult<Vec<String>> = redis::cmd("KEYS").arg(&full_pattern).query(conn);

        match result {
            Ok(keys) => {
                if !keys.is_empty() {
                    let result: RedisResult<i32> = redis::cmd("DEL").arg(&keys).query(conn);
                    match result {
                        Ok(count) => Ok(count as usize),
                        Err(e) => Err(e.into()),
                    }
                } else {
                    Ok(0)
                }
            }
            Err(e) => Err(e.into()),
        }
    }

    pub async fn get_stats(&self) -> Result<RedisCacheStats> {
        let mut conn = self.connection.write().await;
        let conn = conn.as_mut()
            .ok_or_else(|| RelayerError::Cache("Not connected to Redis".to_string()))?;

        let info: RedisResult<String> = redis::cmd("INFO").arg("memory").query(conn);
        
        match info {
            Ok(info_str) => {
                let mut stats = RedisCacheStats {
                    connected: true,
                    key_prefix: self.key_prefix.clone(),
                    default_ttl_seconds: self.default_ttl.as_secs(),
                    memory_usage: 0,
                    key_count: 0,
                    hit_rate: 0.0,
                };

                // Parse memory info (simplified)
                for line in info_str.lines() {
                    if line.starts_with("used_memory:") {
                        if let Some(value) = line.split(':').nth(1) {
                            stats.memory_usage = value.parse().unwrap_or(0);
                        }
                    }
                }

                // Get key count for our prefix
                let pattern = format!("{}*", self.key_prefix);
                let keys_result: RedisResult<Vec<String>> = redis::cmd("KEYS").arg(&pattern).query(conn);
                if let Ok(keys) = keys_result {
                    stats.key_count = keys.len();
                }

                Ok(stats)
            }
            Err(e) => Err(e.into()),
        }
    }

    pub async fn ping(&self) -> Result<bool> {
        let mut conn = self.connection.write().await;
        let conn = conn.as_mut()
            .ok_or_else(|| RelayerError::Cache("Not connected to Redis".to_string()))?;

        let result: RedisResult<String> = redis::cmd("PING").query(conn);
        
        match result {
            Ok(response) => Ok(response == "PONG"),
            Err(_) => Ok(false),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RedisCacheStats {
    pub connected: bool,
    pub key_prefix: String,
    pub default_ttl_seconds: u64,
    pub memory_usage: u64,
    pub key_count: usize,
    pub hit_rate: f64,
}

impl Clone for RedisCache {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            connection: Arc::clone(&self.connection),
            key_prefix: self.key_prefix.clone(),
            default_ttl: self.default_ttl,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_redis_cache_creation() {
        // This would require a Redis instance in a real test
        let cache = RedisCache::new(
            "redis://localhost:6379",
            "test:".to_string(),
            Duration::from_secs(60),
        );
        
        assert!(cache.is_ok());
    }

    #[tokio::test]
    async fn test_cache_entry_creation() {
        let entry = CacheEntry::new("test_value".to_string(), Some(Duration::from_secs(60)));
        
        assert_eq!(entry.value, "test_value");
        assert!(entry.expires_at.is_some());
        assert_eq!(entry.access_count, 0);
    }

    #[tokio::test]
    async fn test_cache_entry_access() {
        let mut entry = CacheEntry::new("test_value".to_string(), Some(Duration::from_secs(60)));
        
        entry.access();
        assert_eq!(entry.access_count, 1);
    }

    #[tokio::test]
    async fn test_cache_entry_expiration() {
        let entry = CacheEntry::new("test_value".to_string(), Some(Duration::from_secs(0)));
        
        // Wait a bit to ensure expiration
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert!(entry.is_expired());
    }
}
