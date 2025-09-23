use crate::map::{
    map_chunk::process_map_event,
    map_layer::{create_layer_as_child, process_layer_sync_events, remove_layer, MapLayerSystemConfig},
    MapAuditedLayer, MapChunkTracker, MapEvent, MapLayerControlEvent, MapLayerIO, MapLayerSyncEvent, MapLayerTracker,
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
    /// Register a map layer with the given configuration.
    fn add_map_layer<L>(&mut self, system_config: MapLayerSystemConfig<L>, layer_config: L::Config)
    where
        L: MapAuditedLayer + MapLayerIO;
}

impl MapAppExt for App {
    fn add_map_layer<L>(&mut self, system_config: MapLayerSystemConfig<L>, layer_config: L::Config)
    where
        L: MapAuditedLayer + MapLayerIO,
    {
        if !self.is_plugin_added::<MapPlugin>() {
            self.add_plugins(MapPlugin::default());
        }

        self.insert_resource(layer_config);
        self.insert_resource(system_config.clone());
        self.insert_resource(MapLayerTracker::<L>::default());
        self.add_event::<MapLayerControlEvent<L>>();
        self.add_event::<MapLayerSyncEvent<L>>();

        self.add_systems(
            PreUpdate,
            create_layer_as_child::<L>.in_set(MapPreUpdateSystem::CreateLayers),
        );
        if system_config.process_sync_events {
            self.add_systems(
                PreUpdate,
                process_layer_sync_events::<L>.in_set(MapPreUpdateSystem::ProcessSyncEvents),
            );
        }

        self.add_systems(PostUpdate, remove_layer::<L>);
    }
}
