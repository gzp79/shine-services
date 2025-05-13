use crate::map::{
    ChunkCommandQueue, ChunkDataUpdate, ChunkHashTrack, ChunkHasher, ChunkId, ChunkOperation, ChunkRoot, ChunkStore,
    TileMap,
};
use bevy::ecs::{
    entity::Entity,
    query::{Added, Without},
    removal_detection::RemovedComponents,
    resource::Resource,
    system::{Commands, Query, Res, ResMut},
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
    commands: ChunkCommandQueue<C, H>,
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
            commands,
            chunks_to_entity: HashMap::new(),
            entity_to_chunk: HashMap::new(),
        }
    }

    pub fn hasher(&self) -> &H {
        self.hasher.as_ref().expect("Hasher is not set")
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

/// Consume the ChunkCommand queue and integrate the commands into the chunk data.
pub fn process_layer_commands_system<C, H>(
    tile_map: Res<TileMap>,
    chunk_layer: Res<ChunkLayer<C, H>>,
    mut chunks: Query<(&ChunkRoot, &mut C, Option<&mut ChunkHashTrack<C, H>>)>,
) where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    let (width, height) = (tile_map.config().width, tile_map.config().height);

    for (chunk_id, mut chunk, mut hash_track) in chunks.iter_mut() {
        let (data, mut operations, drift_detect) = chunk_layer.commands.take_update(chunk_id.id).into_parts();

        // apply whole chunk replacement
        let reset_hash_track = match data {
            ChunkDataUpdate::Data(data) if chunk.version() < data.version() => {
                log::debug!(
                    "Chunk [{:?}]: Replace with a new data at version ({})",
                    chunk_id.id,
                    data.version()
                );
                assert!(data.width() == width && data.height() == height);
                *chunk = data;
                true
            }
            ChunkDataUpdate::Empty if chunk.is_empty() => {
                log::debug!("Chunk [{:?}]: Initialized to empty", chunk_id.id);
                *chunk = C::new(width, height);
                true
            }
            _ => false,
        };
        if reset_hash_track {
            if let Some(hash_track) = hash_track.as_mut() {
                hash_track.clear();
                hash_track.set(chunk.version(), chunk_layer.hasher().hash(&*chunk));
            }
        }

        // apply operations by version
        if !operations.is_empty() {
            log::debug!("Chunk [{:?}]: Applying {} operations", chunk_id.id, operations.len());
            while let Some((version, operation)) = operations.pop_first() {
                if version <= chunk.version() {
                    log::trace!("Chunk [{:?}]: Operation is too old {}, ignoring", chunk_id.id, version);
                } else if version == chunk.version() + 1 {
                    operation.apply(&mut *chunk);
                    *chunk.version_mut() += 1;
                    if let Some(hash_track) = hash_track.as_mut() {
                        hash_track.set(chunk.version(), chunk_layer.hasher().hash(&*chunk));
                    }
                } else {
                    log::debug!(
                        "Chunk [{:?}]: Operation version gap detected: [{}..{})",
                        chunk_id.id,
                        chunk.version() + 1,
                        version
                    );
                    // todo: for client request the missing operations
                    operations.insert(version, operation);
                    break;
                }
            }

            if !operations.is_empty() {
                log::debug!(
                    "Chunk [{:?}]: Storing {} future operations",
                    chunk_id.id,
                    operations.len()
                );
                chunk_layer.commands.store_operations(chunk_id.id, operations);
            }
        }

        if !drift_detect.is_empty() {
            log::debug!(
                "Chunk [{:?}]: Applying {} drift detection hashes",
                chunk_id.id,
                drift_detect.len()
            );
            // todo: when drift detected, request a full reload
        }
    }
}
