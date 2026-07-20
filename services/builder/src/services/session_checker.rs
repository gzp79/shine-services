use crate::{
    models::messages::{DisconnectReason, HubBusMessage, HubCommand, TopicKey, UserEvent},
    services::{HubSender, HubService},
};
use shine_infra::session::{CurrentUserService, SessionKey};
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use uuid::Uuid;

fn apply_lifecycle_event(map: &mut HashMap<Uuid, SessionKey>, message: HubBusMessage) {
    match message {
        HubBusMessage::Hub(UserEvent::UserConnected { user_id, session_key }) => {
            map.insert(user_id, session_key);
        }
        HubBusMessage::Hub(UserEvent::UserDisconnected { user_id, .. }) => {
            map.remove(&user_id);
        }
        HubBusMessage::Chat(_) => {}
    }
}

async fn check_sessions(
    map: &Mutex<HashMap<Uuid, SessionKey>>,
    session_service: &CurrentUserService,
    sender: &HubSender,
) {
    let snapshot: Vec<(Uuid, SessionKey)> = {
        let map = map.lock().await;
        map.iter().map(|(id, key)| (*id, *key)).collect()
    };

    for (user_id, session_key) in snapshot {
        if session_service.get_current_user(user_id, session_key).await.is_err() {
            log::info!("[{user_id}] Session expired, requesting disconnect");
            if let Err(err) = sender.send_command(HubCommand::DisconnectUser {
                user_id,
                reason: DisconnectReason::SessionExpired,
            }) {
                log::error!("[{user_id}] Failed to send expiry disconnect command: {err:#?}");
            }
        }
    }
}

/// Detached process: tracks connected sessions from ConnectionLifecycle
/// events in its own local map, and periodically validates each one against
/// CurrentUserService without ever touching hub-owned state directly.
pub struct SessionChecker;

impl SessionChecker {
    pub async fn spawn(
        hub_service: &HubService,
        session_service: Arc<CurrentUserService>,
        check_interval: Duration,
    ) -> Self {
        let sender = hub_service.sender();
        let mut subscription = hub_service.subscribe(vec![TopicKey::UserEvent]).await;

        let map = Arc::new(Mutex::new(HashMap::<Uuid, SessionKey>::new()));

        let event_map = map.clone();
        tokio::spawn(async move {
            while let Some(message) = subscription.recv().await {
                let mut map = event_map.lock().await;
                apply_lifecycle_event(&mut map, message);
            }
        });

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(check_interval);
            interval.tick().await; // skip the first immediate tick
            loop {
                interval.tick().await;
                check_sessions(&map, &session_service, &sender).await;
            }
        });

        Self
    }
}
