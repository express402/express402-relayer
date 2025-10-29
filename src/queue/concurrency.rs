use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{Duration, Instant};
use uuid::Uuid;

use crate::types::{RelayerError, Result};

#[derive(Debug, Clone)]
pub struct ConcurrencyController {
    semaphore: Arc<Semaphore>,
    active_tasks: Arc<RwLock<Vec<ActiveTask>>>,
    max_concurrent: usize,
    max_queue_size: usize,
    task_timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct ActiveTask {
    pub id: Uuid,
    pub started_at: Instant,
    pub timeout_at: Instant,
    pub priority: u8,
    pub resource_usage: ResourceUsage,
}

#[derive(Debug, Clone)]
pub struct ResourceUsage {
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub network_usage: f64,
    pub estimated_duration: Duration,
}

#[derive(Debug, Clone)]
pub struct ConcurrencyLimits {
    pub max_concurrent_tasks: usize,
    pub max_cpu_usage: f64,
    pub max_memory_usage: f64,
    pub max_network_usage: f64,
    pub task_timeout: Duration,
}

impl ConcurrencyController {
    pub fn new(limits: ConcurrencyLimits) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(limits.max_concurrent_tasks)),
            active_tasks: Arc::new(RwLock::new(Vec::new())),
            max_concurrent: limits.max_concurrent_tasks,
            max_queue_size: limits.max_concurrent_tasks * 2, // Allow some queuing
            task_timeout: limits.task_timeout,
        }
    }

    pub async fn acquire_permit(&self, task_id: Uuid, priority: u8, resource_usage: ResourceUsage) -> Result<ConcurrencyPermit> {
        // Check if we can acquire a permit
        let permit = self.semaphore.acquire().await
            .map_err(|e| RelayerError::Queue(e.to_string()))?;

        // Check resource limits
        if !self.check_resource_limits(&resource_usage).await? {
            drop(permit);
            return Err(RelayerError::Queue("Resource limits exceeded".to_string()));
        }

        // Register the active task
        let active_task = ActiveTask {
            id: task_id,
            started_at: Instant::now(),
            timeout_at: Instant::now() + self.task_timeout,
            priority,
            resource_usage: resource_usage.clone(),
        };

        {
            let mut active_tasks = self.active_tasks.write().await;
            active_tasks.push(active_task);
        }

        tracing::info!("Acquired concurrency permit for task {}", task_id);

        Ok(ConcurrencyPermit {
            task_id,
            permit,
            resource_usage,
        })
    }

    pub async fn release_permit(&self, task_id: Uuid) -> Result<()> {
        // Remove from active tasks
        {
            let mut active_tasks = self.active_tasks.write().await;
            active_tasks.retain(|task| task.id != task_id);
        }

        tracing::info!("Released concurrency permit for task {}", task_id);
        Ok(())
    }

    async fn check_resource_limits(&self, resource_usage: &ResourceUsage) -> Result<bool> {
        let active_tasks = self.active_tasks.read().await;
        
        let total_cpu = active_tasks.iter().map(|t| t.resource_usage.cpu_usage).sum::<f64>() + resource_usage.cpu_usage;
        let total_memory = active_tasks.iter().map(|t| t.resource_usage.memory_usage).sum::<f64>() + resource_usage.memory_usage;
        let total_network = active_tasks.iter().map(|t| t.resource_usage.network_usage).sum::<f64>() + resource_usage.network_usage;

        // Check if adding this task would exceed limits
        if total_cpu > 100.0 || total_memory > 100.0 || total_network > 100.0 {
            return Ok(false);
        }

        Ok(true)
    }

    pub async fn get_concurrency_stats(&self) -> Result<ConcurrencyStats> {
        let active_tasks = self.active_tasks.read().await;
        let available_permits = self.semaphore.available_permits();

        let total_cpu_usage: f64 = active_tasks.iter().map(|t| t.resource_usage.cpu_usage).sum();
        let total_memory_usage: f64 = active_tasks.iter().map(|t| t.resource_usage.memory_usage).sum();
        let total_network_usage: f64 = active_tasks.iter().map(|t| t.resource_usage.network_usage).sum();

        let average_task_duration = if !active_tasks.is_empty() {
            let total_duration: Duration = active_tasks.iter()
                .map(|t| t.resource_usage.estimated_duration)
                .sum();
            total_duration.as_secs_f64() / active_tasks.len() as f64
        } else {
            0.0
        };

        Ok(ConcurrencyStats {
            active_tasks: active_tasks.len(),
            available_permits,
            max_concurrent: self.max_concurrent,
            total_cpu_usage,
            total_memory_usage,
            total_network_usage,
            average_task_duration,
            task_timeout_seconds: self.task_timeout.as_secs(),
        })
    }

    pub async fn cleanup_expired_tasks(&self) -> Result<usize> {
        let now = Instant::now();
        let mut expired_count = 0;

        {
            let mut active_tasks = self.active_tasks.write().await;
            let original_len = active_tasks.len();
            active_tasks.retain(|task| {
                if task.timeout_at <= now {
                    expired_count += 1;
                    tracing::warn!("Task {} expired and was cleaned up", task.id);
                    false
                } else {
                    true
                }
            });
        }

        Ok(expired_count)
    }

    pub async fn get_task_priority_distribution(&self) -> Result<PriorityDistribution> {
        let active_tasks = self.active_tasks.read().await;
        
        let mut distribution = std::collections::HashMap::new();
        for task in active_tasks.iter() {
            *distribution.entry(task.priority).or_insert(0) += 1;
        }

        Ok(PriorityDistribution {
            distribution,
            total_tasks: active_tasks.len(),
        })
    }

    pub async fn adjust_concurrency_limits(&mut self, new_limits: ConcurrencyLimits) -> Result<()> {
        // Update the semaphore with new limits
        self.semaphore = Arc::new(Semaphore::new(new_limits.max_concurrent_tasks));
        self.max_concurrent = new_limits.max_concurrent_tasks;
        self.max_queue_size = new_limits.max_concurrent_tasks * 2;
        self.task_timeout = new_limits.task_timeout;

        tracing::info!("Updated concurrency limits: max_concurrent={}, timeout={}s", 
                      new_limits.max_concurrent_tasks, new_limits.task_timeout.as_secs());

        Ok(())
    }

    pub async fn get_resource_utilization(&self) -> Result<ResourceUtilization> {
        let active_tasks = self.active_tasks.read().await;
        
        let cpu_utilization = active_tasks.iter().map(|t| t.resource_usage.cpu_usage).sum::<f64>();
        let memory_utilization = active_tasks.iter().map(|t| t.resource_usage.memory_usage).sum::<f64>();
        let network_utilization = active_tasks.iter().map(|t| t.resource_usage.network_usage).sum::<f64>();

        Ok(ResourceUtilization {
            cpu_utilization,
            memory_utilization,
            network_utilization,
            active_task_count: active_tasks.len(),
            utilization_timestamp: chrono::Utc::now(),
        })
    }
}

