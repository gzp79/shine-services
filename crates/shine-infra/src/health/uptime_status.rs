use crate::health::StatusProvider;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::time::Instant;

pub struct UptimeStatus {
    start_instant: Instant,
    start_time: DateTime<Utc>,
}

impl UptimeStatus {
    pub fn new() -> Self {
        Self {
            start_instant: Instant::now(),
            start_time: Utc::now(),
        }
    }
}

impl Default for UptimeStatus {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatusProvider for UptimeStatus {
    fn name(&self) -> &'static str {
        "uptime"
    }

    async fn status(&self) -> serde_json::Value {
        let uptime_seconds = self.start_instant.elapsed().as_secs();
        serde_json::json!({
            "startTime": self.start_time.to_rfc3339(),
            "uptimeSeconds": uptime_seconds
        })
    }
}
