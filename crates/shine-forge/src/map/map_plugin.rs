use crate::map::{
    map_chunk::process_map_event,
    map_layer::{create_layer_as_child, process_layer_sync_events, remove_layer},
    MapChunkTracker, MapEvent, MapLayer, MapLayerControlEvent, MapLayerSyncEvent, MapLayerTracker,
};
use bevy::{
    app::{App, Plugin, PreUpdate},
    ecs::{resource::Resource, schedule::IntoScheduleConfigs},
};

#[derive(Default)]
pub struct MapPlugin {}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapChunkTracker>();
        app.add_event::<MapEvent>();

        app.add_systems(PreUpdate, process_map_event);
    }
}

/// Helper to build and register a map layer with the given configuration.
pub fn build_map_layer<C, CFG>(config: CFG, app: &mut App)
where
    C: MapLayer + From<CFG>,
    CFG: Resource + Clone,
{
    if !app.is_plugin_added::<MapPlugin>() {
        app.add_plugins(MapPlugin::default());
    }

    app.insert_resource(config);
    app.insert_resource(MapLayerTracker::<C>::default());
    app.add_event::<MapLayerControlEvent<C>>();
    app.add_event::<MapLayerSyncEvent<C>>();

    app.add_systems(
        PreUpdate,
        (create_layer_as_child::<CFG, C>, process_layer_sync_events::<C>)
            .chain()
            .after(process_map_event),
    );

    app.add_systems(PreUpdate, remove_layer::<C>);
}
