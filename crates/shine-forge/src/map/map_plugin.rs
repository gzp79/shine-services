use crate::map::{
    client::{forward_action_events_to_channel, receive_notification_events_from_channel},
    map_chunk::process_map_messages,
    map_shard::{create_shard, process_shard_notification_messages, remove_shard},
    MapChunkTracker, MapLayerActionMessage, MapLayerNotificationMessage, MapLayerTracker, MapMessage, MapShard,
    MapShardSystemConfig,
};
use bevy::{
    app::{App, Plugin, PreUpdate},
    ecs::schedule::{IntoScheduleConfigs, SystemSet},
};

#[derive(SystemSet, Clone, Hash, Debug, PartialEq, Eq)]
pub enum MapPreUpdateSystems {
    ProcessMapEvents,
    CreateLayers,
    InjectNotifications,
    ProcessNotifications,
    ExtractActions,
}

#[derive(Default)]
pub struct MapPlugin {}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapChunkTracker>();
        app.add_message::<MapMessage>();

        app.configure_sets(
            PreUpdate,
            (
                MapPreUpdateSystems::ProcessMapEvents,
                MapPreUpdateSystems::CreateLayers,
                MapPreUpdateSystems::InjectNotifications,
                MapPreUpdateSystems::ProcessNotifications,
                MapPreUpdateSystems::ExtractActions,
            )
                .chain(),
        );

        app.add_systems(
            PreUpdate,
            process_map_messages.in_set(MapPreUpdateSystems::ProcessMapEvents),
        );
    }
}

pub trait MapAppExt {
    fn add_map_shard<S>(&mut self, system_config: MapShardSystemConfig<S>, layer_config: S::Config)
    where
        S: MapShard;
}

impl MapAppExt for App {
    fn add_map_shard<S>(&mut self, system_config: MapShardSystemConfig<S>, layer_config: S::Config)
    where
        S: MapShard,
    {
        if !self.is_plugin_added::<MapPlugin>() {
            self.add_plugins(MapPlugin::default());
        }

        self.insert_resource(layer_config);
        self.insert_resource(system_config.clone());
        self.insert_resource(MapLayerTracker::<S::Primary>::default());
        self.add_message::<MapLayerActionMessage<S::Primary>>();
        self.add_message::<MapLayerNotificationMessage<S::Primary>>();

        self.add_observer(create_shard::<S>);
        self.add_observer(remove_shard::<S>);

        self.add_systems(
            PreUpdate,
            process_shard_notification_messages::<S>.in_set(MapPreUpdateSystems::ProcessNotifications),
        );

        if let Some(channels) = system_config.client_channels {
            self.insert_resource(channels);

            self.add_systems(
                PreUpdate,
                (
                    forward_action_events_to_channel::<S::Primary>.in_set(MapPreUpdateSystems::InjectNotifications),
                    receive_notification_events_from_channel::<S::Primary>.in_set(MapPreUpdateSystems::ExtractActions),
                ),
            );
        }
    }
}
