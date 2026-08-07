use super::connection_tracker::{spawn_connection_loop, ConnectionConsumer, ConnectionTracker};
use crate::services::HubService;
use shine_infra::session::{CurrentUserService, UserSessionError};
use std::{sync::Arc, time::Duration};
use tokio::task::JoinHandle;

/// Periodic consumer: validates each locally-tracked connection's session against
/// CurrentUserService and requests a targeted disconnect on expiry. Read-only w.r.t. hub state.
pub struct SessionChecker {
    session_service: Arc<CurrentUserService>,
    hub_service: HubService,
}

impl SessionChecker {
    /// Starts the session checker on its own connection loop, validating each tracked session and
    /// issuing a targeted disconnect on expiry.
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
    async fn on_tick(&mut self, tracker: &ConnectionTracker) {
        for (user_id, (connection_id, session_key)) in tracker.connections().iter().map(|(u, c)| (*u, *c)) {
            match self.session_service.get_current_user(user_id, session_key).await {
                // Session is valid; keep the connection.
                Ok(_) => {}
                // The session is definitively gone/invalid: close the connection.
                Err(err @ (UserSessionError::SessionExpired | UserSessionError::SessionCompromised)) => {
                    log::info!("[{user_id}] session-invalid ({err}); requesting hub disconnect");
                    // Target the exact connection we validated. If the user reconnected with a new
                    // connection meanwhile, the hub ignores this stale id and keeps the fresh session.
                    if let Err(err) = self.hub_service.request_disconnection(user_id, connection_id) {
                        log::error!("[{user_id}] Failed to send expiry disconnect command: {err:#?}");
                    }
                }
                // Transient/infrastructure errors (Redis unavailable, pool exhausted, …)
                // Skip this tick; the next one re-checks.
                Err(err) => {
                    log::warn!("[{user_id}] session check skipped due to transient error: {err:#?}");
                }
            }
        }
    }
}
