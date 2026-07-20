use crate::{
    models::messages::{HubCommand, HubEvent, HubMessage, TopicKey},
    services::{HubSender, HubService},
};
use shine_infra::session::{CurrentUserService, SessionKey};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    sync::{watch, Mutex},
    time,
};
use uuid::Uuid;

enum LifecycleUpdate {
    SessionsChanged,
    Noop,
    Shutdown,
}

fn apply_lifecycle_event(map: &mut HashMap<Uuid, SessionKey>, message: HubMessage) -> LifecycleUpdate {
    match message {
        HubMessage::Hub(HubEvent::UserConnected { user_id, session_key }) => {
            map.insert(user_id, session_key);
            LifecycleUpdate::SessionsChanged
        }
        HubMessage::Hub(HubEvent::UserDisconnected { user_id, .. }) => {
            map.remove(&user_id);
            LifecycleUpdate::SessionsChanged
        }
        HubMessage::Hub(HubEvent::Shutdown) => LifecycleUpdate::Shutdown,
        _ => LifecycleUpdate::Noop,
    }
}

async fn check_sessions(snapshot: &[(Uuid, SessionKey)], session_service: &CurrentUserService, sender: &HubSender) {
    for (user_id, session_key) in snapshot.iter().copied() {
        if session_service.get_current_user(user_id, session_key).await.is_err() {
            log::info!("[{user_id}] Session expired, requesting disconnect");
            if let Err(err) = sender.send_command(HubCommand::DisconnectUser { user_id, session_key }) {
                log::error!("[{user_id}] Failed to send expiry disconnect command: {err:#?}");
            }
        }
    }
}

async fn refresh_snapshot(map: &Mutex<HashMap<Uuid, SessionKey>>, snapshot: &mut Vec<(Uuid, SessionKey)>) {
    snapshot.clear();
    let map = map.lock().await;
    snapshot.extend(map.iter().map(|(id, key)| (*id, *key)));
}

/// Detached process: tracks connected sessions from ConnectionLifecycle
/// events in its own local map, and periodically validates each one against
/// CurrentUserService without ever touching hub-owned state directly.
pub struct SessionChecker {
    session_service: Arc<CurrentUserService>,
    hub_service: HubService,
    check_interval: Duration,
}

impl SessionChecker {
    pub fn new(session_service: Arc<CurrentUserService>, hub_service: &HubService, check_interval: Duration) -> Self {
        Self {
            session_service,
            hub_service: hub_service.clone(),
            check_interval,
        }
    }

    pub async fn spawn(self) {
        let sender = self.hub_service.sender();
        let mut subscription = self.hub_service.subscribe(vec![TopicKey::Hub]).await;

        let map = Arc::new(Mutex::new(HashMap::<Uuid, SessionKey>::new()));
        let sessions_dirty = Arc::new(AtomicBool::new(true));
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

        let event_map = map.clone();
        let event_dirty = Arc::clone(&sessions_dirty);
        let event_shutdown_tx = shutdown_tx.clone();
        tokio::spawn(async move {
            while let Some(message) = subscription.recv().await {
                let mut map = event_map.lock().await;
                match apply_lifecycle_event(&mut map, message) {
                    LifecycleUpdate::SessionsChanged => {
                        event_dirty.store(true, Ordering::Release);
                    }
                    LifecycleUpdate::Shutdown => {
                        let _ = event_shutdown_tx.send(true);
                        break;
                    }
                    LifecycleUpdate::Noop => {}
                }
            }
        });

        let check_map = map;
        let check_dirty = Arc::clone(&sessions_dirty);
        tokio::spawn(async move {
            let mut interval = time::interval(self.check_interval);
            let mut snapshot = Vec::<(Uuid, SessionKey)>::new();
            interval.tick().await; // skip the first immediate tick
            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    _ = interval.tick() => {}
                }

                if check_dirty.swap(false, Ordering::AcqRel) {
                    refresh_snapshot(&check_map, &mut snapshot).await;
                }
                check_sessions(&snapshot, &self.session_service, &sender).await;
            }
        });
    }
}
