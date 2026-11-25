use crate::{
    models::messages::DisconnectReason,
    repositories::hub_registry::HubConnection,
    services::hub::{
        control_command::ControlCommand,
        hub_gateway::HubGateway,
        hub_registry_service::HubRegistryService,
        tasks::{BackgroundTask, ConnectionTracker},
    },
};

/// Periodic consumer that refreshes the Redis TTL of every locally-tracked connection and
/// disconnects any the registry no longer holds as active.
pub struct Heartbeat {
    hub_registry: HubRegistryService,
    gateway: HubGateway,
}

impl Heartbeat {
    pub fn new(hub_registry: HubRegistryService, gateway: HubGateway) -> Self {
        Self { hub_registry, gateway }
    }
}

impl BackgroundTask for Heartbeat {
    #[tracing::instrument(level = "debug", name = "hub heartbeat", skip_all)]
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
        let stale = match self.hub_registry.heartbeat_connections(&connections).await {
            Ok(stale) => stale,
            Err(err) => {
                tracing::error!("Failed to heartbeat hub connections: {err:#?}");
                return;
            }
        };

        for HubConnection { user_id, connection_id } in stale {
            tracing::info!("[{user_id}] heartbeat-found-inactive; requesting hub disconnect");
            let command = ControlCommand::DisconnectUser {
                user_id,
                connection_id,
                reason: DisconnectReason::HeartbeatInactive,
            };
            if let Err(err) = self.gateway.send_control(command) {
                tracing::error!("[{user_id}] Failed to send heartbeat disconnect command: {err:#?}");
            }
        }
    }
}
