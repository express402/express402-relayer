use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::types::{RelayerError, Result, Priority};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityManager {
    priority_weights: HashMap<String, u8>,
    priority_multipliers: HashMap<String, f64>,
    dynamic_adjustments: HashMap<String, f64>,
}

impl PriorityManager {
    pub fn new() -> Self {
        let mut priority_weights = HashMap::new();
        priority_weights.insert("low".to_string(), 1);
        priority_weights.insert("normal".to_string(), 2);
        priority_weights.insert("high".to_string(), 3);
        priority_weights.insert("critical".to_string(), 4);

        let mut priority_multipliers = HashMap::new();
        priority_multipliers.insert("low".to_string(), 0.5);
        priority_multipliers.insert("normal".to_string(), 1.0);
        priority_multipliers.insert("high".to_string(), 1.5);
        priority_multipliers.insert("critical".to_string(), 2.0);

        Self {
            priority_weights,
            priority_multipliers,
            dynamic_adjustments: HashMap::new(),
        }
    }

    pub fn get_priority_weight(&self, priority: &Priority) -> u8 {
        let priority_str = self.priority_to_string(priority);
        self.priority_weights.get(&priority_str).copied().unwrap_or(2)
    }

    pub fn get_priority_multiplier(&self, priority: &Priority) -> f64 {
        let priority_str = self.priority_to_string(priority);
        self.priority_multipliers.get(&priority_str).copied().unwrap_or(1.0)
    }

    pub fn calculate_dynamic_priority(&self, priority: &Priority, factors: PriorityFactors) -> f64 {
        let base_weight = self.get_priority_weight(priority) as f64;
        let multiplier = self.get_priority_multiplier(priority);
        
        // Apply dynamic adjustments based on various factors
        let mut adjusted_priority = base_weight * multiplier;

        // Time-based adjustment
        if factors.age_seconds > 300 { // 5 minutes
            adjusted_priority *= 1.2; // Increase priority for old tasks
        }

        // User tier adjustment
        adjusted_priority *= factors.user_tier_multiplier;

        // Gas price adjustment
        if factors.gas_price_ratio > 1.5 {
            adjusted_priority *= 1.1; // Slightly increase priority for high gas prices
        }

        // Network congestion adjustment
        adjusted_priority *= factors.network_congestion_multiplier;

        // Apply any dynamic adjustments
        let priority_str = self.priority_to_string(priority);
        if let Some(adjustment) = self.dynamic_adjustments.get(&priority_str) {
            adjusted_priority *= adjustment;
        }

        adjusted_priority
    }

    pub fn set_priority_weight(&mut self, priority: &Priority, weight: u8) {
        let priority_str = self.priority_to_string(priority);
        self.priority_weights.insert(priority_str, weight);
    }

    pub fn set_priority_multiplier(&mut self, priority: &Priority, multiplier: f64) {
        let priority_str = self.priority_to_string(priority);
        self.priority_multipliers.insert(priority_str, multiplier);
    }

    pub fn adjust_priority_dynamically(&mut self, priority: &Priority, adjustment: f64) {
        let priority_str = self.priority_to_string(priority);
        self.dynamic_adjustments.insert(priority_str, adjustment);
    }

    pub fn reset_dynamic_adjustments(&mut self) {
        self.dynamic_adjustments.clear();
    }

    pub fn get_priority_stats(&self) -> PriorityStats {
        let mut stats = PriorityStats {
            total_weights: 0,
            total_multipliers: 0.0,
            dynamic_adjustments_count: self.dynamic_adjustments.len(),
            priority_distribution: HashMap::new(),
        };

        for (priority_str, weight) in &self.priority_weights {
            stats.total_weights += *weight as u32;
            stats.priority_distribution.insert(priority_str.clone(), *weight as u32);
        }

        for multiplier in self.priority_multipliers.values() {
            stats.total_multipliers += multiplier;
        }

        stats
    }

