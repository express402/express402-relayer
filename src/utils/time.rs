use chrono::{DateTime, Utc, Duration as ChronoDuration};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::types::{RelayerError, Result};

pub struct TimeUtils;

impl TimeUtils {
    /// Get current timestamp as Unix timestamp (seconds)
    pub fn now_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Get current timestamp as Unix timestamp (milliseconds)
    pub fn now_timestamp_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// Get current DateTime<Utc>
    pub fn now() -> DateTime<Utc> {
        Utc::now()
    }

    /// Convert Unix timestamp to DateTime<Utc>
    pub fn timestamp_to_datetime(timestamp: u64) -> Result<DateTime<Utc>> {
        DateTime::from_timestamp(timestamp as i64, 0)
            .ok_or_else(|| RelayerError::Internal("Invalid timestamp".to_string()))
    }

    /// Convert DateTime<Utc> to Unix timestamp
    pub fn datetime_to_timestamp(datetime: DateTime<Utc>) -> u64 {
        datetime.timestamp() as u64
    }

    /// Add duration to a DateTime
    pub fn add_duration(datetime: DateTime<Utc>, duration: Duration) -> DateTime<Utc> {
        datetime + ChronoDuration::from_std(duration).unwrap_or_default()
    }

    /// Subtract duration from a DateTime
    pub fn subtract_duration(datetime: DateTime<Utc>, duration: Duration) -> DateTime<Utc> {
        datetime - ChronoDuration::from_std(duration).unwrap_or_default()
    }

    /// Calculate duration between two DateTimes
    pub fn duration_between(start: DateTime<Utc>, end: DateTime<Utc>) -> Duration {
        let diff = end - start;
        Duration::from_secs(diff.num_seconds() as u64)
    }

    /// Check if a DateTime is expired (older than given duration)
    pub fn is_expired(datetime: DateTime<Utc>, max_age: Duration) -> bool {
        let now = Self::now();
        let diff = now - datetime;
        diff > ChronoDuration::from_std(max_age).unwrap_or_default()
    }

    /// Format DateTime as RFC3339 string
    pub fn format_rfc3339(datetime: DateTime<Utc>) -> String {
        datetime.to_rfc3339()
    }

