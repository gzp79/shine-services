use super::*;

impl HubService {
    /// Subscribes to a set of topics on the hub's message bus.
    pub async fn subscribe(&self, topics: Vec<TopicKey>) -> mpsc::UnboundedReceiver<HubMessage> {
        self.inner.subscribers.subscribe(topics).await
    }

    /// Sends an egress message to a specific connection. Returns whether the message was queued for delivery.
    #[tracing::instrument(level = "debug", skip_all, fields(ws.connection_id = %connection_id))]
    pub async fn send_to_connection(&self, connection_id: Uuid, message: HubMessage) -> Result<bool, HubError> {
        self.inner.gateway.send_to_connection(connection_id, message).await
    }
}
