use crate::services::HubService;
use async_trait::async_trait;
use serde_json::Value as JsonValue;
use shine_infra::health::StatusProvider;

/// Health/status provider that reports live hub counts (connected users and active subscribers).
pub struct HubStatus {
    hub_service: HubService,
}

impl HubStatus {
    pub fn new(hub_service: HubService) -> Self {
        Self { hub_service }
    }
}

#[async_trait]
impl StatusProvider for HubStatus {
    fn name(&self) -> &'static str {
        "hub"
    }

    async fn status(&self) -> JsonValue {
        let stats = self.hub_service.stats().await;
        serde_json::json!({
            "connections": stats.connections,
            "subscribers": stats.subscribers,
        })
    }
}
