use crate::map::MapChunkRenderTracker;
use bevy::ecs::{
    component::Component,
    entity::Entity,
    hierarchy::Children,
    query::QueryFilter,
    system::{Local, Query, Res, SystemParam},
};
use shine_forge::map::{AxialCoord, MapChunkId, MapLayerTracker, MapShard, Tile};
use std::{collections::HashMap, marker::PhantomData};

/// Render component for a single tile in a map chunk.
#[derive(Component)]
pub struct MapTileRender<T>
where
    T: Tile,
{
    pub coord: AxialCoord,
    _ph: PhantomData<T>,
}

struct LookupCache {
    chunk_id: Option<MapChunkId>,
    render_root: Option<Entity>,
    tile_layer: Option<Entity>,
    lookup: HashMap<AxialCoord, Entity>,
}

impl Default for LookupCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LookupCache {
    pub fn new() -> Self {
        Self {
            chunk_id: None,
            render_root: None,
            tile_layer: None,
            lookup: HashMap::new(),
        }
    }

    pub fn select(&mut self, chunk_id: MapChunkId, tile_layer: Entity, render_root: Entity) {
        self.chunk_id = Some(chunk_id);
        self.tile_layer = Some(tile_layer);
        self.render_root = Some(render_root);
        self.lookup.clear();
    }

    pub fn clear(&mut self) {
        self.chunk_id = None;
        self.tile_layer = None;
        self.render_root = None;
        self.lookup.clear();
    }

    pub fn get(&self, coord: &AxialCoord) -> Option<&Entity> {
        self.lookup.get(coord)
    }

    pub fn insert(&mut self, coord: AxialCoord, entity: Entity) {
        self.lookup.insert(coord, entity);
    }
}

#[derive(SystemParam)]
pub struct MapRenderTileQuery<'w, 's, S, F = ()>
where
    S: MapShard,
    F: QueryFilter + 'static,
{
    chunk_render_tracker: Res<'w, MapChunkRenderTracker>,
    layer_tracker: Res<'w, MapLayerTracker<<S as MapShard>::Primary>>,
    chunk_render_children: Query<'w, 's, &'static Children, F>,
    layer_render: Query<'w, 's, (Entity, &'static MapTileRender<<S as MapShard>::Tile>), F>,

    lookup_cache: Local<'s, LookupCache>,
}

impl<'w, 's, S, F> MapRenderTileQuery<'w, 's, S, F>
where
    S: MapShard,
    F: QueryFilter + 'static,
{
    /// Selects the chunk to work with by the given layer entity and clears the lookup cache.
    pub fn select_chunk_by_layer(&mut self, layer_entity: Entity) -> bool {
        if let Some(chunk_id) = self.layer_tracker.get_chunk_id(layer_entity) {
            if let Some(render_root) = self.chunk_render_tracker.get_entity(chunk_id) {
                self.lookup_cache.select(chunk_id, layer_entity, render_root);
                log::debug!("Selected  chunk {chunk_id:?} for layer {layer_entity:?} with render root {render_root:?}");
                return true;
            }
        }

        self.lookup_cache.clear();
        false
    }

    /// Selects the chunk to work with by the given chunk id and clears the lookup cache.
    pub fn select_chunk_by_id(&mut self, chunk_id: MapChunkId) -> bool {
        if let Some(layer_entity) = self.layer_tracker.get_entity(chunk_id) {
            if let Some(render_root) = self.chunk_render_tracker.get_entity(chunk_id) {
                self.lookup_cache.select(chunk_id, layer_entity, render_root);
                return true;
            }
        }

        self.lookup_cache.clear();
        false
    }

    pub fn find_tile_render(&mut self, coord: AxialCoord) -> Option<Entity> {
        if let Some(render_root) = self.lookup_cache.render_root {
            if let Some(entity) = self.lookup_cache.get(&coord) {
                return Some(*entity);
            }

            if let Ok(children) = self.chunk_render_children.get(render_root) {
                for child in children.iter() {
                    // Find the tile render for the given coord within the children of the chunk render.
                    if let Ok((entity, render)) = self.layer_render.get(*child) {
                        if render.coord == coord {
                            self.lookup_cache.insert(coord, entity);
                            return Some(entity);
                        }
                    }
                }
            }
        }

        None
    }
}
