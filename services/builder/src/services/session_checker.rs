use crate::services::{ConnectionConsumer, ConnectionTracker, HubSender, HubService};
use shine_infra::session::CurrentUserService;
use std::sync::Arc;

/// Periodic consumer: validates each locally-tracked connection's session against
/// CurrentUserService and requests a targeted disconnect on expiry. Read-only w.r.t. hub state.
pub struct SessionChecker {
    session_service: Arc<CurrentUserService>,
    sender: HubSender,
}

impl SessionChecker {
    pub fn new(session_service: Arc<CurrentUserService>, hub_service: &HubService) -> Self {
        Self {
            session_service,
            sender: hub_service.sender(),
        }
    }
}

impl ConnectionConsumer for SessionChecker {
    async fn on_tick(&self, tracker: &ConnectionTracker) {
        for (user_id, (connection_id, session_key)) in tracker.connections().iter().map(|(u, c)| (*u, *c)) {
            if self
                .session_service
                .get_current_user(user_id, session_key)
                .await
                .is_err()
            {
                log::info!("[{user_id}] session-expiry-detected; requesting hub disconnect");
                // Target the exact connection we validated. If the user reconnected with a new
                // connection meanwhile, the hub ignores this stale id and keeps the fresh session.
                if let Err(err) = self.sender.disconnect(user_id, connection_id) {
                    log::error!("[{user_id}] Failed to send expiry disconnect command: {err:#?}");
                }
            }
        }
    }
}
