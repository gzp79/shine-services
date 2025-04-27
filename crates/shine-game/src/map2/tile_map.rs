use crate::map2::{
    ChunkId, ChunkOperation, PersistedChunk, PersistedChunkUpdate, PersistedVersion, TileMapConfig, TileMapError,
    UpdatedChunks,
};
use bevy::{
    ecs::{
        entity::Entity,
        resource::Resource,
        system::{Commands, Local, Query, ResMut},
    },
    platform::sync::{Arc, Mutex},
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, BoxedFuture, Task},
};
use std::collections::{hash_map::Entry, HashMap, VecDeque};

use super::PersistedChunkCommand;

pub trait ChunkFactory<C>: 'static + Send + Sync
where
    C: TileMapConfig,
{
    fn read<'a>(
        &'a self,
        config: &'a C,
        chunk_id: ChunkId,
    ) -> BoxedFuture<'a, Result<(C::PersistedChunkStore, usize), TileMapError>>;

    fn read_updates<'a>(
        &'a self,
        config: &'a C,
        chunk_id: ChunkId,
        version: usize,
    ) -> BoxedFuture<'a, Result<Vec<PersistedChunkCommand<C>>, TileMapError>>;

    fn listen_updates<'a>(
        &'a self,
        config: &'a C,
        channel: Arc<Mutex<UpdatedChunks>>,
    ) -> BoxedFuture<'a, Result<(), TileMapError>>;
}

#[derive(Debug)]
pub struct TileMapStatistics {
    pub load_requests: usize,
    pub loading_tasks: usize,
    pub loaded_chunks: usize,
}

enum TaskResult<C>
where
    C: TileMapConfig,
{
    Chunk(PersistedChunk<C>, usize),
    Commands(Vec<PersistedChunkCommand<C>>),
    Empty,
    Retry(usize),
}

#[derive(Resource)]
pub struct TileMap<C>
where
    C: TileMapConfig,
{
    config: C,
    factory: Arc<dyn ChunkFactory<C>>,

    load_requests: VecDeque<(ChunkId, usize)>,
    loading_tasks: HashMap<ChunkId, Task<TaskResult<C>>>,
    loaded_chunks: HashMap<ChunkId, Entity>,
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
            loading_tasks: HashMap::new(),
            loaded_chunks: HashMap::new(),
        }
    }

    pub fn config(&self) -> &C {
        &self.config
    }

    pub fn factory(&self) -> &Arc<dyn ChunkFactory<C>> {
        &self.factory
    }

    pub fn statistics(&self) -> TileMapStatistics {
        TileMapStatistics {
            load_requests: self.load_requests.len(),
            loading_tasks: self.loading_tasks.len(),
            loaded_chunks: self.loaded_chunks.len(),
        }
    }

    pub fn load_chunk(&mut self, chunk_id: ChunkId, commands: &mut Commands) {
        if let Entry::Vacant(entry) = self.loaded_chunks.entry(chunk_id) {
            log::debug!("Chunk {:?} load was requested", chunk_id);
            entry.insert(
                commands
                    .spawn_empty()
                    .insert(chunk_id)
                    .insert(PersistedVersion::new(0))
                    .id(),
            );
        }

        self.load_requests.push_back((chunk_id, self.config.max_retry_count()));
    }

    pub fn refresh_chunk(&mut self, chunk_id: ChunkId) {
        if self.loaded_chunks.contains_key(&chunk_id) {
            self.load_requests.push_back((chunk_id, self.config.max_retry_count()));
        }
    }

    pub fn unload_chunk(&mut self, chunk_id: ChunkId, commands: &mut Commands) {
        if let Some(entity) = self.loaded_chunks.remove(&chunk_id) {
            commands.entity(entity).despawn();
        }
        self.load_requests.retain(|(id, _)| *id != chunk_id);
        self.loading_tasks.remove(&chunk_id);
        //self.local_commands.remove(&chunk_id);
    }

    pub fn get_chunk_entity(&self, chunk_id: ChunkId) -> Option<Entity> {
        self.loaded_chunks.get(&chunk_id).cloned()
    }
}

