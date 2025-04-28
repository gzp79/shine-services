use crate::map2::{ChunkId, TileMap, TileMapConfig};
use bevy::{
    ecs::{
        resource::Resource,
        system::{Local, Res, ResMut},
    },
    platform::sync::{Arc, Mutex},
    tasks::block_on,
};
use std::{marker::PhantomData, mem};

pub type UpdatedChunks = Vec<ChunkId>;

/// Event to request chunk loading and unloading
#[derive(Resource)]
pub struct TileMapRefresh<C>
where
    C: TileMapConfig,
{
    changes: Arc<Mutex<UpdatedChunks>>,
    phantom: PhantomData<C>,
}

impl<C> TileMapRefresh<C>
where
    C: TileMapConfig,
{
    pub fn new(changes: UpdatedChunks) -> Self {
        Self {
            changes: Arc::new(Mutex::new(changes)),
            phantom: PhantomData,
        }
    }
}

pub fn startup_map_refresh<C>(tile_map: Res<TileMap<C>>, map_refresh: Res<TileMapRefresh<C>>)
where
    C: TileMapConfig,
{
    let factory = tile_map.factory().clone();
    let config = tile_map.config().clone();
    let refresh_channel = map_refresh.changes.clone();

    block_on(async move {
        factory
            .listen_updates(&config, refresh_channel)
            .await
            .expect("Failed to start listening to event store");
    });
}

pub fn process_map_refresh<C>(
    mut tile_map: ResMut<TileMap<C>>,
    map_refresh: ResMut<TileMapRefresh<C>>,
    mut updated_chunks: Local<UpdatedChunks>,
) where
    C: TileMapConfig,
{
    assert!(updated_chunks.is_empty());
    mem::swap(&mut *map_refresh.changes.lock().unwrap(), &mut *updated_chunks);

    if !updated_chunks.is_empty() {
        log::trace!("Detected chunk updates: {:?}", updated_chunks);
    }
    for chunk_id in updated_chunks.drain(..) {
        tile_map.refresh_chunk(chunk_id);
    }
}
