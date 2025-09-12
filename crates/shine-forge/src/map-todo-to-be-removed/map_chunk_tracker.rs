use crate::map::{ChunkId, ChunkRoot};
use bevy::ecs::{entity::Entity, resource::Resource, system::Commands};
use std::collections::{hash_map::Entry, HashMap};

/// Tracks the loaded chunks in the world
#[derive(Resource)]
pub struct MapChunkTracker {
    pub chunks: HashMap<ChunkId, Entity>,
}

impl Default for MapChunkTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MapChunkTracker {
    pub fn new() -> Self {
        Self { chunks: HashMap::new() }
    }

    pub fn load_chunk(&mut self, chunk_id: ChunkId, commands: &mut Commands) {
        if let Entry::Vacant(entry) = self.chunks.entry(chunk_id) {
            let entity = commands.spawn_empty().insert(ChunkRoot { id: chunk_id }).id();
            entry.insert(entity);
            log::debug!("Chunk [{chunk_id:?}]: Spawned chunk: {entity:?}");
        }
    }

    pub fn unload_chunk(&mut self, chunk_id: ChunkId, commands: &mut Commands) {
        if let Some(entity) = self.chunks.remove(&chunk_id) {
            log::debug!("Chunk [{chunk_id:?}]: Despawn chunk: {entity:?}");
            commands.entity(entity).despawn();
        }
    }

    pub fn get_entity(&self, chunk_id: ChunkId) -> Option<Entity> {
        self.chunks.get(&chunk_id).cloned()
    }
}
