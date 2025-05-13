use bevy::{
    app::App,
    ecs::event::Events,
    tasks::{AsyncComputeTaskPool, TaskPool},
    DefaultPlugins,
};
use serde::{Deserialize, Serialize};
use shine_game::map::{
    operations, ChunkCommand, ChunkCommandQueue, ChunkHashTrack, ChunkHasher, ChunkId, ChunkLayer, ChunkLayerSetup,
    ChunkOperation, ChunkRoot, ChunkStore, ChunkType, DenseChunk, DenseChunkStore, TileMap, TileMapConfig,
    TileMapEvent, TileMapPlugin,
};
use shine_test::test;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum U8Operations {
    SetTile(operations::SetTile<u8>),
    Fill(operations::Fill<u8>),
}

impl From<operations::SetTile<u8>> for U8Operations {
    fn from(op: operations::SetTile<u8>) -> Self {
        U8Operations::SetTile(op)
    }
}

impl From<operations::Fill<u8>> for U8Operations {
    fn from(op: operations::Fill<u8>) -> Self {
        U8Operations::Fill(op)
    }
}

impl ChunkOperation for U8Operations {
    type Tile = u8;

    fn apply<C>(self, chunk: &mut C)
    where
        C: ChunkStore<Tile = u8>,
    {
        match self {
            U8Operations::SetTile(op) => op.apply(chunk),
            U8Operations::Fill(op) => op.apply(chunk),
        }
    }
}

#[derive(Clone)]
pub struct U8Hasher;

impl ChunkHasher for U8Hasher {
    type Chunk = U8Chunk;
    type Hash = u64;

    fn hash(&self, chunk: &Self::Chunk) -> Self::Hash {
        chunk
            .data()
            .iter()
            .fold(0, |acc, &tile| acc.wrapping_mul(31).wrapping_add(tile as u64))
    }
}

pub struct U8ChunkTypes;
impl ChunkType for U8ChunkTypes {
    const NAME: &'static str = "u8";
    type Tile = u8;
    type Operation = U8Operations;
}

pub type U8Chunk = DenseChunk<U8ChunkTypes>;
pub type U8ChunkCommandQueue = ChunkCommandQueue<U8Chunk, U8Hasher>;
pub type U8ChunkLayer = ChunkLayer<U8Chunk, U8Hasher>;
pub type U8HashTracker = ChunkHashTrack<U8Chunk, U8Hasher>;

#[test]
async fn test_tile_map_chunk_load_unload() {
    AsyncComputeTaskPool::get_or_init(TaskPool::new);
    let mut app = App::new();

    let command_queue = U8ChunkCommandQueue::new();

    // Initialize the TileMap
    app.add_plugins(DefaultPlugins).add_plugins(
        TileMapPlugin::new(TileMapConfig { width: 16, height: 16 }).with_layer(ChunkLayerSetup::new(command_queue)),
    );

    app.update();

    // create a chunk
    let chunk_id: ChunkId = ChunkId(13, 42);
    let chunk_entity;

    app.world_mut()
        .resource_mut::<Events<TileMapEvent>>()
        .send(TileMapEvent::Load(chunk_id));

    app.update();

    {
        log::debug!("Check if chunk is created with the layers");

        let tile_map = app.world().get_resource::<TileMap>().unwrap();
        chunk_entity = tile_map.get_entity(chunk_id).unwrap();

        let layer = app.world().get_resource::<U8ChunkLayer>().unwrap();
        assert_eq!(layer.get_entity(chunk_id), Some(chunk_entity));
        assert_eq!(layer.get_chunk_id(chunk_entity), Some(chunk_id));

        let (chunk_root, chunk) = app.world().entity(chunk_entity).components::<(&ChunkRoot, &U8Chunk)>();
        assert_eq!(chunk_root.id, chunk_id);
        assert!(chunk.is_empty());
    }

    app.world_mut()
        .resource_mut::<Events<TileMapEvent>>()
        .send(TileMapEvent::Unload(chunk_id));

    app.update();

    {
        log::debug!("Check if chunk is dropped");

        let tile_map = app.world().get_resource::<TileMap>().unwrap();
        assert!(tile_map.get_entity(chunk_id).is_none());

        let layer = app.world().get_resource::<U8ChunkLayer>().unwrap();
        assert!(layer.get_entity(chunk_id).is_none());

        let result = app.world().get_entity(chunk_entity);
        assert!(result.is_err());
    }
}

