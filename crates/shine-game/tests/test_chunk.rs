use bevy::{app::App, ecs::event::Events, DefaultPlugins};
use shine_game::map::{
    client, ChunkCommand, ChunkCommandQueue, ChunkEvent, ChunkHashTrack, ChunkHasher, ChunkId,
    ChunkLayer, ChunkLayerSetup, ChunkOperation, ChunkRoot, ChunkVersion, DenseGridChunk,
    GridChunk, LayerSetup, MapChunk, MapChunkTracker, MapEvent, MapPlugin, SparseGridChunk,
};
use shine_test::test;
use std::fmt;

mod shared;
use shared::{
    test_init_bevy, DenseGridU8, DenseGridU8Hasher, GridU8Operation, SparseGridU8,
    SparseGridU8Hasher, TestData, TestDataHasher, TestDataLayerSetup, TestDataOperation,
    TestGridConfig,
};

const WIDTH: usize = 16;
const HEIGHT: usize = 16;

trait TestCase {
    type Chunk: MapChunk;
    type Hasher: ChunkHasher<Self::Chunk, Hash: fmt::Debug> + Default;
    type Operation: ChunkOperation<Self::Chunk>;
    type ClientEventService: client::SendChunkEventService<Self::Chunk>;

    fn layer(
        &self,
        command_queue: ChunkCommandQueue<Self::Chunk, Self::Operation, Self::Hasher>,
    ) -> impl LayerSetup<TestGridConfig>;

    fn has_hash_tracker(&self) -> bool;
    fn test_empty_chunk(&self, chunk: &Self::Chunk);
    fn test_default_chunk(
        &self,
        chunk: &Self::Chunk,
        hash: Option<&<Self::Hasher as ChunkHasher<Self::Chunk>>::Hash>,
    );
}

async fn test_chunk_impl<T>(test_case: T)
where
    T: TestCase,
{
    test_init_bevy();
    let mut app = App::new();

    // Initialize the TileMap
    let command_queue = {
        let command_queue = ChunkCommandQueue::<T::Chunk, T::Operation, T::Hasher>::new();

        app.add_plugins(DefaultPlugins).add_plugins(
            MapPlugin::new(TestGridConfig {
                width: WIDTH,
                height: HEIGHT,
            })
            .with_layer(test_case.layer(command_queue.clone())),
        );

        command_queue
    };

    app.update();

    let chunk_id: ChunkId = ChunkId(13, 42);
    let chunk_entity;

    log::info!("Loading chunk");
    {
        app.world_mut()
            .resource_mut::<Events<MapEvent>>()
            .send(MapEvent::Load(chunk_id));

        app.update();

        log::debug!("Check if chunk is created");
        {
            let tile_map = app.world().get_resource::<MapChunkTracker>().unwrap();
            chunk_entity = tile_map.get_entity(chunk_id).unwrap();

            let layer = app.world().get_resource::<ChunkLayer<T::Chunk>>().unwrap();
            assert_eq!(layer.get_entity(chunk_id), Some(chunk_entity));
            assert_eq!(layer.get_chunk_id(chunk_entity), Some(chunk_id));

            let (chunk_root, test_data) = app
                .world()
                .entity(chunk_entity)
                .components::<(&ChunkRoot, &T::Chunk)>();
            assert_eq!(chunk_root.id, chunk_id);
            assert!(test_data.is_empty());
            test_case.test_empty_chunk(test_data);

            let hash_tracker = app
                .world()
                .entity(chunk_entity)
                .get_components::<&ChunkHashTrack<T::Chunk, T::Hasher>>();

            if test_case.has_hash_tracker() {
                assert!(hash_tracker.is_some());
                let hash_tracker = hash_tracker.unwrap();
                assert_eq!(hash_tracker.get(0), None);
            } else {
                assert!(hash_tracker.is_none());
            }
        }

        log::debug!("Check if events were sent");
        {
            let mut events = app
                .world_mut()
                .resource_mut::<Events<ChunkEvent<T::Chunk>>>();
            let mut event_reader = events.get_cursor();
            let event = event_reader.read(&*events).next().unwrap();
            match event {
                ChunkEvent::Track { id } => {
                    assert_eq!(id, &chunk_id);
                }
                _ => panic!("Unexpected event"),
            }

            assert!(event_reader.read(&*events).next().is_none());
            // emulate a new frame w.r.t. the event reader
            events.clear();
        }
    }

    log::info!("Start chunk tracking, send an empty data");
    {
        command_queue.add_command(chunk_id, ChunkCommand::Empty);
        app.update();

        log::debug!("Check if the chunk is empty");
        {
            let (chunk_root, chunk_version, test_data) = app
                .world()
                .entity(chunk_entity)
                .components::<(&ChunkRoot, &ChunkVersion<T::Chunk>, &T::Chunk)>();
            let hash_tracker = app
                .world()
                .entity(chunk_entity)
                .get_components::<&ChunkHashTrack<T::Chunk, T::Hasher>>();
            assert_eq!(chunk_root.id, chunk_id);
            assert!(!test_data.is_empty());
            assert_eq!(chunk_version.version, 0);

            test_case.test_default_chunk(test_data, hash_tracker.and_then(|h| h.get(0)));
        }
    }

    log::info!("Unloading chunk");
    {
        app.world_mut()
            .resource_mut::<Events<MapEvent>>()
            .send(MapEvent::Unload(chunk_id));

        app.update();

        log::debug!("Check if chunk is dropped");
        {
            let chunk_tracker = app.world().get_resource::<MapChunkTracker>().unwrap();
            assert!(chunk_tracker.get_entity(chunk_id).is_none());

            let layer = app.world().get_resource::<ChunkLayer<T::Chunk>>().unwrap();
            assert!(layer.get_entity(chunk_id).is_none());

            let result = app.world().get_entity(chunk_entity);
            assert!(result.is_err());
        }

        log::info!("Check if untrack event was sent");
        {
            let events = app.world().resource::<Events<ChunkEvent<T::Chunk>>>();
            let mut event_reader = events.get_cursor();
            let event = event_reader.read(events).next().unwrap();
            match event {
                ChunkEvent::Untrack { id } => {
                    assert_eq!(id, &chunk_id);
                }
                _ => panic!("Unexpected event"),
            }

            assert!(event_reader.read(events).next().is_none());
        }
    }
}

