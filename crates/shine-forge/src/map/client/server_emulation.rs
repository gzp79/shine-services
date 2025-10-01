use crate::map::{
    MapAuditedLayer, MapLayerActionMessage, MapLayerNotificationMessage, MapLayerServerChannels, MapLayerVersion,
};
use bevy::log;
use std::collections::HashMap;

/// A simple emulation of a map server that processes actions and sends notifications.
/// It persists no state apart from versions of the tracked chunk, thus any newly tracked
/// chunk starts empty.
pub struct ServerEmulation<L>
where
    L: MapAuditedLayer,
{
    channels: MapLayerServerChannels<L>,
}

impl<L> ServerEmulation<L>
where
    L: MapAuditedLayer,
{
    pub fn new(channels: MapLayerServerChannels<L>) -> Self {
        Self { channels }
    }

    pub async fn run(self) {
        log::info!("Starting emulation server");

        let mut action_receiver = self.channels.action_receiver;
        let notification_sender = self.channels.notification_sender;

        let mut versions = HashMap::new();

        while let Some(event) = action_receiver.recv().await {
            match event {
                MapLayerActionMessage::Track(chunk_id) => {
                    log::info!("Start tracking {chunk_id:?}");
                    versions.insert(chunk_id, 0);

                    // send an empty, initial layer
                    if let Err(err) = notification_sender.send(MapLayerNotificationMessage::Initial { id: chunk_id }) {
                        log::error!("Failed to send initial notification: {err}");
                    }
                }
                MapLayerActionMessage::Untrack(chunk_id) => {
                    log::info!("Stop tracking {chunk_id:?}");
                    versions.remove(&chunk_id);
                }
                MapLayerActionMessage::Update { id, operation } => {
                    log::info!("Update {id:?} with {:?}", operation.name());
                    if let Some(version) = versions.get_mut(&id) {
                        *version += 1;
                        let version = *version;
                        if let Err(err) = notification_sender.send(MapLayerNotificationMessage::Update {
                            id,
                            version: MapLayerVersion(version),
                            operation,
                        }) {
                            log::error!("Failed to send updated notification: {err}");
                        }
                    } else {
                        log::warn!("Received update for untracked chunk {id:?}");
                    }
                }
                MapLayerActionMessage::Snapshot {
                    id,
                    version,
                    checksum,
                    snapshot,
                } => {
                    log::info!(
                        "Snapshot {:?} version {:?} checksum {:?} snapshot {:?}",
                        id,
                        version,
                        checksum,
                        snapshot.as_ref().map(|s| s.len())
                    );
                }
            }
        }
    }
}