    /// Parse RFC3339 string to DateTime
    pub fn parse_rfc3339(s: &str) -> Result<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(s)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| RelayerError::Internal(format!("Invalid RFC3339 format: {}", e)))
    }

    /// Format DateTime as ISO8601 string
    pub fn format_iso8601(datetime: DateTime<Utc>) -> String {
        datetime.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    }

    /// Parse ISO8601 string to DateTime
    pub fn parse_iso8601(s: &str) -> Result<DateTime<Utc>> {
        DateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.3fZ")
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| RelayerError::Internal(format!("Invalid ISO8601 format: {}", e)))
    }

    /// Get start of day for a given DateTime
    pub fn start_of_day(datetime: DateTime<Utc>) -> DateTime<Utc> {
        datetime.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc()
    }

    /// Get end of day for a given DateTime
    pub fn end_of_day(datetime: DateTime<Utc>) -> DateTime<Utc> {
        datetime.date_naive().and_hms_opt(23, 59, 59).unwrap().and_utc()
    }

    /// Get start of week for a given DateTime
    pub fn start_of_week(datetime: DateTime<Utc>) -> DateTime<Utc> {
        let days_since_monday = datetime.weekday().num_days_from_monday();
        datetime - ChronoDuration::days(days_since_monday as i64)
    }

    /// Get start of month for a given DateTime
    pub fn start_of_month(datetime: DateTime<Utc>) -> DateTime<Utc> {
        datetime.date_naive().with_day(1).unwrap().and_hms_opt(0, 0, 0).unwrap().and_utc()
    }

    /// Check if two DateTimes are on the same day
    pub fn is_same_day(datetime1: DateTime<Utc>, datetime2: DateTime<Utc>) -> bool {
        datetime1.date_naive() == datetime2.date_naive()
    }

    /// Check if two DateTimes are on the same week
    pub fn is_same_week(datetime1: DateTime<Utc>, datetime2: DateTime<Utc>) -> bool {
        Self::start_of_week(datetime1) == Self::start_of_week(datetime2)
    }

    /// Check if two DateTimes are on the same month
    pub fn is_same_month(datetime1: DateTime<Utc>, datetime2: DateTime<Utc>) -> bool {
        datetime1.month() == datetime2.month() && datetime1.year() == datetime2.year()
    }

    /// Get human-readable duration string
    pub fn human_duration(duration: Duration) -> String {
        let seconds = duration.as_secs();
        
        if seconds < 60 {
            format!("{}s", seconds)
        } else if seconds < 3600 {
            format!("{}m {}s", seconds / 60, seconds % 60)
        } else if seconds < 86400 {
            format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60)
        } else {
            format!("{}d {}h", seconds / 86400, (seconds % 86400) / 3600)
        }
    }

    /// Get human-readable time ago string
    pub fn time_ago(datetime: DateTime<Utc>) -> String {
        let now = Self::now();
        let diff = now - datetime;
        
        if diff.num_seconds() < 60 {
            "just now".to_string()
        } else if diff.num_minutes() < 60 {
            format!("{}m ago", diff.num_minutes())
        } else if diff.num_hours() < 24 {
            format!("{}h ago", diff.num_hours())
        } else if diff.num_days() < 30 {
            format!("{}d ago", diff.num_days())
        } else if diff.num_days() < 365 {
            format!("{}mo ago", diff.num_days() / 30)
        } else {
            format!("{}y ago", diff.num_days() / 365)
        }
    }

    /// Sleep for a given duration
    pub async fn sleep(duration: Duration) {
        tokio::time::sleep(duration).await;
    }

    /// Sleep until a specific DateTime
    pub async fn sleep_until(datetime: DateTime<Utc>) {
        let now = Self::now();
        if datetime > now {
            let duration = datetime - now;
            Self::sleep(Duration::from_secs(duration.num_seconds() as u64)).await;
        }
    }

    /// Create a timeout future
    pub fn timeout<F, T>(duration: Duration, future: F) -> tokio::time::Timeout<F>
    where
        F: std::future::Future<Output = T>,
    {
        tokio::time::timeout(duration, future)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl TimeRange {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        Self { start, end }
    }

    pub fn contains(&self, datetime: DateTime<Utc>) -> bool {
        datetime >= self.start && datetime <= self.end
    }

    pub fn duration(&self) -> Duration {
        TimeUtils::duration_between(self.start, self.end)
    }

    pub fn is_valid(&self) -> bool {
        self.start <= self.end
    }

    pub fn overlap(&self, other: &TimeRange) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    pub fn intersection(&self, other: &TimeRange) -> Option<TimeRange> {
        if !self.overlap(other) {
            return None;
        }

        let start = self.start.max(other.start);
        let end = self.end.min(other.end);
        
        Some(TimeRange::new(start, end))
    }
}

#[derive(Debug, Clone)]
pub struct Timer {
    start: Instant,
}

impl Timer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }

    pub fn elapsed_us(&self) -> u64 {
        self.start.elapsed().as_micros() as u64
    }

    pub fn reset(&mut self) {
        self.start = Instant::now();
    }
}

impl Default for Timer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_conversion() {
        let timestamp = TimeUtils::now_timestamp();
        let datetime = TimeUtils::timestamp_to_datetime(timestamp).unwrap();
        let back_to_timestamp = TimeUtils::datetime_to_timestamp(datetime);
        assert_eq!(timestamp, back_to_timestamp);
    }

    #[test]
    fn test_duration_calculations() {
        let start = TimeUtils::now();
        let end = start + ChronoDuration::seconds(60);
        let duration = TimeUtils::duration_between(start, end);
        assert_eq!(duration.as_secs(), 60);
    }

    #[test]
    fn test_expiration_check() {
        let past = TimeUtils::now() - ChronoDuration::seconds(120);
        let max_age = Duration::from_secs(60);
        assert!(TimeUtils::is_expired(past, max_age));
    }

    #[test]
    fn test_time_range() {
        let start = TimeUtils::now();
        let end = start + ChronoDuration::hours(1);
        let range = TimeRange::new(start, end);
        
        assert!(range.contains(start + ChronoDuration::minutes(30)));
        assert!(!range.contains(start + ChronoDuration::hours(2)));
        assert!(range.is_valid());
    }

    #[test]
    fn test_timer() {
        let timer = Timer::new();
        std::thread::sleep(Duration::from_millis(10));
        assert!(timer.elapsed_ms() >= 10);
    }

    #[test]
    fn test_human_duration() {
        assert_eq!(TimeUtils::human_duration(Duration::from_secs(30)), "30s");
        assert_eq!(TimeUtils::human_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(TimeUtils::human_duration(Duration::from_secs(3661)), "1h 1m");
    }
}
