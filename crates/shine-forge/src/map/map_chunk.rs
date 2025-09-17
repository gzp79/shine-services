use crate::map::MapEvent;
use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        event::EventReader,
        resource::Resource,
        system::{Commands, ResMut},
    },
    log,
};
use std::collections::{hash_map::Entry, HashMap};

/// Unique identifier of a chunk of the map.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MapChunkId(pub usize, pub usize);

/// Relationship component to attach multiple layers of a chunk to the root.
#[derive(Component, Default)]
#[relationship_target(relationship = MapLayerOf, linked_spawn)]
pub struct MapLayers(Vec<Entity>);

/// Relationship component to link a map layer back to its parent chunk.
#[derive(Component)]
#[relationship(relationship_target = MapLayers)]
pub struct MapLayerOf(pub Entity);

/// A chunk of the map coupling the layers of a specific chunk id.
#[derive(Component)]
#[require(MapLayers)]
pub struct MapChunk {
    pub id: MapChunkId,
}

impl MapChunk {
    pub fn new(id: MapChunkId) -> Self {
        Self { id }
    }
}

/// Tracks the loaded chunks.
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

    pub fn get_entity(&self, chunk_id: MapChunkId) -> Option<Entity> {
        self.chunks.get(&chunk_id).cloned()
    }

    fn load_chunk(&mut self, chunk_id: MapChunkId, commands: &mut Commands) {
        if let Entry::Vacant(entry) = self.chunks.entry(chunk_id) {
            let entity = commands.spawn_empty().insert(MapChunk::new(chunk_id)).id();
            entry.insert(entity);
            log::debug!("Chunk [{chunk_id:?}]: Spawned chunk: {entity:?}");
        }
    }

    fn unload_chunk(&mut self, chunk_id: MapChunkId, commands: &mut Commands) {
        if let Some(entity) = self.chunks.remove(&chunk_id) {
            log::debug!("Chunk [{chunk_id:?}]: Despawn chunk: {entity:?}");
            commands.entity(entity).despawn();
        }
    }
}

/// Process 'MapEvent' events.
pub fn process_map_event(
    mut chunk_tracker: ResMut<MapChunkTracker>,
    mut map_events: EventReader<MapEvent>,
    mut commands: Commands,
) {
    for event in map_events.read() {
        log::debug!("Processing TileMapEvent: {event:?}");
        match event {
            MapEvent::Load(chunk_id) => {
                chunk_tracker.load_chunk(*chunk_id, &mut commands);
            }
            MapEvent::Unload(chunk_id) => {
                chunk_tracker.unload_chunk(*chunk_id, &mut commands);
            }
        }
    }
}
