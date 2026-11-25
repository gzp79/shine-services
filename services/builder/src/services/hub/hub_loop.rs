use crate::{
    models::messages::{DisconnectReason, HubEvent, HubMessage, StreamEvent, StreamMessage, SubscribeFailure},
    repositories::{command_inbox::redis::RedisCommandInboxDb, command_log::pg::PgCommandLogDb},
    services::{
        command_stream::StreamRegistry,
        hub::{
            control_command::ControlCommand,
            hub_connections::{Connections, UnregisteredConnection},
            hub_gateway::{ControlMessage, HubGateway},
            hub_metrics::hub_metrics,
            hub_registry_service::HubRegistryService,
            hub_subscribers::Subscribers,
            tasks::BackgroundJoinHandler,
        },
        StreamEgress,
    },
};
use futures::{stream, StreamExt};
use shine_core::tokio::TaskSupervisor;
use shine_infra::telemetry::link_span;
use std::{sync::Arc, time::Duration, time::Instant};
use tokio::sync::{mpsc, watch};
use tracing::{info_span, Instrument};
use uuid::Uuid;

/// Registry rows removed at the same time on shutdown.
const SHUTDOWN_REGISTRY_CONCURRENCY: usize = 32;

/// Longest the registry cleanup of a shutdown takes. The rows left behind expire with their TTL.
const SHUTDOWN_REGISTRY_TIMEOUT: Duration = Duration::from_secs(5);

/// The command loop of the hub, the sole writer of connection state.
pub struct HubLoop {
    gateway: HubGateway,
    subscribers: Subscribers,
    hub_registry: HubRegistryService,
    stream_registry: Option<StreamRegistry<RedisCommandInboxDb, PgCommandLogDb>>,
    supervisor: TaskSupervisor,
}

impl HubLoop {
    /// The connections: this loop is their only writer.
    fn connections(&self) -> &Connections {
        self.gateway.connections()
    }

    /// Logs and counts a connection removed by the hub, and tells its socket why. The reason is the one the
    /// socket was told, which differs from `reason` when something ended the connection first.
    fn record_drop(&self, removed: UnregisteredConnection, reason: DisconnectReason) -> (Uuid, Uuid, DisconnectReason) {
        let (user_id, connection_id) = (removed.user_id, removed.connection_id);
        let reason = removed.close(reason);
        tracing::info!("[{user_id}] [{connection_id}] Connection dropped: {reason}");
        hub_metrics().connection_dropped(reason);
        (user_id, connection_id, reason)
    }

    /// Starts the command loop, the sole writer of connection state. It takes the consumers that run beside it and
    /// stops them when it ends. Returns the signal that is set once everything has stopped.
    pub(super) fn start(
        gateway: HubGateway,
        mut control_receiver: mpsc::UnboundedReceiver<ControlMessage>,
        subscribers: Subscribers,
        hub_registry: HubRegistryService,
        stream_registry: Option<StreamRegistry<RedisCommandInboxDb, PgCommandLogDb>>,
        supervisor: TaskSupervisor,
        background_tasks: Vec<BackgroundJoinHandler>,
    ) -> watch::Receiver<bool> {
        let (shutdown_complete, shutdown_complete_rx) = watch::channel(false);
        let hub_loop = HubLoop {
            gateway,
            subscribers,
            hub_registry,
            stream_registry,
            supervisor: supervisor.clone(),
        };
        supervisor.spawn("hub command loop", async move {
            while let Some((command, origin, queued_at)) = control_receiver.recv().await {
                let name = command.name();
                let metrics = hub_metrics();
                metrics.control_command_started(name, queued_at.elapsed());

                // Linked, not parented: the queued command must not keep the originating request span open.
                let span = info_span!(
                    parent: None,
                    "hub command",
                    hub.command = name,
                    enduser.id = command.user_id().map(display),
                    ws.connection_id = command.connection_id().map(display)
                );
                link_span(&span, origin);
                let start = Instant::now();
                let keep_running = hub_loop.process(command).instrument(span).await;
                metrics.control_command_finished(name, start.elapsed());
                if !keep_running {
                    break;
                }
            }
            tracing::info!("Hub command loop stopped");
            for task in &background_tasks {
                task.request_stop();
            }
            for task in background_tasks {
                task.join().await;
            }
            drop(control_receiver);
            shutdown_complete.send_replace(true);
        });
        shutdown_complete_rx
    }

