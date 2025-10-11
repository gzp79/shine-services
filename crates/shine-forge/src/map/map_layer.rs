use crate::map::MapChunkId;
use bevy::ecs::{
    component::{Component, Mutable},
    entity::Entity,
    resource::Resource,
};
use std::{collections::HashMap, marker::PhantomData};

pub trait MapLayerConfig: Resource + Clone + Send + Sync + 'static {}

/// Trait to define a layer of a chunk.
pub trait MapLayer: Component<Mutability = Mutable> + 'static {
    type Config: MapLayerConfig;

    fn new() -> Self
    where
        Self: Sized;

    /// Check if the layer is empty (i.e. cleared and has not been initialized).
    fn is_empty(&self) -> bool;

    /// Clears the layer, resetting it to an empty and uninitialized state.
    fn clear(&mut self);

    /// Initializes the layer with the provided configuration, setting it to a default, ready-to-use state.
    /// This can be called multiple times to reconfigure the layer.
    fn initialize(&mut self, config: &Self::Config);
}

/// Map layer with change tracking capabilities.
/// The change tracking is operation dependent, but usually some dirty flag or a list of changed coordinates is used.
pub trait MapAuditedLayer: MapLayer {
    type Audit: MapLayer<Config = Self::Config>;
}

/// Resource to track a layer of a chunk.
#[derive(Resource)]
pub struct MapLayerTracker<L>
where
    L: MapLayer,
{
    chunks_to_entity: HashMap<MapChunkId, Entity>,
    entity_to_chunk: HashMap<Entity, MapChunkId>,
    ph: PhantomData<L>,
}

impl<L> Default for MapLayerTracker<L>
where
    L: MapLayer,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<L> MapLayerTracker<L>
where
    L: MapLayer,
{
    pub fn new() -> Self {
        Self {
            chunks_to_entity: HashMap::new(),
            entity_to_chunk: HashMap::new(),
            ph: PhantomData,
        }
    }

    pub fn get_entity(&self, chunk_id: MapChunkId) -> Option<Entity> {
        self.chunks_to_entity.get(&chunk_id).cloned()
    }

    pub fn get_chunk_id(&self, root: Entity) -> Option<MapChunkId> {
        self.entity_to_chunk.get(&root).cloned()
    }

    pub(in crate::map) fn track(&mut self, chunk_id: MapChunkId, layer_entity: Entity) {
        self.chunks_to_entity.insert(chunk_id, layer_entity);
        self.entity_to_chunk.insert(layer_entity, chunk_id);
    }

    pub(in crate::map) fn untrack(&mut self, layer_entity: &Entity) -> Option<MapChunkId> {
        if let Some(id) = self.entity_to_chunk.remove(layer_entity) {
            self.chunks_to_entity.remove(&id);
            Some(id)
        } else {
            None
        }
    }
}
