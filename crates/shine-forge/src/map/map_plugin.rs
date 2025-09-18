use crate::map::{
    map_chunk::process_map_event,
    map_layer::{create_layer_as_child, process_layer_sync_events, remove_layer},
    MapChunkTracker, MapEvent, MapLayer, MapLayerControlEvent, MapLayerIO, MapLayerSyncEvent, MapLayerTracker,
};
use bevy::{
    app::{App, Plugin, PostUpdate, PreUpdate},
    ecs::{
        resource::Resource,
        schedule::{IntoScheduleConfigs, SystemSet},
    },
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
    /// Helper to register a map layer with the given configuration.
    fn add_map_layer<C, CFG>(&mut self, config: CFG)
    where
        C: MapLayer + From<CFG>,
        CFG: Resource + Clone;

    /// Helper to enable sync event processing for a layer.
    fn add_map_sync_event_processing<C>(&mut self)
    where
        C: MapLayer + MapLayerIO;
}

impl MapAppExt for App {
    fn add_map_layer<C, CFG>(&mut self, config: CFG)
    where
        C: MapLayer + From<CFG>,
        CFG: Resource + Clone,
    {
        if !self.is_plugin_added::<MapPlugin>() {
            self.add_plugins(MapPlugin::default());
        }

        self.insert_resource(config);
        self.insert_resource(MapLayerTracker::<C>::default());
        self.add_event::<MapLayerControlEvent<C>>();
        self.add_event::<MapLayerSyncEvent<C>>();

        self.add_systems(
            PreUpdate,
            create_layer_as_child::<CFG, C>.in_set(MapPreUpdateSystem::CreateLayers),
        );
        self.add_systems(PostUpdate, remove_layer::<C>);
    }

    /// Helper to enable sync event processing for a layer.
    fn add_map_sync_event_processing<C>(&mut self)
    where
        C: MapLayer + MapLayerIO,
    {
        self.add_systems(
            PreUpdate,
            process_layer_sync_events::<C>.in_set(MapPreUpdateSystem::ProcessSyncEvents),
        );
    }
}
