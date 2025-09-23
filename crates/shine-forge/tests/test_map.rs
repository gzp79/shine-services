use bevy::{
    app::{App, PluginGroup},
    ecs::event::Events,
    log::{self, LogPlugin},
    DefaultPlugins,
};
use serde::{Deserialize, Serialize};
use shine_forge::map::{
    HexLayer, HexShard, MapChunk, MapChunkId, MapChunkTracker, MapEvent, MapLayer, MapLayerControlEvent,
    MapLayerTracker, MapShard, Tile,
};
use shine_test::test;

mod shared;
use shared::test_init_bevy;

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
pub struct TestTile {
    pub value: u8,
}

pub type TestShard = HexShard<TestTile>;
pub type TestLayer = <TestShard as MapShard>::Primary;
pub type TestLayerTracker = MapLayerTracker<TestLayer>;
pub type TestOverlayLayer = <TestShard as MapShard>::Overlay;
pub type TestAuditLayer = <TestShard as MapShard>::Audit;

impl Tile for TestTile {}

#[test]
async fn test_server_shard_lifecycle() {
    test_init_bevy();
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.build().disable::<LogPlugin>())
        .add_plugins(TestShard::server(16));

    app.update();

    let chunk_id: MapChunkId = MapChunkId(13, 42);

    for i in 0..2 {
        log::debug!("Pass {i} ...");

        let chunk_root_entity;
        let chunk_entity;

        // create a chunk
        log::debug!("Send load event for chunk {chunk_id:?}");
        app.world_mut()
            .resource_mut::<Events<MapEvent>>()
            .send(MapEvent::Load(chunk_id));

        app.update();

        {
            log::debug!("Check if chunk root is created");
            let tile_map = app.world().get_resource::<MapChunkTracker>().unwrap();
            chunk_root_entity = tile_map.get_entity(chunk_id).unwrap();
            let chunk_root = app.world().entity(chunk_root_entity).components::<&MapChunk>();
            assert_eq!(chunk_root.id, chunk_id);

            log::debug!("Check if shard entity and components are created");
            let tile_map = app.world().get_resource::<TestLayerTracker>().unwrap();
            chunk_entity = tile_map.get_entity(chunk_id).unwrap();

            let chunk_entity_ref = app.world().entity(chunk_entity);
            assert_eq!(chunk_entity_ref.components::<&TestLayer>().radius(), 16);
            assert!(chunk_entity_ref.get_components::<&TestAuditLayer>().is_none());
            assert!(chunk_entity_ref.get_components::<&TestOverlayLayer>().is_none());

            log::debug!("Check control events");
            let mut events = app
                .world_mut()
                .resource_mut::<Events<MapLayerControlEvent<TestLayer>>>();
            let mut event_reader = events.get_cursor();
            match event_reader.read(&*events).next() {
                Some(MapLayerControlEvent::Track(id, _)) => {
                    assert_eq!(id, &chunk_id);
                }
                other => panic!("Unexpected event, got: {other:?}"),
            }
            assert!(event_reader.read(&*events).next().is_none());
            events.clear();
        }

        log::debug!("Send unload event for chunk {chunk_id:?}");
        app.world_mut()
            .resource_mut::<Events<MapEvent>>()
            .send(MapEvent::Unload(chunk_id));

        // complete cleanup happens in 2 updates
        app.update();
        app.update();

        {
            log::debug!("Check if chunk root is dropped");
            let chunk_tracker = app.world().get_resource::<MapChunkTracker>().unwrap();
            assert!(chunk_tracker.get_entity(chunk_id).is_none());
            assert!(app.world().get_entity(chunk_root_entity).is_err());

            log::debug!("Check if shard entity and components are dropped");
            let tile_map = app.world().get_resource::<TestLayerTracker>().unwrap();
            assert_eq!(tile_map.get_entity(chunk_id), None);
            assert_eq!(tile_map.get_chunk_id(chunk_entity), None);
            assert!(app.world().get_entity(chunk_entity).is_err());

            log::debug!("Check control events");
            let mut events = app
                .world_mut()
                .resource_mut::<Events<MapLayerControlEvent<TestLayer>>>();
            let mut event_reader = events.get_cursor();
            match event_reader.read(&*events).next() {
                Some(MapLayerControlEvent::Untrack(id)) => {
                    assert_eq!(id, &chunk_id);
                }
                other => panic!("Unexpected event, got: {other:?}"),
            }
            assert!(event_reader.read(&*events).next().is_none());
            events.clear();
        }
    }
}

