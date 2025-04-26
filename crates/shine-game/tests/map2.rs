use bevy::{
    app::{App, Update},
    ecs::{
        event::{Event, EventReader, Events},
        system::{Commands, ResMut},
    },
    tasks::{AsyncComputeTaskPool, BoxedFuture, TaskPool},
};
use shine_game::map2::{
    Chunk, ChunkCommand, ChunkFactory, ChunkId, ChunkSize, NoOperation, TileMap, TileMapConfig, TileMapError,
    TileMapPlugin,
};
use shine_test::test;

#[derive(Clone)]
pub struct U8TileMapConfig;

impl TileMapConfig for U8TileMapConfig {
    const NAME: &'static str = "test";
    type Tile = u8;
    type ChunkOperation = NoOperation<Self>;

    fn chunk_size(&self) -> ChunkSize {
        ChunkSize { width: 16, height: 16 }
    }

    fn max_retry_count(&self) -> usize {
        3
    }
}

pub struct U8ChunkFactory;
impl ChunkFactory<U8TileMapConfig> for U8ChunkFactory {
    fn read<'a>(
        &'a self,
        config: &'a U8TileMapConfig,
        _chunk_id: ChunkId,
    ) -> BoxedFuture<'a, Result<(Chunk<U8TileMapConfig>, usize), TileMapError>> {
        Box::pin(async move { Ok((Chunk::new(config.chunk_size()), 0)) })
    }

    fn read_updates<'a>(
        &'a self,
        config: &U8TileMapConfig,
        chunk_id: ChunkId,
        version: usize,
    ) -> BoxedFuture<'a, Result<Vec<ChunkCommand<U8TileMapConfig>>, TileMapError>> {
        Box::pin(async move { Ok(vec![]) })
    }
}

#[derive(Event)]
enum TestEvent {
    Load(ChunkId),
    Unload(ChunkId),
}

fn spawn_test_chunk(
    mut tile_map: ResMut<TileMap<U8TileMapConfig>>,
    mut commands: Commands,
    mut ev: EventReader<TestEvent>,
) {
    for chunk_id in ev.read() {
        match chunk_id {
            TestEvent::Load(chunk_id) => {
                tile_map.load_chunk(*chunk_id, &mut commands);
            }
            TestEvent::Unload(chunk_id) => {
                tile_map.unload_chunk(*chunk_id, &mut commands);
            }
        }
    }
}

#[test]
async fn test_map2_load_unload() {
    AsyncComputeTaskPool::get_or_init(TaskPool::new);
    let mut app = App::new();

    // Initialize the TileMap
    app.add_plugins(TileMapPlugin::<U8TileMapConfig>::new(U8TileMapConfig, U8ChunkFactory));
    app.add_systems(Update, spawn_test_chunk);
    app.add_event::<TestEvent>();

    app.update();

    {
        let tile_map = app.world().get_resource::<TileMap<U8TileMapConfig>>().unwrap();
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
        .resource_mut::<Events<TestEvent>>()
        .send(TestEvent::Load(chunk_id_1));

    app.update();

    {
        let tile_map = app.world().get_resource::<TileMap<U8TileMapConfig>>().unwrap();
        let stats = tile_map.statistics();
        log::info!("TileMap statistics: {:?}", stats);
        assert!(stats.load_requests == 0);
        assert!(stats.loading_tasks == 0);
        assert!(stats.loaded_chunks == 1);

        chunk_entity_1 = tile_map.get_chunk_entity(ChunkId(13, 42)).unwrap();
        let chunk = app
            .world()
            .entity(chunk_entity_1)
            .get::<Chunk<U8TileMapConfig>>()
            .unwrap();
        assert_eq!(chunk.width(), 16);
        assert_eq!(chunk.height(), 16);
    }

    // unload a chunk
    app.world_mut()
        .resource_mut::<Events<TestEvent>>()
        .send(TestEvent::Unload(chunk_id_1));

    app.update();
    {
        let tile_map = app.world().get_resource::<TileMap<U8TileMapConfig>>().unwrap();
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
