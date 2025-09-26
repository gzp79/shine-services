use crate::map::{
    ground_tile::sync_ground_tiles,
    map_chunk_render::{create_chunk_render, remove_chunk_render},
    GroundConfig, GroundLayer, GroundShard, MapChunkRenderTracker,
};
use bevy::{
    app::{App, Plugin, PostUpdate, PreUpdate, Update},
    ecs::schedule::IntoScheduleConfigs,
};
use shine_forge::map::{
    shard_channels, MapAppExt, MapLayerServerChannels, MapPreUpdateSystems, MapShardSystemConfig, ServerEmulation,
};
use shine_game::{app::GameSystems, tokio::TokeAppExt};

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        let (client, server) = shard_channels::<GroundShard>();

        app.add_map_shard::<GroundShard>(
            MapShardSystemConfig::client_with_channels(client),
            GroundConfig::new(32),
        );

        app.insert_resource(server);
        app.insert_resource(MapChunkRenderTracker::new());

        app.add_systems(PreUpdate, create_chunk_render.in_set(MapPreUpdateSystems::CreateLayers));
        app.add_systems(Update, sync_ground_tiles.in_set(GameSystems::PrepareSimulate));
        app.add_systems(PostUpdate, remove_chunk_render);
    }

    fn finish(&self, app: &mut App) {
        println!("GZP MapPlugin::finish");
        let server = app
            .world_mut()
            .remove_resource::<MapLayerServerChannels<GroundLayer>>()
            .expect("Expected server channels to exist");

        app.spawn_tokio_task(async move || ServerEmulation::new(server).run().await);
    }
}
