use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Transaction search filters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionFilters {
    pub status: Option<String>,
    pub priority: Option<String>,
    pub user_address: Option<String>,
    pub target_contract: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
}

impl Default for TransactionFilters {
    fn default() -> Self {
        Self {
            status: None,
            priority: None,
            user_address: None,
            target_contract: None,
            start_time: None,
            end_time: None,
        }
    }
}

impl TransactionFilters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_status(mut self, status: String) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_priority(mut self, priority: String) -> Self {
        self.priority = Some(priority);
        self
    }

    pub fn with_user_address(mut self, address: String) -> Self {
        self.user_address = Some(address);
        self
    }

    pub fn with_target_contract(mut self, contract: String) -> Self {
        self.target_contract = Some(contract);
        self
    }

    pub fn with_time_range(mut self, start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        self.start_time = Some(start);
        self.end_time = Some(end);
        self
    }
}

