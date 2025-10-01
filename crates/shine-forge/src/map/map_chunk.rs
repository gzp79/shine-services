use crate::map::{AxialCoord, MapMessage};
use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        message::MessageReader,
        name::Name,
        resource::Resource,
        system::{Commands, ResMut},
    },
    log,
};
use std::collections::{hash_map::Entry, HashMap};

/// Unique identifier of a chunk of the map.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct MapChunkId(pub usize, pub usize);

impl MapChunkId {
    /// Return the relative axial coordinate of a chunk id relative to this chunk id.
    /// This function interprets the chunk coordinates as the q,r components of the axial coordinates.
    pub fn relative_axial_coord(&self, id: MapChunkId) -> AxialCoord {
        let dx = id.0 as isize - self.0 as isize;
        let dy = id.1 as isize - self.1 as isize;
        AxialCoord::new(dx.try_into().unwrap(), dy.try_into().unwrap())
    }
}

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
}

/// Process 'MapMessage' messages.
pub fn process_map_messages(
    mut chunk_tracker: ResMut<MapChunkTracker>,
    mut map_messages: MessageReader<MapMessage>,
    mut commands: Commands,
) {
    for message in map_messages.read() {
        log::debug!("Processing MapMessage: {message:?}");
        match message {
            MapMessage::Load(chunk_id) => {
                if let Entry::Vacant(entry) = chunk_tracker.chunks.entry(*chunk_id) {
                    let entity = commands
                        .spawn((
                            Name::new(format!("MapChunk({},{})", chunk_id.0, chunk_id.1)),
                            MapChunk::new(*chunk_id),
                        ))
                        .id();
                    entry.insert(entity);
                    log::debug!("Chunk [{chunk_id:?}]: Spawned chunk: {entity:?}");
                }
            }
            MapMessage::Unload(chunk_id) => {
                if let Some(entity) = chunk_tracker.chunks.remove(chunk_id) {
                    log::debug!("Chunk [{chunk_id:?}]: Despawn chunk: {entity:?}");
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}
