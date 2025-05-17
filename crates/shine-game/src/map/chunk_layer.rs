use crate::map::{ChunkEvent, ChunkHashTrack, ChunkHasher, ChunkId, ChunkRoot, MapChunk};
use bevy::ecs::{
    entity::Entity,
    event::EventWriter,
    query::{Added, Without},
    removal_detection::RemovedComponents,
    resource::Resource,
    system::{Commands, Query, Res, ResMut},
};
use std::{collections::HashMap, marker::PhantomData};

/// Resource to track a layer of loaded chunks.
#[derive(Resource)]
pub struct ChunkLayer<C>
where
    C: MapChunk,
{
    chunks_to_entity: HashMap<ChunkId, Entity>,
    entity_to_chunk: HashMap<Entity, ChunkId>,
    ph: PhantomData<C>,
}

impl<C> Default for ChunkLayer<C>
where
    C: MapChunk,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C> ChunkLayer<C>
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

    /// Get the entity storing the chunk component for the given chunk id.
    pub fn get_entity(&self, chunk_id: ChunkId) -> Option<Entity> {
        self.chunks_to_entity.get(&chunk_id).cloned()
    }

    /// Get the chunk id from the entity. Consider using the ChunkRoot component instead if layer is
    /// attached to the root entity, that is more efficient.
    pub fn get_chunk_id(&self, entity: Entity) -> Option<ChunkId> {
        self.entity_to_chunk.get(&entity).cloned()
    }
}

/// Create a new layer component for the chunk when a new chunk-entity is spawned.
/// The chunk is created as empty and hence it is only a placeholder. The chunk is marked completed(loaded) when
/// a ChunkCommand::Data or ChunkCommand::Empty is received.
#[allow(clippy::type_complexity)]
pub fn create_layer_system<C, H>(
    mut chunk_layer: ResMut<ChunkLayer<C>>,
    hasher: Option<Res<H>>,
    new_entities: Query<(Entity, &ChunkRoot), (Added<ChunkRoot>, Without<C>)>,
    mut commands: Commands,
    mut events: EventWriter<ChunkEvent<C>>,
) where
    C: MapChunk,
    H: ChunkHasher<C>,
{
    // The ChunkRoot is added only when the chunk is created, thus we can use it as a trigger for the layer-component creation.
    for (entity, chunk_root) in new_entities.iter() {
        log::debug!("Chunk [{:?}]: Create {} layer", chunk_root.id, C::name());
        let mut command = commands.entity(entity);
        command.insert(C::new_empty());
        if hasher.is_some() {
            command.insert(ChunkHashTrack::<C, H>::new());
        }
        chunk_layer.chunks_to_entity.insert(chunk_root.id, entity);
        chunk_layer.entity_to_chunk.insert(entity, chunk_root.id);
        events.write(ChunkEvent::Track { id: chunk_root.id });
    }
}

/// Remove the layer component from the chunk when the chunk-entity is despawned.
/// This is just a minimal bookkeeping as the component has already been removed with the entity.
pub fn remove_layer_system<C>(
    mut chunk_layer: ResMut<ChunkLayer<C>>,
    mut removed_entities: RemovedComponents<ChunkRoot>,
    mut events: EventWriter<ChunkEvent<C>>,
) where
    C: MapChunk,
{
    // The ChunkRoot is removed only when the chunk is despawned, thus we can use it as a trigger for the layer-component removal.
    for entity in removed_entities.read() {
        if let Some(chunk_id) = chunk_layer.entity_to_chunk.remove(&entity) {
            log::debug!("Chunk [{:?}]: Remove {} layer", chunk_id, C::name());
            chunk_layer.chunks_to_entity.remove(&chunk_id);
            // commands.entity(entity).remove::<C>(); - It would causes warning as entity has been released
            events.write(ChunkEvent::Untrack { id: chunk_id });
        }
    }
}
