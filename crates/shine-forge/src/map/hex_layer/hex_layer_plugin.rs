use crate::map::{map_plugin::MapAppExt, HexDenseLayer, HexLayerConfig, HexSparseLayer, Tile};
use bevy::app::{App, Plugin};

/// Register a new dense hexagonal map layer.
pub struct HexDenseLayerPlugin<T>
where
    T: Tile,
{
    config: HexLayerConfig<T>,
    with_sync: bool,
}

impl<T> HexDenseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: HexLayerConfig<T>) -> Self {
        Self { config, with_sync: false }
    }

    pub fn with_sync(mut self, with_sync: bool) -> Self {
        self.with_sync = with_sync;
        self
    }
}

impl<T> Plugin for HexDenseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        app.add_map_layer::<HexDenseLayer<T>, _>(self.config.clone());
        if self.with_sync {
            app.add_map_sync_event_processing::<HexDenseLayer<T>>();
        }
    }
}

/// Register a new sparse hexagonal map layer.
pub struct HexSparseLayerPlugin<T>
where
    T: Tile,
{
    config: HexLayerConfig<T>,
    with_sync: bool,
}

impl<T> HexSparseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: HexLayerConfig<T>) -> Self {
        Self { config, with_sync: false }
    }

    pub fn with_sync(mut self, with_sync: bool) -> Self {
        self.with_sync = with_sync;
        self
    }
}

impl<T> Plugin for HexSparseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        app.add_map_layer::<HexSparseLayer<T>, _>(self.config.clone());
        if self.with_sync {
            app.add_map_sync_event_processing::<HexSparseLayer<T>>();
        }
    }
}
