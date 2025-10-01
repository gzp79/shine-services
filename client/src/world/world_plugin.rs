use crate::world::{
    ground_tile::{debug_ground_tiles, sync_ground_tiles},
    light::spawn_light,
    map_chunk_render::{create_chunk_render, remove_chunk_render},
    GroundConfig, GroundLayer, GroundShard, MapChunkRenderTracker, WorldConfig,
};
use bevy::{
    app::{App, Plugin, Startup, Update},
    ecs::schedule::IntoScheduleConfigs,
};
use shine_forge::map::{shard_channels, MapAppExt, MapLayerServerChannels, MapShardSystemConfig, ServerEmulation};
use shine_game::{app::GameSystems, tokio::TokeAppExt};

pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        let (client, server) = shard_channels::<GroundShard>();
        let world_config = WorldConfig::new();

        app.add_map_shard::<GroundShard>(
            MapShardSystemConfig::client_with_channels(client),
            GroundConfig::new(world_config.ground_chunk_size),
        );

        app.insert_resource(world_config);
        app.insert_resource(server);
        app.insert_resource(MapChunkRenderTracker::new());

        app.add_systems(Startup, spawn_light);

        app.add_observer(create_chunk_render);
        app.add_observer(remove_chunk_render);
        app.add_systems(Update, sync_ground_tiles.in_set(GameSystems::PrepareSimulate));
        app.add_systems(Update, debug_ground_tiles.in_set(GameSystems::PrepareRender));
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