    /// Processes one control command. Returns if the dispatcher should continue (true) or exit (false).
    async fn process(&self, command: ControlCommand) -> bool {
        tracing::debug!("Processing command: {command:#?}");
        match command {
            ControlCommand::ConnectUser {
                user_id,
                connection_id,
                session_key,
                socket_sender,
                topics,
            } => {
                // Record the replacement in the registry first. If this fails, the previously
                // active connection must stay intact and tracked, so return before tearing it down
                // — otherwise a transient Redis error would leave the user with no connection and
                // an orphaned, untracked new socket.
                if let Err(err) = self.hub_registry.create_connection(user_id, connection_id).await {
                    // The write is not atomic with its reply: it may have landed although the call
                    // failed (e.g. a lost response). Then the registry already names the new
                    // connection, and keeping the old one would only get it dropped as inactive.
                    if !self.hub_registry.is_active(user_id, connection_id).await {
                        tracing::error!("[{user_id}] Failed to create hub connection: {err:#?}");
                        return true;
                    }
                    tracing::warn!("[{user_id}] Hub connection registered despite an error: {err:#?}");
                }

                // Now that the new connection is durably registered, tear down any different
                // connection that was active for this user, so the old socket closes and the
                // connection tracker drops it. Same-id re-registration is not a replacement.
                if let Some((old_connection_id, _)) = self.connections().find_active(user_id).await {
                    if old_connection_id != connection_id {
                        if let Some(removed) = self.connections().unregister(user_id, Some(old_connection_id)).await {
                            let (user_id, connection_id, reason) =
                                self.record_drop(removed, DisconnectReason::Replaced);
                            self.publish(HubEvent::UserDisconnected { user_id, connection_id, reason })
                                .await;
                        }
                    }
                }

                self.connections()
                    .register(user_id, connection_id, session_key, socket_sender, topics)
                    .await;
                self.publish(HubEvent::UserConnected {
                    user_id,
                    connection_id,
                    session_key,
                })
                .await;
                true
            }
            ControlCommand::DisconnectUser { user_id, connection_id, reason } => {
                // Only removes the entry when `connection_id` still matches the active
                // connection, so a stale disconnect can never tear down a fresh reconnect.
                let Some(removed) = self.connections().unregister(user_id, Some(connection_id)).await else {
                    return true;
                };

                // A client-requested disconnect is not a drop; its socket is already closing.
                let (removed_connection_id, reason) = if reason.is_drop() {
                    let (_, connection_id, reason) = self.record_drop(removed, reason);
                    (connection_id, reason)
                } else {
                    (removed.connection_id, removed.close(reason))
                };
                if let Err(err) = self
                    .hub_registry
                    .remove_connection(user_id, removed_connection_id)
                    .await
                {
                    // Best-effort: publish UserDisconnected regardless so the socket and trackers
                    // never wedge, and let the registry TTL clean up the stale row.
                    tracing::error!("[{user_id}] Failed to remove hub connection (relying on TTL cleanup): {err:#?}");
                }

                self.publish(HubEvent::UserDisconnected {
                    user_id,
                    connection_id: removed_connection_id,
                    reason,
                })
                .await;
                true
            }
            ControlCommand::HubRegistryChanged { user_id } => {
                self.process_registry_change(user_id).await;
                true
            }
            ControlCommand::SubscribeStream { connection_id, stream_id } => {
                // A subscribe that comes after the connection is gone must not start anything.
                if self.connections().contains(connection_id).await {
                    self.publish_internal(StreamEvent::Subscribed { connection_id, stream_id })
                        .await;
                } else if let Some(registry) = &self.stream_registry {
                    // It was opened after the connection was released: close it again.
                    registry.disconnect(connection_id);
                }
                true
            }
            ControlCommand::UnsubscribeStream { connection_id, stream_id } => {
                if self.connections().contains(connection_id).await {
                    self.publish_internal(StreamEvent::Unsubscribed { connection_id, stream_id })
                        .await;
                }
                true
            }
            ControlCommand::StreamFailed { stream_id } => {
                // The registry is closed first: a client that subscribes again on the failure finds it clean.
                let connections = self
                    .stream_registry
                    .as_ref()
                    .map(|registry| registry.close_stream(&stream_id))
                    .unwrap_or_default();
                for connection_id in connections {
                    self.publish_internal(StreamEvent::Unsubscribed {
                        connection_id,
                        stream_id: stream_id.clone(),
                    })
                    .await;
                    let message = StreamMessage::SubscribeFailed {
                        stream_id: stream_id.clone(),
                        reason: SubscribeFailure::Failed,
                    };
                    StreamEgress::send(&self.gateway, connection_id, Arc::new(message)).await;
                }
                true
            }
            ControlCommand::Shutdown => {
                self.supervisor.shutdown_started();
                self.shutdown().await;
                false
            }
        }
    }

