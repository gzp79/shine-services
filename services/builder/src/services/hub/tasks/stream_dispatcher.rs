use crate::{
    models::messages::{HubEvent, HubMessage, StreamEvent, StreamMessage, TopicKey},
    repositories::{command_inbox::redis::RedisCommandInboxDb, command_log::pg::PgCommandLogDb},
    services::{command_stream::StreamRegistry, HubService, StreamEgress},
};
use std::{sync::Arc, time::Duration};
use tokio::{task::JoinHandle, time};

/// How often the workers that have ended are forgotten.
const PURGE_INTERVAL: Duration = Duration::from_secs(60);

/// Connects the stream registry to the hub: the subscriptions the hub accepted start and release the
/// workers, and a connection that is gone releases all its streams. It reads one ordered channel of
/// lifecycle and subscription events, so a connection's subscribe is always seen before its disconnect.
pub struct StreamDispatcher;

impl StreamDispatcher {
    /// Subscribes to the hub bus and starts the dispatcher. On the hub's shutdown it has the workers
    /// persist what is waiting (for up to `shutdown_timeout`) and ends.
    pub async fn start(
        service: HubService,
        registry: StreamRegistry<PgCommandLogDb, RedisCommandInboxDb>,
        shutdown_timeout: Duration,
    ) -> JoinHandle<()> {
        // Subscribed before the hub's loop starts: an event dispatched before is lost to it for good.
        let mut subscription = service.subscribe(vec![TopicKey::Hub, TopicKey::Stream]).await;
        tokio::spawn(async move {
            let mut purge = time::interval(PURGE_INTERVAL);
            purge.set_missed_tick_behavior(time::MissedTickBehavior::Delay);
            purge.tick().await;
            loop {
                let message = tokio::select! {
                    message = subscription.recv() => match message {
                        Some(message) => message,
                        None => break,
                    },
                    _ = purge.tick() => {
                        registry.purge();
                        continue;
                    }
                };
                match message {
                    HubMessage::StreamEvent(StreamEvent::Subscribed { connection_id, stream_id, kind }) => {
                        if let Err(err) = registry.subscribe(connection_id, &stream_id, &kind) {
                            tracing::warn!("[{connection_id}] Failed to subscribe to stream {stream_id}: {err:#?}");
                            let reason = err.subscribe_failure();
                            service
                                .send(
                                    connection_id,
                                    Arc::new(StreamMessage::SubscribeFailed { stream_id, reason }),
                                )
                                .await;
                        }
                    }
                    HubMessage::StreamEvent(StreamEvent::Unsubscribed { connection_id, stream_id }) => {
                        registry.unsubscribe(connection_id, &stream_id);
                    }
                    HubMessage::Hub(HubEvent::UserDisconnected { connection_id, .. }) => {
                        registry.disconnect(connection_id);
                    }
                    HubMessage::Hub(HubEvent::Shutdown) => {
                        registry.shutdown(shutdown_timeout).await;
                        break;
                    }
                    _ => {}
                }
            }
            tracing::info!("Stream dispatcher stopped");
        })
    }
}