pub fn start_chunk_load_system<C>(mut tile_map: ResMut<TileMap<C>>, chunks: Query<&PersistedVersion>)
where
    C: TileMapConfig,
{
    let task_pool = AsyncComputeTaskPool::get();
    let count = tile_map.load_requests.len();

    let TileMap {
        config,
        factory,
        load_requests,
        loading_tasks,
        loaded_chunks,
        ..
    } = tile_map.as_mut();

    for _ in 0..count {
        let (chunk_id, retry_count) = load_requests.pop_front().expect("Failed to pop chunk load request");
        log::debug!("Processing load request for chunk {:?}", chunk_id);

        if loading_tasks.contains_key(&chunk_id) {
            log::info!("Chunk {:?} is loading, differ the task", chunk_id);
            load_requests.push_back((chunk_id, retry_count));
            continue;
        }
        let chunk_version = match loaded_chunks.get(&chunk_id).and_then(|entity| chunks.get(*entity).ok()) {
            Some(chunk) => chunk.version,
            None => {
                log::warn!("Chunk {:?} is requested, but was removed from the tile map", chunk_id);
                continue;
            }
        };

        let config = config.clone();
        let factory = factory.clone();

        let task = if chunk_version > 0 {
            log::info!(
                "Chunk {:?} is already loaded with version {}, requesting updates",
                chunk_id,
                chunk_version
            );
            task_pool.spawn(async move {
                log::debug!(
                    "Start loading updates for chunk {:?} from version {}",
                    chunk_id,
                    chunk_version
                );
                match factory.read_updates(&config, chunk_id, chunk_version).await {
                    Ok(updates) => TaskResult::Commands(updates),
                    Err(TileMapError::ChunkNotFound) => TaskResult::Empty,
                    #[cfg(feature = "persisted")]
                    Err(_) => TaskResult::Retry(retry_count.saturating_sub(1)),
                }
            })
        } else {
            task_pool.spawn(async move {
                log::debug!("Start loading chunk {:?} ", chunk_id);
                match factory.read(&config, chunk_id).await {
                    Ok((chunk, version)) => TaskResult::Chunk(PersistedChunk::<C>::new(chunk), version),
                    Err(_) => TaskResult::Retry(retry_count.saturating_sub(1)),
                }
            })
        };
        loading_tasks.insert(chunk_id, task);
    }
}

pub fn complete_chunk_load_system<C>(
    mut tile_map: ResMut<TileMap<C>>,
    mut chunk_commands: Query<&mut PersistedChunkUpdate<C>>,
    mut commands: Commands,
) where
    C: TileMapConfig,
{
    let TileMap {
        config,
        loading_tasks,
        loaded_chunks,
        load_requests,
        ..
    } = tile_map.as_mut();

    loading_tasks.retain(|chunk_id, task| {
        let status = block_on(future::poll_once(task));
        let retain = status.is_none();

        let entity = match loaded_chunks.get_mut(chunk_id) {
            Some(entry) => entry,
            None => {
                log::warn!("Chunk {:?} is not loaded, ignoring task result", chunk_id);
                return false;
            }
        };

        if let Some(task_result) = status {
            match task_result {
                TaskResult::Chunk(chunk, version) => {
                    log::debug!("Chunk {:?} loaded successfully", chunk_id);
                    commands
                        .entity(*entity)
                        .insert(chunk)
                        .insert(PersistedVersion::new(version));
                }
                TaskResult::Commands(cmds) => {
                    log::debug!("Chunk {:?} updates loaded successfully", chunk_id);
                    if let Ok(mut chunk_command) = chunk_commands.get_mut(*entity) {
                        chunk_command.extend(cmds);
                    } else {
                        commands.entity(*entity).insert(PersistedChunkUpdate::<C>::new(cmds));
                    }
                }
                TaskResult::Empty => {
                    log::debug!("Chunk {:?} is emptied", chunk_id);
                    commands
                        .entity(*entity)
                        .insert(PersistedChunk::<C>::new_empty(config.chunk_size()))
                        .insert(PersistedVersion::new(0));
                }
                TaskResult::Retry(retry_left) => {
                    if retry_left > 0 {
                        log::warn!("Failed to load chunk ({:?}), retry left: {}", chunk_id, retry_left);
                        load_requests.push_back((*chunk_id, retry_left));
                    } else {
                        log::error!("Failed to load chunk ({:?})", chunk_id);
                    }
                }
            }
        }

        retain
    });
}

/*pub fn process_refresh_system<C>(mut tile_map: ResMut<Map<C>>, mut local: Local<HashSet<ChunkId>>)
where
    C: MapConfig,
{
    mem::swap(&mut *tile_map.refresh_channel.lock().unwrap(), &mut local);

    for chunk_id in local.drain() {
        tile_map.refresh_chunk(chunk_id);
    }
}*/

pub fn process_commands_system<C>(
    mut chunks: Query<(
        &ChunkId,
        &mut PersistedVersion,
        &mut PersistedChunk<C>,
        &mut PersistedChunkUpdate<C>,
    )>,
    mut commands: Local<Vec<PersistedChunkCommand<C>>>,
) where
    C: TileMapConfig,
{
    for (chunk_id, mut version, mut chunk, mut updates) in chunks.iter_mut() {
        for command in updates.drain(..) {
            if command.version == version.version + 1 {
                command.operation.apply(&mut **chunk);
                **version = command.version;
            } else if command.version > **version {
                log::info!(
                    "Command is too early ({}) for chunk {:?} at version {}",
                    command.version,
                    chunk_id,
                    **version
                );
                commands.push(command);
            } else {
                log::debug!(
                    "Command is too late ({}) for chunk {:?} at version {}",
                    command.version,
                    chunk_id,
                    **version
                );
            }
        }
        std::mem::swap(&mut *commands, &mut updates.commands);
        assert!(commands.is_empty());
    }
}
