use bevy::{app::App, ecs::event::Events, DefaultPlugins};
use shine_forge::map::{ChunkId, ChunkRoot, ChunkVersion, MapChunk, MapChunkTracker, MapEvent, MapPlugin};
use shine_test::test;

mod shared;
use shared::{test_init_bevy, TestData, TestDataLayer, TestDataLayerSetup, TestGridConfig};

#[test]
async fn test_map_events() {
    test_init_bevy();
    let mut app = App::new();

    // Initialize the TileMap
    app.add_plugins(DefaultPlugins)
        .add_plugins(MapPlugin::new(TestGridConfig { width: 16, height: 16 }));

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

        let chunk_root = app.world().entity(chunk_entity).components::<&ChunkRoot>();
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
