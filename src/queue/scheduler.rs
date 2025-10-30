use std::collections::{BinaryHeap, HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::{RwLock, Semaphore};
use tokio::time::{Duration, Instant};
use uuid::Uuid;

use crate::types::{RelayerError, Result, TransactionRequest, Priority};

#[derive(Debug)]
pub struct TaskScheduler {
    priority_queue: Arc<RwLock<BinaryHeap<ScheduledTask>>>,
    processing_queue: Arc<RwLock<VecDeque<ScheduledTask>>>,
    completed_tasks: Arc<RwLock<HashMap<Uuid, TaskResult>>>,
    failed_tasks: Arc<RwLock<HashMap<Uuid, TaskResult>>>,
    semaphore: Arc<Semaphore>,
    max_queue_size: usize,
    processing_timeout: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTask {
    pub id: Uuid,
    pub request: TransactionRequest,
    pub priority: u8,
    pub created_at: Instant,
    pub scheduled_at: Instant,
    pub retry_count: u32,
    pub max_retries: u32,
}

impl Ord for ScheduledTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority first, then by scheduled time
        other.priority.cmp(&self.priority)
            .then_with(|| self.scheduled_at.cmp(&other.scheduled_at))
    }
}

impl PartialOrd for ScheduledTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
pub struct TaskResult {
    pub id: Uuid,
    pub success: bool,
    pub tx_hash: Option<String>,
    pub error_message: Option<String>,
    pub processing_time: Duration,
    pub completed_at: Instant,
}

impl TaskScheduler {
    pub fn new(max_concurrent: usize, max_queue_size: usize, processing_timeout: Duration) -> Self {
        Self {
            priority_queue: Arc::new(RwLock::new(BinaryHeap::new())),
            processing_queue: Arc::new(RwLock::new(VecDeque::new())),
            completed_tasks: Arc::new(RwLock::new(HashMap::new())),
            failed_tasks: Arc::new(RwLock::new(HashMap::new())),
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            max_queue_size,
            processing_timeout,
        }
    }

    pub async fn schedule_task(&self, request: TransactionRequest) -> Result<Uuid> {
        // Check queue size limit
        {
            let queue = self.priority_queue.read().await;
            if queue.len() >= self.max_queue_size {
                return Err(RelayerError::Queue("Queue is full".to_string()));
            }
        }

        let priority = request.priority.weight();
        let task = ScheduledTask {
            id: request.id,
            request,
            priority,
            created_at: Instant::now(),
            scheduled_at: Instant::now(),
            retry_count: 0,
            max_retries: 3,
        };

        {
            let mut queue = self.priority_queue.write().await;
            queue.push(task);
        }

        tracing::info!("Scheduled task {} with priority {}", task.id, priority);
        Ok(task.id)
    }

    pub async fn get_next_task(&self) -> Result<Option<ScheduledTask>> {
        let mut queue = self.priority_queue.write().await;
        Ok(queue.pop())
    }

    pub async fn start_processing(&self, task: ScheduledTask) -> Result<()> {
        // Acquire semaphore permit
        let _permit = self.semaphore.acquire().await
            .map_err(|e| RelayerError::Queue(e.to_string()))?;

        {
            let mut processing_queue = self.processing_queue.write().await;
            processing_queue.push_back(task);
        }

        Ok(())
    }

    pub async fn complete_task(&self, task_id: Uuid, success: bool, tx_hash: Option<String>, error_message: Option<String>) -> Result<()> {
        let processing_time = Instant::now() - Instant::now(); // This would be calculated from task start time
        
        let result = TaskResult {
            id: task_id,
            success,
            tx_hash,
            error_message,
            processing_time,
            completed_at: Instant::now(),
        };

        if success {
            let mut completed = self.completed_tasks.write().await;
            completed.insert(task_id, result);
        } else {
            let mut failed = self.failed_tasks.write().await;
            failed.insert(task_id, result);
        }

        // Remove from processing queue
        {
            let mut processing_queue = self.processing_queue.write().await;
            processing_queue.retain(|task| task.id != task_id);
        }

        tracing::info!("Completed task {} with success: {}", task_id, success);
        Ok(())
    }

    pub async fn retry_task(&self, task_id: Uuid) -> Result<()> {
        // Find the task in processing queue
        let mut task_to_retry = None;
        {
            let mut processing_queue = self.processing_queue.write().await;
            if let Some(pos) = processing_queue.iter().position(|task| task.id == task_id) {
                task_to_retry = processing_queue.remove(pos);
            }
        }

        if let Some(mut task) = task_to_retry {
            task.retry_count += 1;
            
            if task.retry_count <= task.max_retries {
                // Add delay before retry
                task.scheduled_at = Instant::now() + Duration::from_secs(5 * task.retry_count as u64);
                
                {
                    let mut queue = self.priority_queue.write().await;
                    queue.push(task);
                }

                tracing::info!("Retrying task {} (attempt {})", task_id, task.retry_count);
            } else {
                // Max retries exceeded, mark as failed
                self.complete_task(task_id, false, None, Some("Max retries exceeded".to_string())).await?;
            }
        }

        Ok(())
    }

    pub async fn get_queue_stats(&self) -> Result<QueueStats> {
        let priority_queue = self.priority_queue.read().await;
        let processing_queue = self.processing_queue.read().await;
        let completed_tasks = self.completed_tasks.read().await;
        let failed_tasks = self.failed_tasks.read().await;

        let pending_count = priority_queue.len();
        let processing_count = processing_queue.len();
        let completed_count = completed_tasks.len();
        let failed_count = failed_tasks.len();
        let available_permits = self.semaphore.available_permits();

        Ok(QueueStats {
            pending_tasks: pending_count,
            processing_tasks: processing_count,
            completed_tasks: completed_count,
            failed_tasks: failed_count,
            available_permits,
            max_queue_size: self.max_queue_size,
            processing_timeout_seconds: self.processing_timeout.as_secs(),
        })
    }

