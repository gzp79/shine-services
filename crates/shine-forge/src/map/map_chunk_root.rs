use crate::map::MapLayers;
use bevy::{
    ecs::{component::Component, entity::Entity, resource::Resource, system::Commands},
    log,
};
use std::collections::{hash_map::Entry, HashMap};

/// Unique identifier for a chunk in the world.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MapChunkId(pub usize, pub usize);

/// The root of a chunk coupling the layers of a specific chunk id.
/// A 'MapChunk' can be added either as a component to entity with a 'MapChunkRoot' component or
/// as a child using the 'MapLayers' relationship.
#[derive(Component)]
#[require(MapLayers)]
pub struct MapChunkRoot {
    pub id: MapChunkId,
}

impl MapChunkRoot {
    pub fn new(id: MapChunkId) -> Self {
        Self { id }
    }
}

/// Tracks the loaded chunks in the world through a map from chunk id to the root entity.
#[derive(Resource)]
pub struct MapChunkTracker {
    pub chunks: HashMap<MapChunkId, Entity>,
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

    pub fn load_chunk(&mut self, chunk_id: MapChunkId, commands: &mut Commands) {
        if let Entry::Vacant(entry) = self.chunks.entry(chunk_id) {
            let entity = commands.spawn_empty().insert(MapChunkRoot::new(chunk_id)).id();
            entry.insert(entity);
            log::debug!("Chunk [{chunk_id:?}]: Spawned chunk: {entity:?}");
        }
    }

    pub fn unload_chunk(&mut self, chunk_id: MapChunkId, commands: &mut Commands) {
        if let Some(entity) = self.chunks.remove(&chunk_id) {
            log::debug!("Chunk [{chunk_id:?}]: Despawn chunk: {entity:?}");
            commands.entity(entity).despawn();
        }
    }

    pub fn get_entity(&self, chunk_id: MapChunkId) -> Option<Entity> {
        self.chunks.get(&chunk_id).cloned()
    }
}