pub struct ConcurrencyPermit {
    pub task_id: Uuid,
    permit: tokio::sync::SemaphorePermit<'static>,
    pub resource_usage: ResourceUsage,
}

impl Drop for ConcurrencyPermit {
    fn drop(&mut self) {
        // Permit is automatically released when dropped
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConcurrencyStats {
    pub active_tasks: usize,
    pub available_permits: usize,
    pub max_concurrent: usize,
    pub total_cpu_usage: f64,
    pub total_memory_usage: f64,
    pub total_network_usage: f64,
    pub average_task_duration: f64,
    pub task_timeout_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PriorityDistribution {
    pub distribution: std::collections::HashMap<u8, usize>,
    pub total_tasks: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceUtilization {
    pub cpu_utilization: f64,
    pub memory_utilization: f64,
    pub network_utilization: f64,
    pub active_task_count: usize,
    pub utilization_timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct ConcurrencyMonitor {
    controller: ConcurrencyController,
    monitoring_interval: Duration,
}

impl ConcurrencyMonitor {
    pub fn new(controller: ConcurrencyController, monitoring_interval: Duration) -> Self {
        Self {
            controller,
            monitoring_interval,
        }
    }

    pub async fn start_monitoring(&self) -> Result<()> {
        let monitor = Arc::new(self.clone());
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(monitor.monitoring_interval);
            
            loop {
                interval.tick().await;
                
                // Clean up expired tasks
                if let Err(e) = monitor.controller.cleanup_expired_tasks().await {
                    tracing::error!("Failed to cleanup expired tasks: {}", e);
                }

                // Log concurrency stats
                if let Ok(stats) = monitor.controller.get_concurrency_stats().await {
                    tracing::debug!("Concurrency stats: active={}, available={}, cpu={:.2}%, memory={:.2}%", 
                                  stats.active_tasks, stats.available_permits, 
                                  stats.total_cpu_usage, stats.total_memory_usage);
                }
            }
        });

        Ok(())
    }

    pub async fn get_monitoring_report(&self) -> Result<ConcurrencyMonitoringReport> {
        let stats = self.controller.get_concurrency_stats().await?;
        let resource_utilization = self.controller.get_resource_utilization().await?;
        let priority_distribution = self.controller.get_task_priority_distribution().await?;

        Ok(ConcurrencyMonitoringReport {
            stats,
            resource_utilization,
            priority_distribution,
            report_timestamp: chrono::Utc::now(),
        })
    }
}

impl Clone for ConcurrencyMonitor {
    fn clone(&self) -> Self {
        Self {
            controller: self.controller.clone(),
            monitoring_interval: self.monitoring_interval,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConcurrencyMonitoringReport {
    pub stats: ConcurrencyStats,
    pub resource_utilization: ResourceUtilization,
    pub priority_distribution: PriorityDistribution,
    pub report_timestamp: chrono::DateTime<chrono::Utc>,
}

impl Default for ConcurrencyLimits {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 10,
            max_cpu_usage: 80.0,
            max_memory_usage: 80.0,
            max_network_usage: 80.0,
            task_timeout: Duration::from_secs(300), // 5 minutes
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_concurrency_controller_creation() {
        let limits = ConcurrencyLimits::default();
        let controller = ConcurrencyController::new(limits);
        
        let stats = controller.get_concurrency_stats().await.unwrap();
        assert_eq!(stats.active_tasks, 0);
        assert_eq!(stats.available_permits, 10);
        assert_eq!(stats.max_concurrent, 10);
    }

    #[tokio::test]
    async fn test_acquire_and_release_permit() {
        let limits = ConcurrencyLimits::default();
        let controller = ConcurrencyController::new(limits);
        
        let task_id = Uuid::new_v4();
        let resource_usage = ResourceUsage {
            cpu_usage: 10.0,
            memory_usage: 20.0,
            network_usage: 5.0,
            estimated_duration: Duration::from_secs(60),
        };

        let permit = controller.acquire_permit(task_id, 2, resource_usage).await.unwrap();
        
        let stats = controller.get_concurrency_stats().await.unwrap();
        assert_eq!(stats.active_tasks, 1);
        assert_eq!(stats.available_permits, 9);

        drop(permit);
        
        let stats = controller.get_concurrency_stats().await.unwrap();
        assert_eq!(stats.active_tasks, 0);
        assert_eq!(stats.available_permits, 10);
    }

    #[tokio::test]
    async fn test_resource_limit_checking() {
        let limits = ConcurrencyLimits {
            max_concurrent_tasks: 2,
            max_cpu_usage: 50.0,
            max_memory_usage: 50.0,
            max_network_usage: 50.0,
            task_timeout: Duration::from_secs(300),
        };
        
        let controller = ConcurrencyController::new(limits);
        
        let task_id1 = Uuid::new_v4();
        let resource_usage1 = ResourceUsage {
            cpu_usage: 30.0,
            memory_usage: 30.0,
            network_usage: 30.0,
            estimated_duration: Duration::from_secs(60),
        };

        let task_id2 = Uuid::new_v4();
        let resource_usage2 = ResourceUsage {
            cpu_usage: 30.0,
            memory_usage: 30.0,
            network_usage: 30.0,
            estimated_duration: Duration::from_secs(60),
        };

        // First task should be allowed
        let _permit1 = controller.acquire_permit(task_id1, 2, resource_usage1).await.unwrap();
        
        // Second task should be allowed (total usage is 60%, under 100% limit)
        let _permit2 = controller.acquire_permit(task_id2, 2, resource_usage2).await.unwrap();
        
        let stats = controller.get_concurrency_stats().await.unwrap();
        assert_eq!(stats.active_tasks, 2);
    }

    #[tokio::test]
    async fn test_priority_distribution() {
        let limits = ConcurrencyLimits::default();
        let controller = ConcurrencyController::new(limits);
        
        let task_id1 = Uuid::new_v4();
        let resource_usage1 = ResourceUsage {
            cpu_usage: 10.0,
            memory_usage: 10.0,
            network_usage: 10.0,
            estimated_duration: Duration::from_secs(60),
        };

        let task_id2 = Uuid::new_v4();
        let resource_usage2 = ResourceUsage {
            cpu_usage: 10.0,
            memory_usage: 10.0,
            network_usage: 10.0,
            estimated_duration: Duration::from_secs(60),
        };

        let _permit1 = controller.acquire_permit(task_id1, 1, resource_usage1).await.unwrap();
        let _permit2 = controller.acquire_permit(task_id2, 3, resource_usage2).await.unwrap();
        
        let distribution = controller.get_task_priority_distribution().await.unwrap();
        assert_eq!(distribution.total_tasks, 2);
        assert_eq!(distribution.distribution.get(&1), Some(&1));
        assert_eq!(distribution.distribution.get(&3), Some(&1));
    }
}
