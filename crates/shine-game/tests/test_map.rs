use bevy::{
    app::App,
    ecs::event::Events,
    tasks::{AsyncComputeTaskPool, TaskPool},
    DefaultPlugins,
};
use shine_game::map::{ChunkId, ChunkRoot, MapChunk, MapChunkTracker, MapConfig, MapEvent, MapPlugin};
use shine_test::test;

#[path = "shared/test_data.rs"]
mod test_data;
pub use test_data::*;

#[test]
async fn test_map_events() {
    AsyncComputeTaskPool::get_or_init(TaskPool::new);
    let mut app = App::new();

    // Initialize the TileMap
    app.add_plugins(DefaultPlugins)
        .add_plugins(MapPlugin::new(MapConfig { width: 16, height: 16 }));

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
    AsyncComputeTaskPool::get_or_init(TaskPool::new);
    let mut app = App::new();

    // Initialize the TileMap
    app.add_plugins(DefaultPlugins)
        .add_plugins(MapPlugin::new(MapConfig { width: 16, height: 16 }).with_layer(TestDataLayerSetup::new()));

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

        let (chunk_root, test_data) = app.world().entity(chunk_entity).components::<(&ChunkRoot, &TestData)>();
        assert_eq!(chunk_root.id, chunk_id);
        assert!(test_data.is_empty());
        assert_eq!(test_data.data(), None);
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
