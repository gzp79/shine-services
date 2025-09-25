use crate::map::{
    client::{forward_action_events_to_channel, receive_notification_events_from_channel},
    map_chunk::process_map_event,
    map_shard::{create_shard, process_shard_notification_events, remove_shard},
    MapChunkTracker, MapEvent, MapLayerActionEvent, MapLayerNotificationEvent, MapLayerTracker, MapShard,
    MapShardSystemConfig,
};
use bevy::{
    app::{App, Plugin, PostUpdate, PreUpdate},
    ecs::schedule::{IntoScheduleConfigs, SystemSet},
};

#[derive(SystemSet, Clone, Hash, Debug, PartialEq, Eq)]
pub enum MapPreUpdateSystem {
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
        app.add_event::<MapEvent>();

        app.configure_sets(
            PreUpdate,
            (
                MapPreUpdateSystem::ProcessMapEvents,
                MapPreUpdateSystem::CreateLayers,
                MapPreUpdateSystem::InjectNotifications,
                MapPreUpdateSystem::ProcessNotifications,
                MapPreUpdateSystem::ExtractActions,
            )
                .chain(),
        );

        app.add_systems(
            PreUpdate,
            process_map_event.in_set(MapPreUpdateSystem::ProcessMapEvents),
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
        self.add_event::<MapLayerActionEvent<S::Primary>>();
        self.add_event::<MapLayerNotificationEvent<S::Primary>>();

        self.add_systems(
            PreUpdate,
            (
                create_shard::<S>.in_set(MapPreUpdateSystem::CreateLayers),
                process_shard_notification_events::<S>.in_set(MapPreUpdateSystem::ProcessNotifications),
            ),
        );
        self.add_systems(PostUpdate, remove_shard::<S>);

        if let Some(channels) = system_config.client_channels {
            self.insert_resource(channels);

            self.add_systems(
                PreUpdate,
                (
                    forward_action_events_to_channel::<S::Primary>.in_set(MapPreUpdateSystem::InjectNotifications),
                    receive_notification_events_from_channel::<S::Primary>.in_set(MapPreUpdateSystem::ExtractActions),
                ),
            );
        }
    }
}
