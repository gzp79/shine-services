use crate::map2::{Chunk, ChunkCommand, ChunkFactory, ChunkId, ChunkOperation, TileMapConfig, TileMapError};
use bevy::{
    ecs::{
        entity::Entity,
        resource::Resource,
        system::{Commands, Local, Query, ResMut},
    },
    platform::sync::Mutex,
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task},
};
use std::{
    collections::{hash_map::Entry, HashMap, HashSet, VecDeque},
    mem,
    sync::Arc,
};

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
    Chunk(Chunk<C>, usize),
    Commands(Vec<ChunkCommand<C>>),
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
    loaded_chunks: HashMap<ChunkId, (Entity, usize)>,
    // command from the server
    server_commands: HashMap<ChunkId, Vec<ChunkCommand<C>>>,
    // local commands, and the version of the chunk at the time of the command
    local_commands: HashMap<ChunkId, Vec<ChunkCommand<C>>>,
    // Channel to notify about updated chunks
    refresh_channel: Arc<Mutex<HashSet<ChunkId>>>,
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
            server_commands: HashMap::new(),
            local_commands: HashMap::new(),
            refresh_channel: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    pub fn config(&self) -> &C {
        &self.config
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
            entry.insert((commands.spawn_empty().id(), 0));
        }

        self.load_requests.push_back((chunk_id, self.config.max_retry_count()));
    }

    pub fn refresh_chunk(&mut self, chunk_id: ChunkId) {
        if self.loaded_chunks.contains_key(&chunk_id) {
            self.load_requests.push_back((chunk_id, self.config.max_retry_count()));
        }
    }

    pub fn unload_chunk(&mut self, chunk_id: ChunkId, commands: &mut Commands) {
        if let Some((entity, _)) = self.loaded_chunks.remove(&chunk_id) {
            commands.entity(entity).despawn();
        }
        self.load_requests.retain(|(id, _)| *id != chunk_id);
        self.loading_tasks.remove(&chunk_id);
        self.server_commands.remove(&chunk_id);
        self.local_commands.remove(&chunk_id);
    }

    pub fn update_chunk(&mut self, chunk_id: ChunkId, operation: C::ChunkOperation) {
        if let Some((_, version)) = self.loaded_chunks.get(&chunk_id) {
            self.local_commands.entry(chunk_id).or_default().push(ChunkCommand {
                version: *version,
                operation,
            });
        }
    }

    pub fn get_chunk_entity(&self, chunk_id: ChunkId) -> Option<Entity> {
        self.loaded_chunks.get(&chunk_id).map(|(entity, _)| *entity)
    }
}

pub fn start_chunk_load_system<C>(mut tile_map: ResMut<TileMap<C>>)
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
        let chunk_version = match loaded_chunks.get(&chunk_id) {
            Some((_, version)) => *version,
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
                    Ok((chunk, version)) => TaskResult::Chunk(chunk, version),
                    Err(_) => TaskResult::Retry(retry_count.saturating_sub(1)),
                }
            })
        };
        loading_tasks.insert(chunk_id, task);
    }
}

pub fn complete_chunk_load_system<C>(mut tile_map: ResMut<TileMap<C>>, mut commands: Commands)
where
    C: TileMapConfig,
{
    let TileMap {
        config,
        loading_tasks,
        loaded_chunks,
        load_requests,
        server_commands: chunk_updates,
        ..
    } = tile_map.as_mut();

    loading_tasks.retain(|chunk_id, task| {
        let status = block_on(future::poll_once(task));
        let retain = status.is_none();

        let (entity, version) = match loaded_chunks.get_mut(chunk_id) {
            Some(entry) => entry,
            None => {
                log::warn!("Chunk {:?} is not loaded, ignoring task result", chunk_id);
                return false;
            }
        };

        if let Some(task_result) = status {
            match task_result {
                TaskResult::Chunk(chunk, ver) => {
                    log::debug!("Chunk {:?} loaded successfully", chunk_id);
                    commands.entity(*entity).insert(chunk);
                    *version = ver;
                }
                TaskResult::Commands(cmds) => {
                    log::debug!("Chunk {:?} updates loaded successfully", chunk_id);
                    chunk_updates.entry(*chunk_id).or_default().extend(cmds);
                }
                TaskResult::Empty => {
                    log::debug!("Chunk {:?} is emptied", chunk_id);
                    commands.entity(*entity).insert(Chunk::<C>::new(config.chunk_size()));
                    *version = 0;
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

pub fn process_refresh_system<C>(mut tile_map: ResMut<TileMap<C>>, mut local: Local<HashSet<ChunkId>>)
where
    C: TileMapConfig,
{
    mem::swap(&mut *tile_map.refresh_channel.lock().unwrap(), &mut local);

    for chunk_id in local.drain() {
        tile_map.refresh_chunk(chunk_id);
    }
}

pub fn process_commands_system<C>(mut tile_map: ResMut<TileMap<C>>, mut chunks: Query<&mut Chunk<C>>)
where
    C: TileMapConfig,
{
    let TileMap {
        loaded_chunks,
        server_commands,
        local_commands: local_operation,
        ..
    } = tile_map.as_mut();

    for (chunk_id, commands) in server_commands.iter_mut() {
        if let Some((entity, version)) = loaded_chunks.get_mut(chunk_id) {
            if let Ok(mut chunk) = chunks.get_mut(*entity) {
                commands.sort_by(|a, b| a.version.cmp(&b.version));
                commands.retain_mut(|command| {
                    if command.version == *version + 1 {
                        command.operation.apply(&mut chunk);
                        *version = command.version;
                        false
                    } else if command.version > *version {
                        log::info!(
                            "Command is too early ({}) for chunk {:?} at version {}",
                            command.version,
                            chunk_id,
                            version
                        );
                        true
                    } else {
                        log::debug!(
                            "Command is too late ({}) for chunk {:?} at version {}",
                            command.version,
                            chunk_id,
                            version
                        );
                        false
                    }
                });
            }
        }
    }

    for (chunk_id, commands) in local_operation.iter_mut() {
        if let Some((entity, version)) = loaded_chunks.get_mut(chunk_id) {
            if let Ok(mut chunk) = chunks.get_mut(*entity) {
                commands.sort_by(|a, b| a.version.cmp(&b.version));
                commands.retain_mut(|command| {
                    if command.version == *version + 1 {
                        command.operation.apply_local(&mut chunk, *version);
                        *version = command.version;
                        false
                    } else if command.version > *version {
                        log::info!(
                            "Command is too early ({}) for chunk {:?} at version {}",
                            command.version,
                            chunk_id,
                            version
                        );
                        true
                    } else {
                        log::debug!(
                            "Command is too late ({}) for chunk {:?} at version {}",
                            command.version,
                            chunk_id,
                            version
                        );
                        false
                    }
                });
            }
        }
    }
}
