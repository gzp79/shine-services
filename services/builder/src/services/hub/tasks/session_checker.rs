use super::connection_tracker::{spawn_connection_loop, ConnectionConsumer, ConnectionTracker};
use crate::{models::messages::DisconnectReason, services::HubService};
use futures::{stream, StreamExt};
use shine_infra::session::{CurrentUserService, SessionKey, UserSessionError};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Session lookups in flight at once. The lookups are independent Redis round trips, a sequential pass
/// would take `connections * latency` and could outlast the check interval.
const CHECK_CONCURRENCY: usize = 32;

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
        let connections: Vec<_> = tracker
            .connections()
            .iter()
            .map(|(user_id, (connection_id, session_key))| (*user_id, *connection_id, *session_key))
            .collect();

        let session_service = &self.session_service;
        let verdicts = check_sessions(connections, CHECK_CONCURRENCY, |user_id, session_key| async move {
            session_service.get_current_user(user_id, session_key).await.map(|_| ())
        })
        .await;

        let mut skipped = 0usize;
        let mut first_skipped = None;
        for (user_id, connection_id, verdict) in verdicts {
            match verdict {
                // Session is valid; keep the connection.
                Ok(()) => {}
                // The session is definitively gone/invalid: close the connection.
                Err(err @ (UserSessionError::SessionExpired | UserSessionError::SessionCompromised)) => {
                    tracing::info!("[{user_id}] session-invalid ({err}); requesting hub disconnect");
                    // Target the exact connection we validated. If the user reconnected with a new
                    // connection meanwhile, the hub ignores this stale id and keeps the fresh session.
                    if let Err(err) =
                        self.hub_service
                            .request_drop(user_id, connection_id, DisconnectReason::SessionInvalid)
                    {
                        tracing::error!("[{user_id}] Failed to send expiry disconnect command: {err:#?}");
                    }
                }
                // Transient/infrastructure errors (Redis unavailable, pool exhausted, ...): skip, the next tick
                // checks again. Reported once per tick, an outage would otherwise log once per connection.
                Err(err) => {
                    skipped += 1;
                    first_skipped.get_or_insert(err);
                }
            }
        }
        if let Some(err) = first_skipped {
            tracing::warn!("session check skipped for {skipped} connections due to transient errors, first: {err:#?}");
        }
    }
}

/// Checks the session of each connection with at most `concurrency` lookups in flight. The results come back
/// in completion order, tagged with the connection they belong to.
async fn check_sessions<F, Fut>(
    connections: Vec<(Uuid, Uuid, SessionKey)>,
    concurrency: usize,
    check: F,
) -> Vec<(Uuid, Uuid, Result<(), UserSessionError>)>
where
    F: Fn(Uuid, SessionKey) -> Fut,
    Fut: Future<Output = Result<(), UserSessionError>>,
{
    let check = &check;
    stream::iter(connections)
        .map(|(user_id, connection_id, session_key)| async move {
            (user_id, connection_id, check(user_id, session_key).await)
        })
        .buffer_unordered(concurrency)
        .collect()
        .await
}

#[cfg(test)]
#[path = "session_checker_test.rs"]
mod session_checker_test;