#[test]
async fn test_client_shard_lifecycle() {
    test_init_bevy();
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.build().disable::<LogPlugin>())
        .add_plugins(TestShard::client(16));

    app.update();

    let chunk_id: MapChunkId = MapChunkId(13, 42);

    for i in 0..2 {
        log::debug!("Pass {i} ...");

        let chunk_root_entity;
        let chunk_entity;

        // create a chunk
        log::debug!("Send load event for chunk {chunk_id:?}");
        app.world_mut()
            .resource_mut::<Events<MapEvent>>()
            .send(MapEvent::Load(chunk_id));

        app.update();

        {
            log::debug!("Check if chunk root is created");
            let tile_map = app.world().get_resource::<MapChunkTracker>().unwrap();
            chunk_root_entity = tile_map.get_entity(chunk_id).unwrap();
            let chunk_root = app.world().entity(chunk_root_entity).components::<&MapChunk>();
            assert_eq!(chunk_root.id, chunk_id);

            log::debug!("Check if shard entity and components are created");
            let tile_map = app.world().get_resource::<TestLayerTracker>().unwrap();
            chunk_entity = tile_map.get_entity(chunk_id).unwrap();

            let chunk_entity_ref = app.world().entity(chunk_entity);
            assert!(chunk_entity_ref.components::<&TestLayer>().is_empty());
            assert!(chunk_entity_ref.components::<&TestAuditLayer>().is_empty());
            assert!(chunk_entity_ref.components::<&TestOverlayLayer>().is_empty());

            log::debug!("Check control events");
            let mut events = app
                .world_mut()
                .resource_mut::<Events<MapLayerControlEvent<TestLayer>>>();
            let mut event_reader = events.get_cursor();
            match event_reader.read(&*events).next() {
                Some(MapLayerControlEvent::Track(id, _)) => {
                    assert_eq!(id, &chunk_id);
                }
                other => panic!("Unexpected event, got: {other:?}"),
            }
            assert!(event_reader.read(&*events).next().is_none());
            events.clear();
        }

        log::debug!("Send unload event for chunk {chunk_id:?}");
        app.world_mut()
            .resource_mut::<Events<MapEvent>>()
            .send(MapEvent::Unload(chunk_id));

        // complete cleanup happens in 2 updates
        app.update();
        app.update();

        {
            log::debug!("Check if chunk root is dropped");
            let chunk_tracker = app.world().get_resource::<MapChunkTracker>().unwrap();
            assert!(chunk_tracker.get_entity(chunk_id).is_none());
            assert!(app.world().get_entity(chunk_root_entity).is_err());

            log::debug!("Check if shard entity and components are dropped");
            let tile_map = app.world().get_resource::<TestLayerTracker>().unwrap();
            assert_eq!(tile_map.get_entity(chunk_id), None);
            assert_eq!(tile_map.get_chunk_id(chunk_entity), None);
            assert!(app.world().get_entity(chunk_entity).is_err());

            log::debug!("Check control events");
            let mut events = app
                .world_mut()
                .resource_mut::<Events<MapLayerControlEvent<TestLayer>>>();
            let mut event_reader = events.get_cursor();
            match event_reader.read(&*events).next() {
                Some(MapLayerControlEvent::Untrack(id)) => {
                    assert_eq!(id, &chunk_id);
                }
                other => panic!("Unexpected event, got: {other:?}"),
            }
            assert!(event_reader.read(&*events).next().is_none());
            events.clear();
        }
    }
}
