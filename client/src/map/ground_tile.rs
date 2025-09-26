use crate::map::MapRenderTileQuery;
use bevy::ecs::{
    component::Component,
    entity::Entity,
    system::{Commands, Query},
};
use serde::{Deserialize, Serialize};
use shine_forge::map::{HexShard, HexTileLayer, MapShard, Tile};

#[derive(Serialize, Deserialize, Clone)]
pub struct GroundTile {
    tile: u8,
}

impl Default for GroundTile {
    fn default() -> Self {
        Self::empty()
    }
}

impl GroundTile {
    pub fn empty() -> Self {
        Self { tile: 0 }
    }
}

impl Tile for GroundTile {}

pub type GroundShard = HexShard<GroundTile>;
pub type GroundLayer = <GroundShard as MapShard>::Primary;
pub type GroundAudit = <GroundShard as MapShard>::Audit;
//pub type GroundOverlay = <GroundShard as MapShard>::Overlay;
pub type GroundConfig = <GroundShard as MapShard>::Config;

#[derive(Component)]
pub struct GroundRender;

/// System to synchronize ground tiles with the rendering system.
pub fn sync_ground_tiles(
    mut ground_layer_q: Query<(Entity, &GroundLayer, &mut GroundAudit)>,
    mut ground_render_tiles_q: MapRenderTileQuery<GroundShard>,
    mut commands: Commands,
) {
    for (ground_entity, ground_layer, mut ground_audit) in ground_layer_q.iter_mut() {
        if ground_audit.has_none() {
            continue;
        }

        if ground_render_tiles_q.select_chunk_by_layer(ground_entity) {
            for updated in ground_audit.ones() {
                let tile = ground_layer.get(updated);

                if let Some(entity) = ground_render_tiles_q.find_tile_render(updated) {
                    log::info!("Updating existing ground tile render entity at {:?}", updated);
                } else {
                    log::info!("Creating new ground tile render entity at {:?}", updated);
                }
            }
            ground_audit.reset_all();
        }
    }
}
