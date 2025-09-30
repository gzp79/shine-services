use crate::world::ChunkRender;
use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        name::Name,
        query::{Added, Without},
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
        }
    }

    pub fn get_entity(&self, chunk_id: MapChunkId) -> Option<Entity> {
        self.chunks_to_entity.get(&chunk_id).cloned()
    }

    pub fn get_chunk_id(&self, root: Entity) -> Option<MapChunkId> {
        self.entity_to_chunk.get(&root).cloned()
    }

    pub(in crate::map) fn track(&mut self, chunk_id: MapChunkId, chunk_render: Entity) {
        self.chunks_to_entity.insert(chunk_id, chunk_render);
        self.entity_to_chunk.insert(chunk_render, chunk_id);
    }

    pub(in crate::map) fn untrack(&mut self, chunk_render: &Entity) -> Option<MapChunkId> {
        if let Some(id) = self.entity_to_chunk.remove(chunk_render) {
            self.chunks_to_entity.remove(&id);
            Some(id)
        } else {
            None
        }
    }
}

/// Create chunk render and performs some book-keeping when a new chunk root is spawned.
pub fn create_chunk_render(
    mut chunk_render_tracker: ResMut<MapChunkRenderTracker>,
    new_root_query: Query<&MapChunk, (Added<MapChunk>, Without<ChunkRender>)>,
    mut commands: Commands,
) {
    for chunk_root in new_root_query.iter() {
        log::debug!("Chunk [{:?}]: Create chunk render", chunk_root.id);

        let entity = commands
            .spawn((
                Name::new(format!("ChunkRender({:?})", chunk_root.id)),
                MapChunkRender::new(chunk_root.id),
                Transform::IDENTITY, // todo: use MapChunkId::relative to position it correctly
            ))
            .id();
        chunk_render_tracker.track(chunk_root.id, entity);
    }
}

/// When a chunk is despawned, perform some cleanup.
pub fn remove_chunk_render(
    mut chunk_render_tracker: ResMut<MapChunkRenderTracker>,
    mut removed_component: RemovedComponents<MapChunkRender>,
) {
    for entity in removed_component.read() {
        if let Some(chunk_id) = chunk_render_tracker.untrack(&entity) {
            log::debug!("Chunk [{chunk_id:?}]: Remove chunk render");
        }
    }
}
