use crate::map::{build_map_layer, HexDenseLayer, HexLayerConfig, HexSparseLayer, Tile};
use bevy::app::{App, Plugin};

/// Register a new dense hexagonal map layer with the given tile type.
pub struct HexDenseLayerPlugin<T>
where
    T: Tile,
{
    config: HexLayerConfig<T>,
}

impl<T> HexDenseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: HexLayerConfig<T>) -> Self {
        Self { config }
    }
}

impl<T> Plugin for HexDenseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        build_map_layer::<HexDenseLayer<T>, _>(self.config.clone(), app);
    }
}

/// Register a new sparse hexagonal map layer with the given tile type.
pub struct HexSparseLayerPlugin<T>
where
    T: Tile,
{
    config: HexLayerConfig<T>,
}

impl<T> HexSparseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: HexLayerConfig<T>) -> Self {
        Self { config }
    }
}

impl<T> Plugin for HexSparseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        build_map_layer::<HexSparseLayer<T>, _>(self.config.clone(), app);
    }
}
