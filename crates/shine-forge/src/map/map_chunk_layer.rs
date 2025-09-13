use crate::map::{MapChunk, MapChunkId};
use bevy::ecs::{component::Component, entity::Entity, resource::Resource};
use std::{collections::HashMap, marker::PhantomData};

/// The chunk root collecting the layers of a specific chunk id.
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

/// Relationship component to attach multiple layers of a chunk to the root.
#[derive(Component, Default)]
#[relationship_target(relationship = MapLayerOf, linked_spawn)]
pub struct MapLayers(Vec<Entity>);

/// Relationship component to link a map layer back to its parent root.
#[derive(Component)]
#[relationship(relationship_target = MapLayers)]
pub struct MapLayerOf(Entity);

/// Resource to track a single layer of a chunk of the map.
#[derive(Resource)]
pub struct MapLayer<C>
where
    C: MapChunk,
{
    chunks_to_entity: HashMap<MapChunkId, Entity>,
    entity_to_chunk: HashMap<Entity, MapChunkId>,
    ph: PhantomData<C>,
}

impl<C> Default for MapLayer<C>
where
    C: MapChunk,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C> MapLayer<C>
where
    C: MapChunk,
{
    pub fn new() -> Self {
        Self {
            chunks_to_entity: HashMap::new(),
            entity_to_chunk: HashMap::new(),
            ph: PhantomData,
        }
    }

    /// Get the root entity for the given chunk id.
    pub fn get_root(&self, chunk_id: MapChunkId) -> Option<Entity> {
        self.chunks_to_entity.get(&chunk_id).cloned()
    }

    /// Get the chunk id from the root.
    /// Consider using the ChunkRoot component directly as that is more efficient.
    pub fn get_chunk_id(&self, root: Entity) -> Option<MapChunkId> {
        self.entity_to_chunk.get(&root).cloned()
    }
}
