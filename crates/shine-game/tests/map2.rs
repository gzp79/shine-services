use bevy::{
    app::App,
    ecs::event::Events,
    platform::sync::{Arc, Mutex},
    tasks::{AsyncComputeTaskPool, BoxedFuture, TaskPool},
};
use serde::{Deserialize, Serialize};
use shine_game::map2::{
    operations, ChunkCommand, ChunkFactory, ChunkId, ChunkOperation, ChunkStore, DenseChunkStore, PersistedChunk,
    TileMap, TileMapConfig, TileMapError, TileMapEvent, TileMapPlugin, UpdatedChunks,
};
use shine_test::test;

#[derive(Serialize, Deserialize)]
pub enum U8Operations {
    SetTile(operations::SetTile<u8>),
    GetTile(operations::Fill<u8>),
}

impl ChunkOperation for U8Operations {
    type Tile = u8;

    fn apply<C>(self, chunk: &mut C)
    where
        C: ChunkStore<Tile = u8>,
    {
        match self {
            U8Operations::SetTile(op) => op.apply(chunk),
            U8Operations::GetTile(op) => op.apply(chunk),
        }
    }
}

pub type U8Commands = ChunkCommand<U8Operations>;
pub type U8MapEvent = TileMapEvent<U8MapConfig>;

#[derive(Clone)]
pub struct U8MapConfig;

impl TileMapConfig for U8MapConfig {
    const NAME: &'static str = "test";
    type Tile = u8;

    type PersistedChunkStore = DenseChunkStore<Self::Tile>;
    type PersistedChunkOperation = U8Operations;

    fn chunk_size(&self) -> (usize, usize) {
        (16, 16)
    }

    fn max_retry_count(&self) -> usize {
        3
    }
}

pub struct U8ChunkFactory;
impl ChunkFactory<U8MapConfig> for U8ChunkFactory {
    fn read<'a>(
        &'a self,
        config: &'a U8MapConfig,
        _chunk_id: ChunkId,
    ) -> BoxedFuture<'a, Result<(DenseChunkStore<u8>, usize), TileMapError>> {
        Box::pin(async move {
            let (w, h) = config.chunk_size();
            Ok((DenseChunkStore::new(w, h), 0))
        })
    }

    fn read_updates<'a>(
        &'a self,
        _config: &U8MapConfig,
        _chunk_id: ChunkId,
        _version: usize,
    ) -> BoxedFuture<'a, Result<Vec<U8Commands>, TileMapError>> {
        Box::pin(async move { Ok(vec![]) })
    }

    fn listen_updates<'a>(
        &'a self,
        _config: &'a U8MapConfig,
        _channel: Arc<Mutex<UpdatedChunks>>,
    ) -> BoxedFuture<'a, Result<(), TileMapError>> {
        Box::pin(async move { Ok(()) })
    }
}

#[test]
async fn test_map2_load_unload() {
    AsyncComputeTaskPool::get_or_init(TaskPool::new);
    let mut app = App::new();

    // Initialize the TileMap
    app.add_plugins(TileMapPlugin::<U8MapConfig>::new(U8MapConfig, U8ChunkFactory));

    app.update();

    {
        let tile_map = app.world().get_resource::<TileMap<U8MapConfig>>().unwrap();
        let stats = tile_map.statistics();
        log::info!("TileMap statistics: {:?}", stats);
        assert!(stats.load_requests == 0);
        assert!(stats.loading_tasks == 0);
        assert!(stats.loaded_chunks == 0);
    }

    // load a chunk
    let chunk_id_1: ChunkId = ChunkId(13, 42);
    let chunk_entity_1;

    app.world_mut()
        .resource_mut::<Events<U8MapEvent>>()
        .send(U8MapEvent::Load(chunk_id_1));

    app.update();

    {
        let tile_map = app.world().get_resource::<TileMap<U8MapConfig>>().unwrap();
        let stats = tile_map.statistics();
        log::info!("Map statistics: {:?}", stats);
        assert!(stats.load_requests == 0);
        assert!(stats.loading_tasks == 0);
        assert!(stats.loaded_chunks == 1);

        chunk_entity_1 = tile_map.get_chunk_entity(ChunkId(13, 42)).unwrap();
        let chunk = app
            .world()
            .entity(chunk_entity_1)
            .get::<PersistedChunk<U8MapConfig>>()
            .unwrap();
        assert_eq!(chunk.width(), 16);
        assert_eq!(chunk.height(), 16);
    }

    // unload a chunk
    app.world_mut()
        .resource_mut::<Events<U8MapEvent>>()
        .send(U8MapEvent::Unload(chunk_id_1));

    app.update();
    {
        let tile_map = app.world().get_resource::<TileMap<U8MapConfig>>().unwrap();
        let stats = tile_map.statistics();
        log::info!("TileMap statistics: {:?}", stats);
        assert!(stats.load_requests == 0);
        assert!(stats.loading_tasks == 0);
        assert!(stats.loaded_chunks == 0);

        assert!(tile_map.get_chunk_entity(chunk_id_1).is_none());

        let chunk = app.world().get_entity(chunk_entity_1);
        assert!(chunk.is_err());
    }
}