struct TestDataTestCase;

impl TestCase for TestDataTestCase {
    type Chunk = TestData;
    type Hasher = TestDataHasher;
    type Operation = TestDataOperation;
    type ClientEventService = client::NullChunkEventService;

    fn layer(
        &self,
        command_queue: ChunkCommandQueue<Self::Chunk, Self::Operation, Self::Hasher>,
    ) -> impl LayerSetup<TestGridConfig> {
        TestDataLayerSetup::new_with_queue(command_queue)
    }

    fn has_hash_tracker(&self) -> bool {
        true
    }

    fn test_empty_chunk(&self, chunk: &Self::Chunk) {
        assert_eq!(chunk.data(), None);
    }

    fn test_default_chunk(&self, chunk: &Self::Chunk, hash: Option<&usize>) {
        assert_eq!(chunk.data(), Some(WIDTH * HEIGHT));
        assert_eq!(hash, Some(&(WIDTH * HEIGHT)));
    }
}

#[test]
async fn test_data_chunk() {
    test_chunk_impl(TestDataTestCase).await;
}

struct DenseGridTestCase {
    with_hasher: bool,
    with_client: bool,
}

impl TestCase for DenseGridTestCase {
    type Chunk = DenseGridU8;
    type Hasher = DenseGridU8Hasher;
    type Operation = GridU8Operation;
    type ClientEventService = client::NullChunkEventService;

    fn layer(
        &self,
        command_queue: ChunkCommandQueue<Self::Chunk, Self::Operation, Self::Hasher>,
    ) -> impl LayerSetup<TestGridConfig> {
        let mut layer = ChunkLayerSetup::<
            Self::Chunk,
            Self::Operation,
            Self::Hasher,
            Self::ClientEventService,
        >::new(command_queue);
        if self.with_hasher {
            layer = layer.with_hash_tracker(DenseGridU8Hasher);
        }
        if self.with_client {
            layer = layer.with_client_send_service(client::NullChunkEventService);
        }
        layer
    }

    fn has_hash_tracker(&self) -> bool {
        self.with_hasher
    }

    fn test_empty_chunk(&self, chunk: &Self::Chunk) {
        assert_eq!(chunk.width(), 0);
        assert_eq!(chunk.height(), 0);
        assert!(chunk.data().is_empty());
    }

    fn test_default_chunk(&self, chunk: &Self::Chunk, hash: Option<&u64>) {
        assert_eq!(chunk.width(), WIDTH);
        assert_eq!(chunk.height(), HEIGHT);
        if self.with_hasher {
            assert_eq!(hash, Some(&0));
        } else {
            assert!(hash.is_none());
        }
    }
}

#[test]
async fn test_dense_grid_chunk() {
    test_chunk_impl(DenseGridTestCase {
        with_hasher: false,
        with_client: false,
    })
    .await;
}

#[test]
async fn test_dense_grid_chunk_with_hash() {
    test_chunk_impl(DenseGridTestCase {
        with_hasher: true,
        with_client: false,
    })
    .await;
}

#[test]
async fn test_dense_grid_chunk_for_client() {
    test_chunk_impl(DenseGridTestCase {
        with_hasher: true,
        with_client: true,
    })
    .await;
}

struct SparseGridTestCase {
    with_hasher: bool,
}

impl TestCase for SparseGridTestCase {
    type Chunk = SparseGridU8;
    type Hasher = SparseGridU8Hasher;
    type Operation = GridU8Operation;
    type ClientEventService = client::NullChunkEventService;

    fn layer(
        &self,
        command_queue: ChunkCommandQueue<Self::Chunk, Self::Operation, Self::Hasher>,
    ) -> impl LayerSetup<TestGridConfig> {
        let mut layer = ChunkLayerSetup::<
            Self::Chunk,
            Self::Operation,
            Self::Hasher,
            Self::ClientEventService,
        >::new(command_queue);
        if self.with_hasher {
            layer = layer.with_hash_tracker(SparseGridU8Hasher);
        }
        layer
    }

    fn has_hash_tracker(&self) -> bool {
        self.with_hasher
    }

    fn test_empty_chunk(&self, chunk: &Self::Chunk) {
        assert_eq!(chunk.width(), 0);
        assert_eq!(chunk.height(), 0);
        assert!(chunk.occupied().next().is_none());
    }

    fn test_default_chunk(&self, chunk: &Self::Chunk, hash: Option<&u64>) {
        assert_eq!(chunk.width(), WIDTH);
        assert_eq!(chunk.height(), HEIGHT);
        assert!(chunk.occupied().next().is_none());
        if self.with_hasher {
            assert_eq!(hash, Some(&0));
        } else {
            assert!(hash.is_none());
        }
    }
}

#[test]
async fn test_sparse_grid_chunk() {
    test_chunk_impl(SparseGridTestCase { with_hasher: false }).await;
}

#[test]
async fn test_sparse_grid_chunk_with_hash() {
    test_chunk_impl(SparseGridTestCase { with_hasher: true }).await;
}
