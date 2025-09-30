use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        name::Name,
        query::Added,
        removal_detection::RemovedComponents,
        resource::Resource,
        system::{Commands, Query, ResMut},
    },
    transform::components::Transform,
};
use shine_forge::map::{MapChunk, MapChunkId};
use std::collections::HashMap;

/// Rendering root of a map chunk.
/// It is similar to `MapChunk`, but exists only for rendering purposes usually on the client side.
#[derive(Component)]
pub struct MapChunkRender {
    pub chunk_id: MapChunkId,
}

impl MapChunkRender {
    pub fn new(chunk_id: MapChunkId) -> Self {
        Self { chunk_id }
    }
}

/// Tracks the loaded chunks for rendering
#[derive(Resource)]
pub struct MapChunkRenderTracker {
    chunks_to_entity: HashMap<MapChunkId, Entity>,
    entity_to_chunk: HashMap<Entity, MapChunkId>,
    chunk_root_to_entity: HashMap<Entity, Entity>,
}

impl Default for MapChunkRenderTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl MapChunkRenderTracker {
    pub fn new() -> Self {
        Self {
            chunks_to_entity: HashMap::new(),
            entity_to_chunk: HashMap::new(),
            chunk_root_to_entity: HashMap::new(),
        }
    }

    pub fn get_entity(&self, chunk_id: MapChunkId) -> Option<Entity> {
        self.chunks_to_entity.get(&chunk_id).cloned()
    }

    /*pub fn get_root_entity(&self, chunk_render: Entity) -> Option<Entity> {
        self.chunk_root_to_entity.get(&chunk_render).cloned()
    }

    pub fn get_chunk_id(&self, chunk_render: Entity) -> Option<MapChunkId> {
        self.entity_to_chunk.get(&chunk_render).cloned()
    }*/

    pub(in crate::world) fn track(&mut self, chunk_id: MapChunkId, chunk_root: Entity, chunk_render: Entity) {
        self.chunks_to_entity.insert(chunk_id, chunk_render);
        self.entity_to_chunk.insert(chunk_render, chunk_id);
        self.chunk_root_to_entity.insert(chunk_root, chunk_render);
    }

    pub(in crate::world) fn untrack(&mut self, chunk_root: &Entity) -> Option<(MapChunkId, Entity)> {
        if let Some(chunk_render) = self.chunk_root_to_entity.remove(chunk_root) {
            let chunk_id = self.entity_to_chunk.remove(&chunk_render).unwrap();
            let chunk_render = self.chunks_to_entity.remove(&chunk_id).unwrap();
            Some((chunk_id, chunk_render))
        } else {
            None
        }
    }
}

/// Create chunk render and performs some book-keeping when a new chunk root is spawned.
pub fn create_chunk_render(
    mut chunk_render_tracker: ResMut<MapChunkRenderTracker>,
    new_chunk_root_q: Query<(Entity, &MapChunk), Added<MapChunk>>,
    mut commands: Commands,
) {
    for (chunk_root_entity, chunk_root) in new_chunk_root_q.iter() {
        log::debug!("Chunk [{:?}]: Create chunk render", chunk_root.id);

        let chunk_render = commands
            .spawn((
                Name::new(format!("ChunkRender({:?})", chunk_root.id)),
                MapChunkRender::new(chunk_root.id),
                Transform::IDENTITY, // todo: use MapChunkId::relative to position it correctly
            ))
            .id();
        chunk_render_tracker.track(chunk_root.id, chunk_root_entity, chunk_render);
    }
}

/// When a chunk is despawned, perform some cleanup.
pub fn remove_chunk_render(
    mut chunk_render_tracker: ResMut<MapChunkRenderTracker>,
    mut removed_chunk_root_q: RemovedComponents<MapChunk>,
    mut commands: Commands,
) {
    for chunk_root_entity in removed_chunk_root_q.read() {
        if let Some((chunk_id, chunk_render)) = chunk_render_tracker.untrack(&chunk_root_entity) {
            log::debug!("Chunk [{chunk_id:?}]: Remove chunk render");
            commands.entity(chunk_render).despawn();
        }
    }
}
