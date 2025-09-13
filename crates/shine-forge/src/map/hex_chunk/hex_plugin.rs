use crate::map::{build_map_layer, HexConfig, HexDense, HexSparse, MapLayer, Tile};
use bevy::app::{App, Plugin};

#[allow(type_alias_bounds)]
pub type MapHexDenseLayer<T: Tile> = MapLayer<HexDense<T>>;

/// Register a new dense hexagonal map layer with the given tile type.
pub struct MapHexDenseLayerPlugin<T>
where
    T: Tile,
{
    config: HexConfig<T>,
}

impl<T> MapHexDenseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: HexConfig<T>) -> Self {
        Self { config }
    }
}

impl<T> Plugin for MapHexDenseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        build_map_layer::<HexDense<T>, _>(self.config.clone(), app);
    }
}

#[allow(type_alias_bounds)]
pub type MapHexSparseLayer<T: Tile> = MapLayer<HexSparse<T>>;

/// Register a new sparse hexagonal map layer with the given tile type.
pub struct MapHexSparseLayerPlugin<T>
where
    T: Tile,
{
    config: HexConfig<T>,
}

impl<T> MapHexSparseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: HexConfig<T>) -> Self {
        Self { config }
    }
}

impl<T> Plugin for MapHexSparseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        build_map_layer::<HexSparse<T>, _>(self.config.clone(), app);
    }
}
