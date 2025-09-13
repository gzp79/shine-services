use bevy::{
    app::App,
    app::PluginGroup,
    ecs::event::Events,
    log::{self, LogPlugin},
    DefaultPlugins,
};
use serde::{Deserialize, Serialize};
use shine_forge::map::{HexConfig, MapChunkId, MapChunkRoot, MapChunkTracker, MapEvent, MapHexDenseLayerPlugin, Tile};
use shine_test::test;

mod shared;
use shared::test_init_bevy;

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct TestTile {
    pub value: u8,
}

impl Tile for TestTile {}

#[test]
async fn test_chunk_root_load_unload() {
    test_init_bevy();
    let mut app = App::new();

    // Test loading and unloading of map chunks via MapEvent.
    // Without layers only the chunk root entity is spawned and despawned.

    app.add_plugins(DefaultPlugins.build().disable::<LogPlugin>())
        .add_plugins(MapHexDenseLayerPlugin::<TestTile>::new(HexConfig::new(16)));

    app.update();

    let chunk_id: MapChunkId = MapChunkId(13, 42);
    let chunk_entity;

    // create a chunk
    app.world_mut()
        .resource_mut::<Events<MapEvent>>()
        .send(MapEvent::Load(chunk_id));

    app.update();

    {
        log::debug!("Check if chunk is created");

        let tile_map = app.world().get_resource::<MapChunkTracker>().unwrap();
        chunk_entity = tile_map.get_entity(chunk_id).unwrap();

        let chunk_root = app.world().entity(chunk_entity).components::<&MapChunkRoot>();
        assert_eq!(chunk_root.id, chunk_id);
    }

    app.world_mut()
        .resource_mut::<Events<MapEvent>>()
        .send(MapEvent::Unload(chunk_id));

    app.update();

    {
        log::debug!("Check if chunk is dropped");

        let chunk_tracker = app.world().get_resource::<MapChunkTracker>().unwrap();
        assert!(chunk_tracker.get_entity(chunk_id).is_none());

        let result = app.world().get_entity(chunk_entity);
        assert!(result.is_err());
    }
}

/*
#[test]
async fn test_map_chunk_load() {
    test_init_bevy();
    let mut app = App::new();

    // Initialize the TileMap
    app.add_plugins(DefaultPlugins)
        .add_plugins(MapPlugin::new(TestGridConfig { width: 16, height: 16 }).with_layer(TestDataLayerSetup::new()));

    app.update();

    let chunk_id: ChunkId = ChunkId(13, 42);
    let chunk_entity;

    // create a chunk
    app.world_mut()
        .resource_mut::<Events<MapEvent>>()
        .send(MapEvent::Load(chunk_id));

    app.update();

    {
        log::debug!("Check if chunk is created");

        let tile_map = app.world().get_resource::<MapChunkTracker>().unwrap();
        chunk_entity = tile_map.get_entity(chunk_id).unwrap();

        let layer = app.world().get_resource::<TestDataLayer>().unwrap();
        assert_eq!(layer.get_entity(chunk_id), Some(chunk_entity));
        assert_eq!(layer.get_chunk_id(chunk_entity), Some(chunk_id));

        let (chunk_root, chunk_version, test_data) =
            app.world()
                .entity(chunk_entity)
                .components::<(&ChunkRoot, &ChunkVersion<TestData>, &TestData)>();
        assert_eq!(chunk_root.id, chunk_id);
        assert!(test_data.is_empty());
        assert_eq!(test_data.data(), None);
        assert_eq!(chunk_version.version, 0);
    }

    app.world_mut()
        .resource_mut::<Events<MapEvent>>()
        .send(MapEvent::Unload(chunk_id));

    app.update();

    {
        log::debug!("Check if chunk is dropped");

        let chunk_tracker = app.world().get_resource::<MapChunkTracker>().unwrap();
        assert!(chunk_tracker.get_entity(chunk_id).is_none());

        let layer = app.world().get_resource::<TestDataLayer>().unwrap();
        assert!(layer.get_entity(chunk_id).is_none());

        let result = app.world().get_entity(chunk_entity);
        assert!(result.is_err());
    }
}
*/
