use crate::map::{GroundConfig, GroundLayer, GroundShard};
use bevy::app::{App, Plugin};
use shine_forge::map::{shard_channels, MapAppExt, MapLayerServerChannels, MapShardSystemConfig, ServerEmulation};

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        let (client, server) = shard_channels::<GroundShard>();

        app.add_map_shard::<GroundShard>(
            MapShardSystemConfig::client_with_channels(client),
            GroundConfig::new(32),
        );
        app.insert_resource(server);
    }

    fn finish(&self, app: &mut App) {
        let server = app
            .world_mut()
            .remove_resource::<MapLayerServerChannels<GroundLayer>>()
            .expect("Expected server channels to exist");
        ServerEmulation::new(server).run();
    }
}