    pub async fn get_task_status(&self, task_id: Uuid) -> Result<TaskStatus> {
        // Check if task is in priority queue
        {
            let queue = self.priority_queue.read().await;
            if queue.iter().any(|task| task.id == task_id) {
                return Ok(TaskStatus::Pending);
            }
        }

        // Check if task is processing
        {
            let processing_queue = self.processing_queue.read().await;
            if processing_queue.iter().any(|task| task.id == task_id) {
                return Ok(TaskStatus::Processing);
            }
        }

        // Check if task is completed
        {
            let completed = self.completed_tasks.read().await;
            if completed.contains_key(&task_id) {
                return Ok(TaskStatus::Completed);
            }
        }

        // Check if task failed
        {
            let failed = self.failed_tasks.read().await;
            if failed.contains_key(&task_id) {
                return Ok(TaskStatus::Failed);
            }
        }

        Ok(TaskStatus::NotFound)
    }

    pub async fn cancel_task(&self, task_id: Uuid) -> Result<bool> {
        let mut cancelled = false;

        // Remove from priority queue
        {
            let mut queue = self.priority_queue.write().await;
            let original_len = queue.len();
            queue.retain(|task| task.id != task_id);
            cancelled = cancelled || queue.len() < original_len;
        }

        // Remove from processing queue
        {
            let mut processing_queue = self.processing_queue.write().await;
            let original_len = processing_queue.len();
            processing_queue.retain(|task| task.id != task_id);
            cancelled = cancelled || processing_queue.len() < original_len;
        }

        if cancelled {
            tracing::info!("Cancelled task {}", task_id);
        }

        Ok(cancelled)
    }

    pub async fn clear_completed_tasks(&self, older_than: Duration) -> Result<usize> {
        let cutoff_time = Instant::now() - older_than;
        let mut removed_count = 0;

        // Clear old completed tasks
        {
            let mut completed = self.completed_tasks.write().await;
            let original_len = completed.len();
            completed.retain(|_, result| result.completed_at > cutoff_time);
            removed_count += original_len - completed.len();
        }

        // Clear old failed tasks
        {
            let mut failed = self.failed_tasks.write().await;
            let original_len = failed.len();
            failed.retain(|_, result| result.completed_at > cutoff_time);
            removed_count += original_len - failed.len();
        }

        tracing::info!("Cleared {} old tasks", removed_count);
        Ok(removed_count)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Processing,
    Completed,
    Failed,
    NotFound,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueueStats {
    pub pending_tasks: usize,
    pub processing_tasks: usize,
    pub completed_tasks: usize,
    pub failed_tasks: usize,
    pub available_permits: usize,
    pub max_queue_size: usize,
    pub processing_timeout_seconds: u64,
}

impl Clone for TaskScheduler {
    fn clone(&self) -> Self {
        Self {
            priority_queue: Arc::clone(&self.priority_queue),
            processing_queue: Arc::clone(&self.processing_queue),
            completed_tasks: Arc::clone(&self.completed_tasks),
            failed_tasks: Arc::clone(&self.failed_tasks),
            semaphore: Arc::clone(&self.semaphore),
            max_queue_size: self.max_queue_size,
            processing_timeout: self.processing_timeout,
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
    async fn test_task_scheduler_creation() {
        let scheduler = TaskScheduler::new(5, 1000, Duration::from_secs(300));
        
        let stats = scheduler.get_queue_stats().await.unwrap();
        assert_eq!(stats.pending_tasks, 0);
        assert_eq!(stats.processing_tasks, 0);
        assert_eq!(stats.max_queue_size, 1000);
    }

    #[tokio::test]
    async fn test_schedule_task() {
        let scheduler = TaskScheduler::new(5, 1000, Duration::from_secs(300));
        let request = create_test_request();
        
        let task_id = scheduler.schedule_task(request).await.unwrap();
        
        let stats = scheduler.get_queue_stats().await.unwrap();
        assert_eq!(stats.pending_tasks, 1);
        
        let status = scheduler.get_task_status(task_id).await.unwrap();
        assert!(matches!(status, TaskStatus::Pending));
    }

    #[tokio::test]
    async fn test_task_priority() {
        let scheduler = TaskScheduler::new(5, 1000, Duration::from_secs(300));
        
        let mut request1 = create_test_request();
        request1.priority = Priority::Low;
        
        let mut request2 = create_test_request();
        request2.priority = Priority::High;
        
        scheduler.schedule_task(request1).await.unwrap();
        scheduler.schedule_task(request2).await.unwrap();
        
        // High priority task should be returned first
        let next_task = scheduler.get_next_task().await.unwrap().unwrap();
        assert_eq!(next_task.priority, Priority::High.weight());
    }

    #[tokio::test]
    async fn test_complete_task() {
        let scheduler = TaskScheduler::new(5, 1000, Duration::from_secs(300));
        let request = create_test_request();
        let task_id = scheduler.schedule_task(request).await.unwrap();
        
        let task = scheduler.get_next_task().await.unwrap().unwrap();
        scheduler.start_processing(task).await.unwrap();
        
        scheduler.complete_task(task_id, true, Some("0x123".to_string()), None).await.unwrap();
        
        let status = scheduler.get_task_status(task_id).await.unwrap();
        assert!(matches!(status, TaskStatus::Completed));
        
        let stats = scheduler.get_queue_stats().await.unwrap();
        assert_eq!(stats.completed_tasks, 1);
    }
}
