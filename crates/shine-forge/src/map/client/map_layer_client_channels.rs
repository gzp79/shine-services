use crate::map::{MapAuditedLayer, MapLayerActionMessage, MapLayerNotificationMessage, MapShard};
use bevy::{
    ecs::{
        message::{MessageReader, MessageWriter},
        resource::Resource,
        system::{Res, ResMut},
    },
    log,
};
use tokio::sync::mpsc;

/// Channels for communicating map layer actions and notifications between bevy systems and external systems.
pub fn layer_channels<L>() -> (MapLayerClientChannels<L>, MapLayerServerChannels<L>)
where
    L: MapAuditedLayer,
{
    let (action_sender, action_receiver) = mpsc::unbounded_channel();
    let (notification_sender, notification_receiver) = mpsc::unbounded_channel();

    let client_channels = MapLayerClientChannels {
        action_sender,
        notification_receiver,
    };
    let server_channels = MapLayerServerChannels {
        action_receiver,
        notification_sender,
    };
    (client_channels, server_channels)
}

/// Channels for communicating map shard (layer) actions and notifications between bevy systems and external systems.
pub fn shard_channels<S>() -> (MapLayerClientChannels<S::Primary>, MapLayerServerChannels<S::Primary>)
where
    S: MapShard,
{
    layer_channels::<S::Primary>()
}

#[derive(Resource)]
pub struct MapLayerClientChannels<L>
where
    L: MapAuditedLayer,
{
    /// Bevy -> External System
    pub action_sender: mpsc::UnboundedSender<MapLayerActionMessage<L>>,
    /// External System -> Bevy
    pub notification_receiver: mpsc::UnboundedReceiver<MapLayerNotificationMessage<L>>,
}

#[derive(Resource)]
pub struct MapLayerServerChannels<L>
where
    L: MapAuditedLayer,
{
    /// External System -> Bevy
    pub action_receiver: mpsc::UnboundedReceiver<MapLayerActionMessage<L>>,
    /// Bevy -> External System
    pub notification_sender: mpsc::UnboundedSender<MapLayerNotificationMessage<L>>,
}

/// System forwarding MapLayerActionEvent to external systems.
pub fn forward_action_events_to_channel<L>(
    mut action_events: MessageReader<MapLayerActionMessage<L>>,
    channels: Res<MapLayerClientChannels<L>>,
) where
    L: MapAuditedLayer,
{
    for event in action_events.read() {
        if let Err(e) = channels.action_sender.send(event.clone()) {
            log::error!("Failed to send action event to client: {}", e);
        }
    }
}

/// System forwarding the external MapLayerNotificationEvent to the Bevy event system.
pub fn receive_notification_events_from_channel<L>(
    mut channels: ResMut<MapLayerClientChannels<L>>,
    mut notification_events: MessageWriter<MapLayerNotificationMessage<L>>,
) where
    L: MapAuditedLayer,
{
    while let Ok(event) = channels.notification_receiver.try_recv() {
        log::debug!("Received notification event from channel: {:?}", event);
        notification_events.write(event);
    }
}
