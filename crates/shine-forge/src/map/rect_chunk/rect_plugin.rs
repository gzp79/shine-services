use crate::map::{build_map_layer, MapLayer, RectConfig, RectDense, RectSparse, Tile};
use bevy::app::{App, Plugin};

#[allow(type_alias_bounds)]
pub type MapRectDenseLayer<T: Tile> = MapLayer<RectDense<T>>;

/// Register a new dense rectangular map layer with the given tile type.
pub struct MapRectDenseLayerPlugin<T>
where
    T: Tile,
{
    config: RectConfig<T>,
}

impl<T> MapRectDenseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: RectConfig<T>) -> Self {
        Self { config }
    }
}

impl<T> Plugin for MapRectDenseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        build_map_layer::<RectDense<T>, _>(self.config.clone(), app);
    }
}

#[allow(type_alias_bounds)]
pub type MapRectSparseLayer<T: Tile> = MapLayer<RectSparse<T>>;

/// Register a new sparse rectangular map layer with the given tile type.
pub struct MapRectSparseLayerPlugin<T>
where
    T: Tile,
{
    config: RectConfig<T>,
}

impl<T> MapRectSparseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: RectConfig<T>) -> Self {
        Self { config }
    }
}

impl<T> Plugin for MapRectSparseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        build_map_layer::<RectSparse<T>, _>(self.config.clone(), app);
    }
}
