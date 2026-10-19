use super::connection_tracker::{spawn_connection_loop, ConnectionConsumer, ConnectionTracker};
use crate::{
    models::messages::DisconnectReason,
    services::{hub::Consumer, HubService},
};
use shine_infra::session::CurrentUserService;
use std::{sync::Arc, time::Duration};
use tokio::task::JoinHandle;

/// Number of sessions checked in one Redis round trip.
const CHECK_BATCH_SIZE: usize = 256;

/// Periodic consumer that validates each locally-tracked connection's session and requests a
/// targeted disconnect on expiry.
pub struct SessionChecker {
    session_service: Arc<CurrentUserService>,
    hub_service: HubService,
}

impl SessionChecker {
    /// Starts the session checker on its own connection loop.
    pub async fn start(
        service: HubService,
        session_service: Arc<CurrentUserService>,
        interval: Duration,
    ) -> JoinHandle<()> {
        let consumer = SessionChecker {
            session_service,
            hub_service: service.clone(),
        };
        spawn_connection_loop(&service, interval, consumer).await
    }
}

impl ConnectionConsumer for SessionChecker {
    #[tracing::instrument(level = "debug", name = "hub session check", skip_all)]
    async fn on_tick(&mut self, tracker: &ConnectionTracker) {
        let _tick = self.hub_service.tick_guard(Consumer::SessionCheck);
        let connections: Vec<_> = tracker.connections().iter().map(|(u, c)| (*u, *c)).collect();

        let mut skipped = 0usize;
        let mut last_error = None;
        for chunk in connections.chunks(CHECK_BATCH_SIZE) {
            let sessions: Vec<_> = chunk.iter().map(|(user_id, (_, key))| (*user_id, *key)).collect();
            let active = match self.session_service.are_sessions_active(&sessions).await {
                Ok(active) => active,
                // Transient/infrastructure errors (Redis unavailable, pool exhausted, …)
                // Skip this batch; the next tick re-checks.
                Err(err) => {
                    skipped += chunk.len();
                    last_error = Some(err);
                    continue;
                }
            };

            for (&(user_id, (connection_id, _)), active) in chunk.iter().zip(active) {
                if active {
                    continue;
                }

                tracing::info!("[{user_id}] session expired; requesting hub disconnect");
                if let Err(err) =
                    self.hub_service
                        .request_drop(user_id, connection_id, DisconnectReason::SessionInvalid)
                {
                    tracing::error!("[{user_id}] Failed to send expiry disconnect command: {err:#?}");
                }
            }
        }

        if let Some(err) = last_error {
            tracing::warn!("session check skipped for {skipped} connection(s) due to transient error: {err:#?}");
        }
    }
}
