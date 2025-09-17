use crate::map::{map_plugin::build_map_layer, RectDenseLayer, RectLayerConfig, RectSparseLayer, Tile};
use bevy::app::{App, Plugin};

/// Register a new dense rectangular map layer.
pub struct RectDenseLayerPlugin<T>
where
    T: Tile,
{
    config: RectLayerConfig<T>,
}

impl<T> RectDenseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: RectLayerConfig<T>) -> Self {
        Self { config }
    }
}

impl<T> Plugin for RectDenseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        build_map_layer::<RectDenseLayer<T>, _>(self.config.clone(), app);
    }
}

/// Register a new sparse rectangular map layer.
pub struct RectSparseLayerPlugin<T>
where
    T: Tile,
{
    config: RectLayerConfig<T>,
}

impl<T> RectSparseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: RectLayerConfig<T>) -> Self {
        Self { config }
    }
}

impl<T> Plugin for RectSparseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        build_map_layer::<RectSparseLayer<T>, _>(self.config.clone(), app);
    }
}
