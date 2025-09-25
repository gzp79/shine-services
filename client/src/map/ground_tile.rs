use serde::{Deserialize, Serialize};
use shine_forge::map::{HexShard, MapShard, Tile};

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
pub type GroundConfig = <GroundShard as MapShard>::Config;