    async fn process_registry_change(&self, user_id: Uuid) {
        let Some((connection_id, _session_key)) = self.connections().find_active(user_id).await else {
            return;
        };

        let is_active = match self.hub_registry.heartbeat_connection(user_id, connection_id).await {
            Ok(is_active) => is_active,
            Err(err) => {
                tracing::error!("[{user_id}] Failed to heartbeat hub connection: {err:#?}");
                return;
            }
        };

        if is_active {
            return;
        }

        // Force removal of the connection we just found to be inactive in the registry. If a
        // fresh reconnect replaced it in the meantime, the connection ids differ and we leave
        // the new connection untouched.
        let Some(removed) = self.connections().unregister(user_id, Some(connection_id)).await else {
            return;
        };

        let (user_id, connection_id, reason) = self.record_drop(removed, DisconnectReason::RegistryInactive);
        self.publish(HubEvent::UserDisconnected { user_id, connection_id, reason })
            .await;
    }

    /// Stops the internal consumers, then drops every connection. Nothing is published after the
    /// Shutdown event: the consumers are gone, and the sockets are told directly.
    async fn shutdown(&self) {
        self.subscribers.publish(HubMessage::Hub(HubEvent::Shutdown)).await;
        self.subscribers.clear().await;

        // The sockets are told first, whatever state the registry is in.
        let dropped: Vec<(Uuid, Uuid)> = self
            .connections()
            .unregister_all()
            .await
            .into_iter()
            .map(|removed| {
                let (user_id, connection_id, _) = self.record_drop(removed, DisconnectReason::Shutdown);
                (user_id, connection_id)
            })
            .collect();

        // Side by side and within a time limit: a slow registry would otherwise cost every connection its
        // own wait one after the other, and the process may be killed before it is done.
        let cleanup = async {
            if let Err(err) = self.hub_registry.unlisten().await {
                tracing::warn!("Failed to stop listening to hub registry changes: {err:#?}");
            }
            stream::iter(dropped)
                .for_each_concurrent(SHUTDOWN_REGISTRY_CONCURRENCY, |(user_id, connection_id)| async move {
                    if let Err(err) = self.hub_registry.remove_connection(user_id, connection_id).await {
                        tracing::warn!(
                            "[{user_id}] Failed to remove hub connection on shutdown (TTL will clean up): {err:#?}"
                        );
                    }
                })
                .await;
        };
        if tokio::time::timeout(SHUTDOWN_REGISTRY_TIMEOUT, cleanup).await.is_err() {
            tracing::warn!(
                "The registry cleanup did not finish within {SHUTDOWN_REGISTRY_TIMEOUT:?}, TTL will clean up"
            );
        }
    }

    /// Delivers a stream event to the internal subscribers only: subscribing and unsubscribing must never
    /// load the sockets' queues.
    async fn publish_internal(&self, event: StreamEvent) {
        self.subscribers.publish(HubMessage::StreamEvent(event)).await;
    }

    /// Delivers a lifecycle `HubEvent` to every internal subscriber and every topic-matching connection. A
    /// connection found dead is requested through the gateway, and removed by its own command.
    async fn publish(&self, event: HubEvent) {
        if let HubEvent::UserDisconnected { connection_id, .. } = &event {
            if let Some(registry) = &self.stream_registry {
                registry.disconnect(*connection_id);
            }
        }
        let message = HubMessage::Hub(event);
        // Internal subscribers first: only the loop delivers to them.
        self.subscribers.publish(message.clone()).await;

        if let Err(err) = self.gateway.broadcast(message).await {
            tracing::warn!("Failed to queue the removal of a dead connection (the hub is stopping): {err:#?}");
        }
    }
}
