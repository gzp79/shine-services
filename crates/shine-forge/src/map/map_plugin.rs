use crate::map::{map_chunk_root::MapChunkTracker, map_event::process_map_event_system, MapChunk, MapEvent, MapLayer};
use bevy::{
    app::{App, Plugin, PreUpdate},
    ecs::resource::Resource,
};

#[derive(Default)]
pub struct MapPlugin {}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapChunkTracker>();
        app.add_event::<MapEvent>();

        app.add_systems(PreUpdate, process_map_event_system);
    }
}

/// Helper function to build and register a map layer plugin with the given configuration and chunk type.
pub(in crate::map) fn build_map_layer<C, CFG>(config: CFG, app: &mut App)
where
    C: MapChunk + From<CFG>,
    CFG: Resource + Clone,
{
    if !app.is_plugin_added::<MapPlugin>() {
        app.add_plugins(MapPlugin::default());
    }

    app.insert_resource(config);
    app.insert_resource(MapLayer::<C>::default());
}
