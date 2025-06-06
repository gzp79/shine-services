use crate::map::{grid::GridChunkTypes, hex::HexChunkTypes};

pub struct HeightMapTypes;

impl GridChunkTypes for HeightMapTypes {
    type Tile = f32;

    fn name() -> &'static str {
        "HeightMap"
    }
}

impl HexChunkTypes for HeightMapTypes {
    type Tile = f32;

    fn name() -> &'static str {
        "HeightMap"
    }
}
