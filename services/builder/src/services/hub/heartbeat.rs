use crate::services::{ConnectionConsumer, ConnectionTracker, HubSender, HubService};

/// Periodic consumer of a connection view. Refreshes each locally-tracked connection's Redis
/// TTL; a connection the registry no longer holds as active is disconnected through the CAS'd
/// command path (so a fresher local connection is never torn down).
pub struct HeartbeatTask {
    hub_service: HubService,
    sender: HubSender,
}

impl HeartbeatTask {
    pub fn new(hub_service: HubService) -> Self {
        let sender = hub_service.sender();
        Self { hub_service, sender }
    }
}

impl ConnectionConsumer for HeartbeatTask {
    async fn on_tick(&self, tracker: &ConnectionTracker) {
        for (user_id, (connection_id, _session_key)) in tracker.connections().iter().map(|(u, c)| (*u, *c)) {
            match self
                .hub_service
                .heartbeat_registry_connection(user_id, connection_id)
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    log::info!("[{user_id}] heartbeat-found-inactive; requesting hub disconnect");
                    if let Err(err) = self.sender.disconnect(user_id, connection_id) {
                        log::error!("[{user_id}] Failed to send heartbeat disconnect command: {err:#?}");
                    }
                }
                Err(err) => {
                    log::error!("[{user_id}] Failed to heartbeat hub connection: {err:#?}");
                }
            }
        }
    }
}
