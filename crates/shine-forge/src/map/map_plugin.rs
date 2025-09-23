use crate::map::{
    map_chunk::process_map_event,
    map_shard::{create_shard, process_shard_sync_events, remove_shard},
    MapChunkTracker, MapEvent, MapLayerControlEvent, MapLayerSyncEvent, MapLayerTracker, MapShard,
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
    ProcessSyncEvents,
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
                MapPreUpdateSystem::ProcessSyncEvents,
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
        self.add_event::<MapLayerControlEvent<S::Primary>>();
        self.add_event::<MapLayerSyncEvent<S::Primary>>();

        self.add_systems(PreUpdate, create_shard::<S>.in_set(MapPreUpdateSystem::CreateLayers));
        if system_config.process_sync_events {
            self.add_systems(
                PreUpdate,
                process_shard_sync_events::<S>.in_set(MapPreUpdateSystem::ProcessSyncEvents),
            );
        }

        self.add_systems(PostUpdate, remove_shard::<S>);
    }
}
