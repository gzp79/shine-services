use crate::map::{ChunkCommandQueue, ChunkHashTrack, ChunkHasher, ChunkId, ChunkRoot, ChunkStore};
use bevy::ecs::{
    entity::Entity,
    query::{Added, Without},
    removal_detection::RemovedComponents,
    resource::Resource,
    system::{Commands, Query, ResMut},
};
use std::collections::HashMap;

/// Resource to track the loaded chunks of the given layer.
#[derive(Resource)]
pub struct ChunkLayer<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    hasher: Option<H>,
    command_queue: ChunkCommandQueue<C, H>,
    chunks_to_entity: HashMap<ChunkId, Entity>,
    entity_to_chunk: HashMap<Entity, ChunkId>,
}

impl<C, H> ChunkLayer<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    pub fn new(hasher: Option<H>, commands: ChunkCommandQueue<C, H>) -> Self {
        Self {
            hasher,
            command_queue: commands,
            chunks_to_entity: HashMap::new(),
            entity_to_chunk: HashMap::new(),
        }
    }

    pub fn hasher(&self) -> &H {
        self.hasher.as_ref().expect("Hasher is not set")
    }

    pub fn command_queue(&self) -> &ChunkCommandQueue<C, H> {
        &self.command_queue
    }

    pub fn get_entity(&self, chunk_id: ChunkId) -> Option<Entity> {
        self.chunks_to_entity.get(&chunk_id).cloned()
    }

    /// Get the chunk id from the entity. Consider using the ChunkRoot component instead, that is more efficient.
    /// This function is only useful during despawn as the ChunkRoot component has been already removed.
    pub fn get_chunk_id(&self, entity: Entity) -> Option<ChunkId> {
        self.entity_to_chunk.get(&entity).cloned()
    }
}

/// Create a new layer component for the chunk when a new chunk-entity is spawned.
/// The chunk is created as empty and hence it is only a placeholder. The chunk is marked completed(loaded) when
/// a ChunkCommand::Data or ChunkCommand::Empty is received.
pub fn create_layer_system<C, H>(
    mut chunk_layer: ResMut<ChunkLayer<C, H>>,
    new_entities: Query<(Entity, &ChunkRoot), (Added<ChunkRoot>, Without<C>)>,
    mut commands: Commands,
) where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    // The ChunkRoot is added only when the chunk is created, thus we can use it as a trigger for the layer-component creation.
    for (entity, chunk_root) in new_entities.iter() {
        log::debug!("Chunk [{:?}]: Create {} layer", chunk_root.id, C::NAME);
        let mut command = commands.entity(entity);
        command.insert(C::new_empty());
        if chunk_layer.hasher.is_some() {
            command.insert(ChunkHashTrack::<C, H>::new());
        }
        chunk_layer.chunks_to_entity.insert(chunk_root.id, entity);
        chunk_layer.entity_to_chunk.insert(entity, chunk_root.id);
        // todo: request load
        // client: send Track Chunk
        // server: read snapshot from DB
    }
}

/// Remove the layer component from the chunk when the chunk-entity is despawned.
/// This is just a minimal bookkeeping as the component has already been removed with the entity.
pub fn remove_layer_system<C, H>(
    mut chunk_layer: ResMut<ChunkLayer<C, H>>,
    mut removed_entities: RemovedComponents<ChunkRoot>,
) where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    // The ChunkRoot is removed only when the chunk is despawned, thus we can use it as a trigger for the layer-component removal.
    for entity in removed_entities.read() {
        if let Some(chunk_id) = chunk_layer.entity_to_chunk.remove(&entity) {
            log::debug!("Chunk [{:?}]: Remove {} layer", chunk_id, C::NAME);
            chunk_layer.chunks_to_entity.remove(&chunk_id);
            // commands.entity(entity).remove::<C>(); - It would causes warning as entity has been released
        }
    }
}
