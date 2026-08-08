use super::connection_tracker::{spawn_connection_loop, ConnectionConsumer, ConnectionTracker};
use crate::{repositories::hub_registry::HubConnection, services::HubService};
use std::time::Duration;
use tokio::task::JoinHandle;

/// Periodic consumer that refreshes the Redis TTL of every locally-tracked connection and
/// disconnects any the registry no longer holds as active.
pub struct Heartbeat {
    hub_service: HubService,
}

impl Heartbeat {
    /// Starts the heartbeat on its own connection loop.
    pub async fn start(service: HubService, interval: Duration) -> JoinHandle<()> {
        let consumer = Heartbeat { hub_service: service.clone() };
        spawn_connection_loop(&service, interval, consumer).await
    }
}

impl ConnectionConsumer for Heartbeat {
    async fn on_tick(&mut self, tracker: &ConnectionTracker) {
        let connections: Vec<HubConnection> = tracker
            .connections()
            .iter()
            .map(|(user_id, (connection_id, _session_key))| HubConnection {
                user_id: *user_id,
                connection_id: *connection_id,
            })
            .collect();

        if connections.is_empty() {
            return;
        }

        // One batched heartbeat over all tracked connections: extends the TTL of the ones still
        // active in the registry and returns the ones it no longer holds, which we disconnect.
        let stale = match self.hub_service.heartbeat_registry_connections(&connections).await {
            Ok(stale) => stale,
            Err(err) => {
                log::error!("Failed to heartbeat hub connections: {err:#?}");
                return;
            }
        };

        for HubConnection { user_id, connection_id } in stale {
            log::info!("[{user_id}] heartbeat-found-inactive; requesting hub disconnect");
            if let Err(err) = self.hub_service.request_disconnection(user_id, connection_id) {
                log::error!("[{user_id}] Failed to send heartbeat disconnect command: {err:#?}");
            }
        }
    }
}
