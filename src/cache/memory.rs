use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant};

use crate::types::{RelayerError, Result};
use super::redis::{RedisCache, RedisCacheStats};

#[derive(Debug)]
pub struct MemoryCache<T> {
    data: Arc<RwLock<HashMap<String, CacheEntry<T>>>>,
    max_size: usize,
    default_ttl: Duration,
    cleanup_interval: Duration,
}

#[derive(Debug, Clone)]
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

impl<T> MemoryCache<T>
where
    T: Clone + Send + Sync + 'static,
{
    pub fn new(max_size: usize, default_ttl: Duration, cleanup_interval: Duration) -> Self {
        let cache = Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            max_size,
            default_ttl,
            cleanup_interval,
        };

        // Start cleanup task
        cache.start_cleanup_task();
        cache
    }

    pub async fn get(&self, key: &str) -> Result<Option<T>> {
        let mut data = self.data.write().await;
        
        if let Some(entry) = data.get_mut(key) {
            if entry.is_expired() {
                data.remove(key);
                return Ok(None);
            }
            
            entry.access();
            Ok(Some(entry.value.clone()))
        } else {
            Ok(None)
        }
    }

    pub async fn set(&self, key: String, value: T, ttl: Option<Duration>) -> Result<()> {
        let mut data = self.data.write().await;
        
        // Check if we need to evict items
        if data.len() >= self.max_size && !data.contains_key(&key) {
            self.evict_lru(&mut data).await?;
        }
        
        let entry = CacheEntry::new(value, ttl.or(Some(self.default_ttl)));
        data.insert(key, entry);
        
        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<bool> {
        let mut data = self.data.write().await;
        Ok(data.remove(key).is_some())
    }

    pub async fn exists(&self, key: &str) -> Result<bool> {
        let data = self.data.read().await;
        
        if let Some(entry) = data.get(key) {
            Ok(!entry.is_expired())
        } else {
            Ok(false)
        }
    }

    pub async fn clear(&self) -> Result<()> {
        let mut data = self.data.write().await;
        data.clear();
        Ok(())
    }

    pub async fn get_multiple(&self, keys: &[String]) -> Result<HashMap<String, T>> {
        let data = self.data.read().await;
        let mut result = HashMap::new();
        
        for key in keys {
            if let Some(entry) = data.get(key) {
                if !entry.is_expired() {
                    result.insert(key.clone(), entry.value.clone());
                }
            }
        }
        
        Ok(result)
    }

    pub async fn set_multiple(&self, items: HashMap<String, T>, ttl: Option<Duration>) -> Result<()> {
        let mut data = self.data.write().await;
        
        for (key, value) in items {
            // Check if we need to evict items
            if data.len() >= self.max_size && !data.contains_key(&key) {
                self.evict_lru(&mut data).await?;
            }
            
            let entry = CacheEntry::new(value, ttl.or(Some(self.default_ttl)));
            data.insert(key, entry);
        }
        
        Ok(())
    }

    pub async fn get_stats(&self) -> Result<MemoryCacheStats> {
        let data = self.data.read().await;
        
        let mut total_access_count = 0u64;
        let mut total_size = 0usize;
        let mut expired_count = 0usize;
        
        for entry in data.values() {
            total_access_count += entry.access_count;
            total_size += 1;
            
            if entry.is_expired() {
                expired_count += 1;
            }
        }
        
        let average_access_count = if total_size > 0 {
            total_access_count as f64 / total_size as f64
        } else {
            0.0
        };
        
        Ok(MemoryCacheStats {
            total_entries: total_size,
            expired_entries: expired_count,
            max_size: self.max_size,
            usage_percentage: (total_size as f64 / self.max_size as f64) * 100.0,
            total_access_count,
            average_access_count,
            default_ttl_seconds: self.default_ttl.as_secs(),
        })
    }

    pub async fn cleanup_expired(&self) -> Result<usize> {
        let mut data = self.data.write().await;
        let mut removed_count = 0;
        
        data.retain(|_, entry| {
            if entry.is_expired() {
                removed_count += 1;
                false
            } else {
                true
            }
        });
        
        Ok(removed_count)
    }

    async fn evict_lru(&self, data: &mut HashMap<String, CacheEntry<T>>) -> Result<()> {
        if data.is_empty() {
            return Ok(());
        }
        
        // Find the least recently used entry
        let mut lru_key = None;
        let mut oldest_access = Instant::now();
        
        for (key, entry) in data.iter() {
            if entry.last_accessed < oldest_access {
                oldest_access = entry.last_accessed;
                lru_key = Some(key.clone());
            }
        }
        
        if let Some(key) = lru_key {
            data.remove(&key);
            tracing::debug!("Evicted LRU key: {}", key);
        }
        
        Ok(())
    }

    fn start_cleanup_task(&self) {
        let cache = Arc::new(self.clone());
        let cleanup_interval = self.cleanup_interval;
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(cleanup_interval);
            
            loop {
                interval.tick().await;
                
                if let Err(e) = cache.cleanup_expired().await {
                    tracing::error!("Failed to cleanup expired entries: {}", e);
                }
            }
        });
    }

    pub async fn get_keys(&self) -> Result<Vec<String>> {
        let data = self.data.read().await;
        let mut keys = Vec::new();
        
        for (key, entry) in data.iter() {
            if !entry.is_expired() {
                keys.push(key.clone());
            }
        }
        
        Ok(keys)
    }

    pub async fn get_entry_info(&self, key: &str) -> Result<Option<CacheEntryInfo>> {
        let data = self.data.read().await;
        
        if let Some(entry) = data.get(key) {
            if entry.is_expired() {
                return Ok(None);
            }
            
            Ok(Some(CacheEntryInfo {
                key: key.to_string(),
                created_at: entry.created_at,
                expires_at: entry.expires_at,
                access_count: entry.access_count,
                last_accessed: entry.last_accessed,
                age_seconds: entry.created_at.elapsed().as_secs(),
                is_expired: entry.is_expired(),
            }))
        } else {
            Ok(None)
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemoryCacheStats {
    pub total_entries: usize,
    pub expired_entries: usize,
    pub max_size: usize,
    pub usage_percentage: f64,
    pub total_access_count: u64,
    pub average_access_count: f64,
    pub default_ttl_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheEntryInfo {
    pub key: String,
    pub created_at: Instant,
    pub expires_at: Option<Instant>,
    pub access_count: u64,
    pub last_accessed: Instant,
    pub age_seconds: u64,
    pub is_expired: bool,
}

impl<T> Clone for MemoryCache<T> {
    fn clone(&self) -> Self {
        Self {
            data: Arc::clone(&self.data),
            max_size: self.max_size,
            default_ttl: self.default_ttl,
            cleanup_interval: self.cleanup_interval,
        }
    }
}

pub struct CacheManager<T> {
    memory_cache: MemoryCache<T>,
    redis_cache: Option<RedisCache>,
    use_redis: bool,
}

impl<T> CacheManager<T>
where
    T: Clone + Send + Sync + Serialize + for<'de> Deserialize<'de> + 'static,
{
    pub fn new(
        memory_cache: MemoryCache<T>,
        redis_cache: Option<RedisCache>,
        use_redis: bool,
    ) -> Self {
        Self {
            memory_cache,
            redis_cache,
            use_redis,
        }
    }

    pub async fn get(&self, key: &str) -> Result<Option<T>> {
        // Try memory cache first
        if let Some(value) = self.memory_cache.get(key).await? {
            return Ok(Some(value));
        }

        // Try Redis cache if available
        if self.use_redis {
            if let Some(ref redis_cache) = self.redis_cache {
                if let Some(value) = redis_cache.get::<T>(key).await? {
                    // Store in memory cache for faster access
                    self.memory_cache.set(key.to_string(), value.clone(), None).await?;
                    return Ok(Some(value));
                }
            }
        }

        Ok(None)
    }

    pub async fn set(&self, key: String, value: T, ttl: Option<Duration>) -> Result<()> {
        // Set in memory cache
        self.memory_cache.set(key.clone(), value.clone(), ttl).await?;

        // Set in Redis cache if available
        if self.use_redis {
            if let Some(ref redis_cache) = self.redis_cache {
                redis_cache.set(&key, &value, ttl).await?;
            }
        }

        Ok(())
    }

    pub async fn delete(&self, key: &str) -> Result<bool> {
        let mut deleted = false;

        // Delete from memory cache
        if self.memory_cache.delete(key).await? {
            deleted = true;
        }

        // Delete from Redis cache if available
        if self.use_redis {
            if let Some(ref redis_cache) = self.redis_cache {
                if redis_cache.delete(key).await? {
                    deleted = true;
                }
            }
        }

        Ok(deleted)
    }

    pub async fn exists(&self, key: &str) -> Result<bool> {
        // Check memory cache first
        if self.memory_cache.exists(key).await? {
            return Ok(true);
        }

        // Check Redis cache if available
        if self.use_redis {
            if let Some(ref redis_cache) = self.redis_cache {
                return redis_cache.exists(key).await;
            }
        }

        Ok(false)
    }

    pub async fn clear(&self) -> Result<()> {
        // Clear memory cache
        self.memory_cache.clear().await?;

        // Clear Redis cache if available
        if self.use_redis {
            if let Some(ref redis_cache) = self.redis_cache {
                redis_cache.clear_pattern("*").await?;
            }
        }

        Ok(())
    }

    pub async fn get_stats(&self) -> Result<CacheManagerStats> {
        let memory_stats = self.memory_cache.get_stats().await?;
        let redis_stats = if self.use_redis && self.redis_cache.is_some() {
            self.redis_cache.as_ref().unwrap().get_stats().await.ok()
        } else {
            None
        };

        Ok(CacheManagerStats {
            memory_stats,
            redis_stats,
            use_redis: self.use_redis,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheManagerStats {
    pub memory_stats: MemoryCacheStats,
    pub redis_stats: Option<RedisCacheStats>,
    pub use_redis: bool,
}

impl<T> Clone for CacheManager<T> {
    fn clone(&self) -> Self {
        Self {
            memory_cache: self.memory_cache.clone(),
            redis_cache: self.redis_cache.clone(),
            use_redis: self.use_redis,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_cache_creation() {
        let cache = MemoryCache::new(
            100,
            Duration::from_secs(60),
            Duration::from_secs(300),
        );
        
        let stats = cache.get_stats().await.unwrap();
        assert_eq!(stats.total_entries, 0);
        assert_eq!(stats.max_size, 100);
    }

    #[tokio::test]
    async fn test_memory_cache_set_get() {
        let cache = MemoryCache::new(
            100,
            Duration::from_secs(60),
            Duration::from_secs(300),
        );
        
        cache.set("key1".to_string(), "value1".to_string(), None).await.unwrap();
        
        let value = cache.get("key1").await.unwrap();
        assert_eq!(value, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn test_memory_cache_expiration() {
        let cache = MemoryCache::new(
            100,
            Duration::from_secs(60),
            Duration::from_secs(300),
        );
        
        cache.set("key1".to_string(), "value1".to_string(), Some(Duration::from_millis(10))).await.unwrap();
        
        // Wait for expiration
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        let value = cache.get("key1").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_memory_cache_lru_eviction() {
        let cache = MemoryCache::new(
            2, // Very small cache
            Duration::from_secs(60),
            Duration::from_secs(300),
        );
        
        cache.set("key1".to_string(), "value1".to_string(), None).await.unwrap();
        cache.set("key2".to_string(), "value2".to_string(), None).await.unwrap();
        cache.set("key3".to_string(), "value3".to_string(), None).await.unwrap(); // Should evict key1
        
        // key1 should be evicted
        let value1 = cache.get("key1").await.unwrap();
        assert_eq!(value1, None);
        
        // key2 and key3 should still be there
        let value2 = cache.get("key2").await.unwrap();
        let value3 = cache.get("key3").await.unwrap();
        assert_eq!(value2, Some("value2".to_string()));
        assert_eq!(value3, Some("value3".to_string()));
    }

    #[tokio::test]
    async fn test_cache_entry_access_tracking() {
        let mut entry = CacheEntry::new("test_value".to_string(), Some(Duration::from_secs(60)));
        
        assert_eq!(entry.access_count, 0);
        
        entry.access();
        assert_eq!(entry.access_count, 1);
        
        entry.access();
        assert_eq!(entry.access_count, 2);
    }
}