    fn priority_to_string(&self, priority: &Priority) -> String {
        match priority {
            Priority::Low => "low".to_string(),
            Priority::Normal => "normal".to_string(),
            Priority::High => "high".to_string(),
            Priority::Critical => "critical".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityFactors {
    pub age_seconds: u64,
    pub user_tier_multiplier: f64,
    pub gas_price_ratio: f64,
    pub network_congestion_multiplier: f64,
    pub retry_count: u32,
    pub estimated_gas_cost: u64,
}

impl Default for PriorityFactors {
    fn default() -> Self {
        Self {
            age_seconds: 0,
            user_tier_multiplier: 1.0,
            gas_price_ratio: 1.0,
            network_congestion_multiplier: 1.0,
            retry_count: 0,
            estimated_gas_cost: 0,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PriorityStats {
    pub total_weights: u32,
    pub total_multipliers: f64,
    pub dynamic_adjustments_count: usize,
    pub priority_distribution: HashMap<String, u32>,
}

pub struct PriorityCalculator {
    priority_manager: PriorityManager,
    historical_data: HashMap<String, PriorityHistory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityHistory {
    pub priority_type: String,
    pub total_tasks: u64,
    pub completed_tasks: u64,
    pub average_completion_time: f64,
    pub success_rate: f64,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

impl PriorityCalculator {
    pub fn new() -> Self {
        Self {
            priority_manager: PriorityManager::new(),
            historical_data: HashMap::new(),
        }
    }

    pub fn calculate_optimal_priority(&self, factors: PriorityFactors) -> Priority {
        // Calculate priority scores for each priority level
        let low_score = self.priority_manager.calculate_dynamic_priority(&Priority::Low, factors.clone());
        let normal_score = self.priority_manager.calculate_dynamic_priority(&Priority::Normal, factors.clone());
        let high_score = self.priority_manager.calculate_dynamic_priority(&Priority::High, factors.clone());
        let critical_score = self.priority_manager.calculate_dynamic_priority(&Priority::Critical, factors.clone());

        // Select the priority with the highest score
        if critical_score >= high_score && critical_score >= normal_score && critical_score >= low_score {
            Priority::Critical
        } else if high_score >= normal_score && high_score >= low_score {
            Priority::High
        } else if normal_score >= low_score {
            Priority::Normal
        } else {
            Priority::Low
        }
    }

    pub fn update_priority_history(&mut self, priority: &Priority, completion_time: f64, success: bool) {
        let priority_str = self.priority_to_string(priority);
        
        let history = self.historical_data.entry(priority_str.clone()).or_insert(PriorityHistory {
            priority_type: priority_str,
            total_tasks: 0,
            completed_tasks: 0,
            average_completion_time: 0.0,
            success_rate: 0.0,
            last_updated: chrono::Utc::now(),
        });

        history.total_tasks += 1;
        if success {
            history.completed_tasks += 1;
        }

        // Update average completion time
        history.average_completion_time = 
            (history.average_completion_time * (history.total_tasks - 1) as f64 + completion_time) 
            / history.total_tasks as f64;

        // Update success rate
        history.success_rate = history.completed_tasks as f64 / history.total_tasks as f64;
        history.last_updated = chrono::Utc::now();
    }

    pub fn get_priority_recommendation(&self, factors: PriorityFactors) -> PriorityRecommendation {
        let optimal_priority = self.calculate_optimal_priority(factors.clone());
        
        // Get historical data for the optimal priority
        let priority_str = self.priority_to_string(&optimal_priority);
        let history = self.historical_data.get(&priority_str);

        let estimated_completion_time = history
            .map(|h| h.average_completion_time)
            .unwrap_or(300.0); // Default 5 minutes

        let expected_success_rate = history
            .map(|h| h.success_rate)
            .unwrap_or(0.95); // Default 95%

        PriorityRecommendation {
            recommended_priority: optimal_priority,
            estimated_completion_time,
            expected_success_rate,
            confidence_score: self.calculate_confidence_score(factors),
            reasoning: self.generate_reasoning(&optimal_priority, factors),
        }
    }

    fn calculate_confidence_score(&self, factors: PriorityFactors) -> f64 {
        let mut confidence = 0.5; // Base confidence

        // Increase confidence based on historical data availability
        if factors.age_seconds > 0 {
            confidence += 0.1;
        }

        if factors.retry_count == 0 {
            confidence += 0.1;
        }

        // Adjust based on network conditions
        if factors.network_congestion_multiplier > 0.8 && factors.network_congestion_multiplier < 1.2 {
            confidence += 0.1;
        }

        confidence.min(1.0)
    }

    fn generate_reasoning(&self, priority: &Priority, factors: PriorityFactors) -> String {
        let mut reasons = Vec::new();

        match priority {
            Priority::Critical => {
                reasons.push("High priority due to critical nature".to_string());
                if factors.age_seconds > 300 {
                    reasons.push("Task has been waiting for over 5 minutes".to_string());
                }
                if factors.user_tier_multiplier > 1.5 {
                    reasons.push("High-tier user request".to_string());
                }
            }
            Priority::High => {
                reasons.push("Elevated priority for faster processing".to_string());
                if factors.gas_price_ratio > 1.2 {
                    reasons.push("Higher gas price indicates urgency".to_string());
                }
            }
            Priority::Normal => {
                reasons.push("Standard processing priority".to_string());
            }
            Priority::Low => {
                reasons.push("Lower priority for cost optimization".to_string());
                if factors.gas_price_ratio < 0.8 {
                    reasons.push("Lower gas price allows for slower processing".to_string());
                }
            }
        }

        if factors.retry_count > 0 {
            reasons.push(format!("Retry attempt #{}", factors.retry_count));
        }

        reasons.join("; ")
    }

    fn priority_to_string(&self, priority: &Priority) -> String {
        match priority {
            Priority::Low => "low".to_string(),
            Priority::Normal => "normal".to_string(),
            Priority::High => "high".to_string(),
            Priority::Critical => "critical".to_string(),
        }
    }

    pub fn get_priority_manager(&self) -> &PriorityManager {
        &self.priority_manager
    }

    pub fn get_priority_manager_mut(&mut self) -> &mut PriorityManager {
        &mut self.priority_manager
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PriorityRecommendation {
    pub recommended_priority: Priority,
    pub estimated_completion_time: f64,
    pub expected_success_rate: f64,
    pub confidence_score: f64,
    pub reasoning: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_priority_manager_creation() {
        let manager = PriorityManager::new();
        
        assert_eq!(manager.get_priority_weight(&Priority::Low), 1);
        assert_eq!(manager.get_priority_weight(&Priority::Normal), 2);
        assert_eq!(manager.get_priority_weight(&Priority::High), 3);
        assert_eq!(manager.get_priority_weight(&Priority::Critical), 4);
    }

    #[test]
    fn test_priority_multipliers() {
        let manager = PriorityManager::new();
        
        assert_eq!(manager.get_priority_multiplier(&Priority::Low), 0.5);
        assert_eq!(manager.get_priority_multiplier(&Priority::Normal), 1.0);
        assert_eq!(manager.get_priority_multiplier(&Priority::High), 1.5);
        assert_eq!(manager.get_priority_multiplier(&Priority::Critical), 2.0);
    }

    #[test]
    fn test_dynamic_priority_calculation() {
        let manager = PriorityManager::new();
        let factors = PriorityFactors {
            age_seconds: 400, // Over 5 minutes
            user_tier_multiplier: 1.5,
            gas_price_ratio: 1.2,
            network_congestion_multiplier: 1.0,
            retry_count: 0,
            estimated_gas_cost: 100000,
        };

        let dynamic_priority = manager.calculate_dynamic_priority(&Priority::High, factors);
        
        // Should be higher than base priority due to age and user tier
        assert!(dynamic_priority > 3.0);
    }

    #[test]
    fn test_priority_calculator() {
        let mut calculator = PriorityCalculator::new();
        let factors = PriorityFactors::default();
        
        let recommendation = calculator.get_priority_recommendation(factors);
        
        assert!(recommendation.confidence_score >= 0.0 && recommendation.confidence_score <= 1.0);
        assert!(!recommendation.reasoning.is_empty());
    }

    #[test]
    fn test_priority_history_update() {
        let mut calculator = PriorityCalculator::new();
        
        calculator.update_priority_history(&Priority::Normal, 120.0, true);
        calculator.update_priority_history(&Priority::Normal, 180.0, false);
        
        let history = calculator.historical_data.get("normal").unwrap();
        assert_eq!(history.total_tasks, 2);
        assert_eq!(history.completed_tasks, 1);
        assert_eq!(history.success_rate, 0.5);
        assert_eq!(history.average_completion_time, 150.0);
    }
}
