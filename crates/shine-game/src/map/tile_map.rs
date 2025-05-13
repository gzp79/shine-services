use bevy::ecs::{component::Component, entity::Entity, resource::Resource, system::Commands};
use std::collections::{hash_map::Entry, HashMap};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkId(pub usize, pub usize);

#[derive(Component)]
pub struct ChunkRoot {
    pub id: ChunkId,
}

#[derive(Clone, Debug)]
pub struct TileMapConfig {
    pub width: usize,
    pub height: usize,
}

#[derive(Resource)]
pub struct TileMap {
    config: TileMapConfig,
    chunks: HashMap<ChunkId, Entity>,
}

impl TileMap {
    pub fn new(config: TileMapConfig) -> Self {
        Self { config, chunks: HashMap::new() }
    }

    pub fn config(&self) -> &TileMapConfig {
        &self.config
    }

    pub fn load_chunk(&mut self, chunk_id: ChunkId, commands: &mut Commands) {
        if let Entry::Vacant(entry) = self.chunks.entry(chunk_id) {
            let entity = commands.spawn_empty().insert(ChunkRoot { id: chunk_id }).id();
            entry.insert(entity);
            log::debug!("Chunk [{:?}]: Spawned chunk: {:?}", chunk_id, entity);
        }
    }

    pub fn unload_chunk(&mut self, chunk_id: ChunkId, commands: &mut Commands) {
        if let Some(entity) = self.chunks.remove(&chunk_id) {
            log::debug!("Chunk [{:?}]: Despawn chunk: {:?}", chunk_id, entity);
            commands.entity(entity).despawn();
        }
    }

    pub fn get_entity(&self, chunk_id: ChunkId) -> Option<Entity> {
        self.chunks.get(&chunk_id).cloned()
    }
}
