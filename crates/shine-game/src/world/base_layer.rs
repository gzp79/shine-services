use crate::{
    math::quadrangulation::Rot4Idx,
    world::{CellIndex, Layer, LayerBuilder, LayerUpdate, Tile, TileIndex},
};

/// Base terrain layer.
pub type BaseLayer = Layer<u32>;

/// Layer-kind marker selecting a chunk's base layer.
pub struct Base;

/// Sets a quadrant of a tile in the base layer.
pub struct SetQuadrant {
    pub tile: TileIndex,
    pub quadrant: Rot4Idx,
    pub value: u8,
}

impl LayerUpdate for SetQuadrant {
    type Kind = Base;

    fn update(&self, builder: &mut LayerBuilder<'_, u32>) {
        builder.set_quadrant(self.tile, self.quadrant, self.value);
    }
}

/// Sets a cell's value on the matching quadrant of every tile surrounding it (within a single chunk).
pub struct SetCell {
    pub cell: CellIndex,
    pub value: u8,
}

impl LayerUpdate for SetCell {
    type Kind = Base;

    fn update(&self, builder: &mut LayerBuilder<'_, u32>) {
        builder.set_cell(self.cell, self.value);
    }
}

/// Sets every quadrant of every tile in the base layer to `value`.
pub struct Clear {
    pub value: u8,
}

impl LayerUpdate for Clear {
    type Kind = Base;

    fn update(&self, builder: &mut LayerBuilder<'_, u32>) {
        builder.fill(u32::from_quadrants([self.value; 4]));
    }
}
