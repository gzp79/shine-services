#![cfg(feature = "persisted")]

use bevy::{
    app::App,
    ecs::event::Events,
    tasks::{AsyncComputeTaskPool, TaskPool},
    DefaultPlugins,
};
use ring::rand::{SecureRandom, SystemRandom};
use serde::{Deserialize, Serialize};
use shine_game::map2::{
    event_db::{ESChunkDB, ESChunkFactory},
    operations, ChunkCommand, ChunkId, ChunkOperation, ChunkStore, DenseChunkStore, PersistedChunk, TileMap,
    TileMapConfig, TileMapEvent, TileMapPlugin,
};
use shine_infra::db::{self, event_source::Event as ESEvent, DBError, PGConnectionPool};
use shine_test::test;
use std::{env, time::Instant};
use tokio::sync::OnceCell;

/// Initialize the test environment
static INIT: OnceCell<()> = OnceCell::const_new();
async fn initialize(cns: &str) {
    INIT.get_or_init(|| async {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let pool = create_pg_pool(cns).await.unwrap();
        let mut client = pool.get().await.unwrap();
        client
            .migrate("map2_server_state_test", &ESChunkDB::<U8MapConfig>::migrations())
            .await
            .unwrap();
    })
    .await;
}

pub async fn create_pg_pool(cns: &str) -> Result<PGConnectionPool, DBError> {
    log::info!("Creating postgres pool...");
    let postgres = db::create_postgres_pool(cns)
        .await
        .map_err(DBError::PGCreatePoolError)?;
    Ok(postgres)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum U8Operations {
    SetTile(operations::SetTile<u8>),
    Fill(operations::Fill<u8>),
    AddTile(operations::AddTile<u8>),
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

impl From<operations::AddTile<u8>> for U8Operations {
    fn from(op: operations::AddTile<u8>) -> Self {
        U8Operations::AddTile(op)
    }
}

impl ESEvent for U8Operations {
    const NAME: &'static str = "u8_map_test";
    fn event_type(&self) -> &'static str {
        match self {
            U8Operations::SetTile(_) => "SetTile",
            U8Operations::Fill(_) => "Fill",
            U8Operations::AddTile(_) => "AddTile",
        }
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
            U8Operations::AddTile(op) => op.apply(chunk),
        }
    }
}

pub type U8Commands = ChunkCommand<U8Operations>;
pub type U8MapEvent = TileMapEvent<U8MapConfig>;

#[derive(Clone)]
pub struct U8MapConfig;

impl TileMapConfig for U8MapConfig {
    const NAME: &'static str = "u8_map_test";
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

#[test]
async fn test_map2_server_state() {
    let cns = match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => cns,
        _ => {
            log::warn!("SHINE_TEST_PG_CNS not set, skipping test");
            return;
        }
    };

    initialize(&cns).await;
    AsyncComputeTaskPool::get_or_init(TaskPool::new);
    let mut app = App::new();

    let pool = create_pg_pool(&cns).await.unwrap();
    let factory = ESChunkFactory::new(&pool).await.unwrap();
    app.add_plugins(DefaultPlugins)
        .add_plugins(TileMapPlugin::<U8MapConfig>::new(U8MapConfig, factory.clone()));

    app.update();

    let rng = SystemRandom::new();

    for _ in 0..1000 {
        // start tracking chunk
        let mut chunk_id_bytes = [0u8; 8];
        rng.fill(&mut chunk_id_bytes).unwrap();
        let chunk_id_1 = usize::from_ne_bytes(chunk_id_bytes);

        rng.fill(&mut chunk_id_bytes).unwrap();
        let chunk_id_2 = usize::from_ne_bytes(chunk_id_bytes);

        let chunk_id: ChunkId = ChunkId(chunk_id_1, chunk_id_2);

        let tile_index = (8, 8);
        let mut value;

        app.world_mut()
            .resource_mut::<Events<U8MapEvent>>()
            .send(U8MapEvent::Load(chunk_id));

        {
            log::info!("Waiting initial chunk load...");
            let instant = Instant::now();
            loop {
                app.update();

                let tile_map = app.world().get_resource::<TileMap<U8MapConfig>>().unwrap();
                let stats = tile_map.statistics();
                if stats.loading_tasks == 0 {
                    log::info!("Initial chunk load complete in {:?}", instant.elapsed());

                    let tile_map = app.world().get_resource::<TileMap<U8MapConfig>>().unwrap();
                    let entity = tile_map.get_chunk_entity(chunk_id).unwrap();
                    let chunk = app.world().entity(entity).get::<PersistedChunk<U8MapConfig>>().unwrap();

                    value = *chunk.get(tile_index.0, tile_index.1);
                    log::info!("Chunk value {}", value);
                    value = if value == 255 { 1 } else { value + 1 };
                    break;
                }

                if instant.elapsed().as_secs() > 10 {
                    panic!("Timeout waiting for initial chunk load");
                }
            }
        }

        for _ in 0..10 {
            log::info!("Sending update command {}...", value);
            let instant = Instant::now();

            factory
                .store_operation(
                    chunk_id,
                    operations::SetTile {
                        x: tile_index.0,
                        y: tile_index.1,
                        tile: value,
                    },
                )
                .await
                .unwrap();
            log::info!("Chunk operation store completed in {:?}", instant.elapsed());

            log::info!("Waiting chunk update ...");
            loop {
                app.update();

                let tile_map = app.world().get_resource::<TileMap<U8MapConfig>>().unwrap();
                let entity = tile_map.get_chunk_entity(chunk_id).unwrap();
                let chunk = app.world().entity(entity).get::<PersistedChunk<U8MapConfig>>().unwrap();

                if chunk.get(tile_index.0, tile_index.1) == &value {
                    log::info!("Chunk update completed in {:?}", instant.elapsed());
                    value = if value == 255 { 1 } else { value + 1 };
                    break;
                }

                if instant.elapsed().as_secs() > 10 {
                    panic!("Timeout waiting for chunk update");
                }
            }
        }

        log::info!("\n\n");
    }
}
