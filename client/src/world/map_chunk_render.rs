use bevy::{
    ecs::{
        component::Component,
        entity::Entity,
        lifecycle::{Add, Remove},
        name::Name,
        observer::On,
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

    /*pub fn get_chunk_id(&self, chunk_render: Entity) -> Option<MapChunkId> {
        self.entity_to_chunk.get(&chunk_render).cloned()
    }*/

    pub(in crate::world) fn track(&mut self, chunk_id: MapChunkId, chunk_render: Entity) {
        self.chunks_to_entity.insert(chunk_id, chunk_render);
        self.entity_to_chunk.insert(chunk_render, chunk_id);
    }

    pub(in crate::world) fn untrack(&mut self, chunk_id: MapChunkId) -> Option<Entity> {
        if let Some(chunk_render) = self.chunks_to_entity.remove(&chunk_id) {
            self.entity_to_chunk.remove(&chunk_render).unwrap();
            Some(chunk_render)
        } else {
            None
        }
    }
}

/// Create chunk render and performs some book-keeping when a new chunk root is spawned.
pub fn create_chunk_render(
    new_chunk_trigger: On<Add, MapChunk>,
    mut chunk_render_tracker: ResMut<MapChunkRenderTracker>,
    chunk_root_q: Query<&MapChunk>,
    mut commands: Commands,
) {
    let chunk_root_entity = new_chunk_trigger.entity;
    let chunk_root = chunk_root_q.get(chunk_root_entity).unwrap();
    log::debug!("Chunk [{:?}]: Create chunk render", chunk_root.id);

    let chunk_render = commands
        .spawn((
            Name::new(format!("ChunkRender({:?})", chunk_root.id)),
            MapChunkRender::new(chunk_root.id),
            Transform::IDENTITY, // todo: use MapChunkId::relative to position it correctly
        ))
        .id();
    chunk_render_tracker.track(chunk_root.id, chunk_render);
}

/// When a chunk root is despawned, despawn the chunk render (with all the children)
pub fn remove_chunk_render(
    removed_chunk_trigger: On<Remove, MapChunk>,
    mut chunk_render_tracker: ResMut<MapChunkRenderTracker>,
    chunk_root_q: Query<&MapChunk>,
    mut commands: Commands,
) {
    let chunk_root_entity = removed_chunk_trigger.entity;
    let chunk_root = chunk_root_q.get(chunk_root_entity).unwrap();
    log::debug!("Chunk [{:?}]: Remove chunk render", chunk_root.id);

    if let Some(chunk_render) = chunk_render_tracker.untrack(chunk_root.id) {
        commands.entity(chunk_render).despawn();
    }
}
