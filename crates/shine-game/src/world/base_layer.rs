use crate::{
    math::quadrangulation::Rot4Idx,
    world::{Layer, LayerUpdate, TileIndex},
};

/// Base terrain layer.
pub type BaseLayer = Layer<u32>;

/// Layer-kind marker selecting a chunk's base layer.
pub struct Base;

/// Sets a single cell of a tile in the base layer.
pub struct SetCell {
    pub tile: TileIndex,
    pub cell: Rot4Idx,
    pub value: u8,
}

impl LayerUpdate for SetCell {
    type Kind = Base;

    fn update(&self, layer: &mut BaseLayer) {
        layer.set_tile(self.tile, self.cell, self.value);
    }
}
