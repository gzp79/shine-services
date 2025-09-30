use crate::world::{MapRenderTileQuery, MapTileRender, WorldConfig};
use bevy::{
    color::palettes::css,
    ecs::{
        bundle::Bundle,
        entity::Entity,
        hierarchy::ChildOf,
        name::Name,
        system::{Commands, Query, Res},
    },
    gizmos::gizmos::Gizmos,
    math::Vec3,
    transform::components::{GlobalTransform, Transform},
};
use serde::{Deserialize, Serialize};
use shine_forge::map::{AxialCoord, HexShard, HexTileLayer, MapShard, Tile};

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

    pub fn create_render_bundle(root: Entity, coord: AxialCoord, ground_tile_size: f32) -> impl Bundle {
        let local = coord.world_coordinate(ground_tile_size);
        let transform = Transform::from_translation(Vec3::new(local.x, local.y, 0.0));
        (
            Name::new(format!("GroundTile({},{})", coord.q, coord.r)),
            GroundRender::new(coord),
            transform,
            ChildOf(root),
        )
    }
}

impl Tile for GroundTile {}

pub type GroundShard = HexShard<GroundTile>;
pub type GroundLayer = <GroundShard as MapShard>::Primary;
pub type GroundAudit = <GroundShard as MapShard>::Audit;
//pub type GroundOverlay = <GroundShard as MapShard>::Overlay;
pub type GroundConfig = <GroundShard as MapShard>::Config;
pub type GroundRender = MapTileRender<GroundTile>;

/// System to synchronize ground tiles with the rendering system.
pub fn sync_ground_tiles(
    world_config: Res<WorldConfig>,
    mut ground_layer_q: Query<(Entity, &GroundLayer, &mut GroundAudit)>,
    mut ground_render_tiles_q: MapRenderTileQuery<GroundShard>,
    mut commands: Commands,
) {
    for (ground_entity, ground_layer, mut ground_audit) in ground_layer_q.iter_mut() {
        if ground_audit.has_none() {
            continue;
        }

        if let Some(render_root) = ground_render_tiles_q.select_chunk_by_layer(ground_entity) {
            for coord in ground_audit.ones() {
                let tile = ground_layer.get(coord);

                if let Some(entity) = ground_render_tiles_q.find_tile_render(coord) {
                    log::info!("Updating existing ground tile render entity at {:?}", coord);
                } else {
                    log::info!("Creating new ground tile render entity at {:?}", coord);
                    commands.spawn(GroundTile::create_render_bundle(
                        render_root,
                        coord,
                        world_config.ground_tile_size,
                    ));
                }
            }
            ground_audit.reset_all();
        }
    }
}

/// Draw debug gizmos for ground tiles.
pub fn debug_ground_tiles(
    world_config: Res<WorldConfig>,
    mut ground_layer_q: Query<(&GroundRender, &GlobalTransform)>,
    mut gizmos: Gizmos,
) {
    for (tile, transform) in ground_layer_q.iter() {
        gizmos.circle(transform.translation(), world_config.ground_tile_size, css::YELLOW);
    }
}