#[test]
async fn test_tile_map_chunk_operations() {
    AsyncComputeTaskPool::get_or_init(TaskPool::new);
    let mut app = App::new();

    let command_queue = U8ChunkCommandQueue::new();

    // Initialize the TileMap with a chunk
    app.add_plugins(DefaultPlugins);
    app.add_plugins(
        TileMapPlugin::new(TileMapConfig { width: 16, height: 16 })
            .with_layer(ChunkLayerSetup::new(command_queue.clone()).with_hash_tracker(U8Hasher)),
    );

    let chunk_id: ChunkId = ChunkId(13, 42);

    app.world_mut()
        .resource_mut::<Events<TileMapEvent>>()
        .send(TileMapEvent::Load(chunk_id));

    app.update();

    //replace the whole chunk
    let mut new_data = U8Chunk::new(16, 16);
    new_data.data_mut().fill(13);
    *new_data.version_mut() = 42;
    command_queue.store_command(chunk_id, ChunkCommand::Data(new_data));

    //update the chunk tile at 3,4 with an operation
    command_queue.store_command(
        chunk_id,
        ChunkCommand::Operations(vec![
            (41, operations::SetTile { x: 1, y: 7, tile: 1 }.into()),
            (42, operations::SetTile { x: 2, y: 6, tile: 2 }.into()),
            (43, operations::SetTile { x: 3, y: 5, tile: 3 }.into()),
            (44, operations::SetTile { x: 4, y: 4, tile: 4 }.into()),
            (46, operations::SetTile { x: 5, y: 3, tile: 5 }.into()),
            (47, operations::SetTile { x: 6, y: 2, tile: 6 }.into()),
            (48, operations::SetTile { x: 7, y: 1, tile: 7 }.into()),
        ]),
    );

    app.update();

    {
        let tile_map = app.world().get_resource::<TileMap>().unwrap();
        let entity = tile_map.get_entity(chunk_id).unwrap();

        let (chunk_root, chunk, hash_tracker) = app
            .world()
            .entity(entity)
            .components::<(&ChunkRoot, &U8Chunk, &U8HashTracker)>();
        assert_eq!(chunk_root.id, chunk_id);
        assert_eq!(chunk.version(), 44);
        assert_eq!(hash_tracker.get(44), Some(&6898905103204884639));
        assert_eq!(chunk.width(), 16);
        assert_eq!(chunk.height(), 16);
    }

    // add the missing gap, so full operation queue can be replayed
    command_queue.store_command(
        chunk_id,
        ChunkCommand::Operations(vec![(45, operations::SetTile { x: 9, y: 9, tile: 9 }.into())]),
    );

    app.update();

    {
        let tile_map = app.world().get_resource::<TileMap>().unwrap();
        let entity = tile_map.get_entity(chunk_id).unwrap();

        let (chunk_root, chunk, hash_tracker) = app
            .world()
            .entity(entity)
            .components::<(&ChunkRoot, &U8Chunk, &U8HashTracker)>();
        assert_eq!(chunk_root.id, chunk_id);
        assert_eq!(chunk.version(), 48);
        assert_eq!(hash_tracker.get(48), Some(&7757264616343645620));
        assert_eq!(chunk.width(), 16);
        assert_eq!(chunk.height(), 16);
    }

    // adding outdated commands should not affect the chunk
    for cmd in [
        ChunkCommand::Empty,
        ChunkCommand::Data({
            let mut chunk = U8Chunk::new(16, 16);
            *chunk.version_mut() = 13;
            chunk
        }),
        ChunkCommand::Operations(vec![(5, operations::SetTile { x: 1, y: 1, tile: 99 }.into())]),
    ] {
        command_queue.store_command(chunk_id, cmd);

        app.update();

        {
            let tile_map = app.world().get_resource::<TileMap>().unwrap();
            let entity = tile_map.get_entity(chunk_id).unwrap();

            let (chunk_root, chunk, hash_tracker) = app
                .world()
                .entity(entity)
                .components::<(&ChunkRoot, &U8Chunk, &U8HashTracker)>();
            assert_eq!(chunk_root.id, chunk_id);
            assert_eq!(chunk.version(), 48);
            assert_eq!(hash_tracker.get(48), Some(&7757264616343645620));
        }
    }

    // test tile
}
