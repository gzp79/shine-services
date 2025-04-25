use crate::map2::{Chunk, ChunkCommand, ChunkFactory, ChunkId, ChunkOperation, TileMapConfig};
use bevy::{
    ecs::{
        entity::Entity,
        resource::Resource,
        system::{Commands, Query, ResMut},
    },
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task},
};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
};

type ChunkLoadTask<T> = Task<Result<Chunk<T>, ()>>;

#[derive(Resource)]
pub struct TileMap<C>
where
    C: TileMapConfig,
{
    config: C,
    factory: Arc<dyn ChunkFactory<C>>,
    load_requests: VecDeque<(ChunkId, usize)>,
    loading_chunks: HashMap<ChunkId, (ChunkLoadTask<C>, usize)>,
    loaded_chunks: HashMap<ChunkId, Entity>,
    commands: VecDeque<ChunkCommand<C::ChunkOperation>>,
}

impl<C> TileMap<C>
where
    C: TileMapConfig,
{
    pub fn new(config: C, factory: Arc<dyn ChunkFactory<C>>) -> Self {
        Self {
            config,
            factory,
            load_requests: VecDeque::new(),
            loading_chunks: HashMap::new(),
            loaded_chunks: HashMap::new(),
            commands: VecDeque::new(),
        }
    }

    pub fn config(&self) -> &C {
        &self.config
    }

    pub fn request_chunk_load(&mut self, chunk_id: ChunkId) {
        if !self.loaded_chunks.contains_key(&chunk_id) && !self.load_requests.iter().any(|(id, _)| id == &chunk_id) {
            self.load_requests.push_back((chunk_id, 3));
        }
    }

    pub fn add_command(&mut self, command: ChunkCommand<C::ChunkOperation>) {
        if self.load_requests.iter().any(|(id, _)| id == &command.chunk_id)
            || self.loading_chunks.contains_key(&command.chunk_id)
            || self.loaded_chunks.contains_key(&command.chunk_id)
        {
            self.commands.push_back(command);
        }
    }
}

pub fn start_chunk_load_system<C>(mut tile_map: ResMut<TileMap<C>>)
where
    C: TileMapConfig,
{
    let task_pool = AsyncComputeTaskPool::get();

    while let Some((chunk_id, retry_count)) = tile_map.load_requests.pop_front() {
        if tile_map.loaded_chunks.contains_key(&chunk_id) || tile_map.loading_chunks.contains_key(&chunk_id) {
            continue;
        }

        let config = tile_map.config.clone();
        let factory = tile_map.factory.clone();
        let task = task_pool.spawn(async move {
            println!("Loading chunk asynchronously: {:?}", chunk_id);
            let chunk = factory.read(&config, chunk_id).await;
            println!("Chunk loaded: {:?}", chunk_id);
            chunk
        });
        tile_map.loading_chunks.insert(chunk_id, (task, retry_count));
    }
}

pub fn complete_chunk_load_system<C>(mut tile_map: ResMut<TileMap<C>>, mut commands: Commands)
where
    C: TileMapConfig,
{
    let TileMap {
        loading_chunks,
        loaded_chunks,
        load_requests,
        ..
    } = tile_map.as_mut();

    loading_chunks.retain(|chunk_id, (task, retry_count)| {
        let status = block_on(future::poll_once(task));
        let retain = status.is_none();

        if let Some(chunk) = status {
            if let Ok(chunk) = chunk {
                debug_assert!(!loaded_chunks.contains_key(chunk_id));
                let entity = commands.spawn_empty().insert(chunk).id();
                loaded_chunks.insert(*chunk_id, entity);
            } else if *retry_count > 0 {
                log::warn!("Failed to load chunk ({:?}), retry left: {}", chunk_id, retry_count);
                //todo: add max retry count
                load_requests.push_back((*chunk_id, *retry_count - 1));
            } else {
                log::error!("Failed to load chunk ({:?})", chunk_id);
            }
        }

        retain
    });
}

pub fn process_commands_system<C>(mut tile_map: ResMut<TileMap<C>>, mut chunks: Query<&mut Chunk<C>>)
where
    C: TileMapConfig,
{
    let TileMap {
        loading_chunks,
        loaded_chunks,
        commands,
        ..
    } = tile_map.as_mut();

    commands.retain_mut(|command| {
        if loading_chunks.contains_key(&command.chunk_id) {
            log::info!(
                "Chunk is still loading, command for {:?} will be delayed",
                command.chunk_id
            );
            false
        } else {
            if let Some(mut chunk) = loaded_chunks
                .get(&command.chunk_id)
                .and_then(|entity| chunks.get_mut(*entity).ok())
            {
                if let Some(version) = command.version {
                    if chunk.version() == version - 1 {
                        command.operation.apply(&mut chunk);
                        chunk.set_version(version);
                        false
                    } else if chunk.version() < version {
                        log::info!(
                            "Command is too early ({}) for chunk {:?} at version {}",
                            version,
                            command.chunk_id,
                            chunk.version()
                        );
                        true
                    } else {
                        log::debug!(
                            "Command is too late ({}) for chunk {:?} at version {}",
                            version,
                            command.chunk_id,
                            chunk.version()
                        );
                        false
                    }
                } else {
                    command.operation.apply_local(&mut chunk);
                    false
                }
            } else {
                log::info!("Chunk {:?} is not tracked by the tile-map", command.chunk_id);
                false
            }
        }
    });
}
