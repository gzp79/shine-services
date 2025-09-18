use crate::map::{map_plugin::MapAppExt, RectDenseLayer, RectLayerConfig, RectSparseLayer, Tile};
use bevy::app::{App, Plugin};

/// Register a new dense rectangular map layer.
pub struct RectDenseLayerPlugin<T>
where
    T: Tile,
{
    config: RectLayerConfig<T>,
    with_spawn_initialized: bool,
    with_sync: bool,
}

impl<T> RectDenseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: RectLayerConfig<T>) -> Self {
        Self {
            config,
            with_spawn_initialized: false,
            with_sync: false,
        }
    }

    pub fn with_spawn_initialized(mut self, with_spawn_initialized: bool) -> Self {
        self.with_spawn_initialized = with_spawn_initialized;
        self
    }

    pub fn with_sync(mut self, with_sync: bool) -> Self {
        self.with_sync = with_sync;
        self
    }
}

impl<T> Plugin for RectDenseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        app.add_map_layer::<RectDenseLayer<T>>(self.config.clone(), self.with_spawn_initialized);
        if self.with_sync {
            app.add_map_sync_event_processing::<RectDenseLayer<T>>();
        }
    }
}

/// Register a new sparse rectangular map layer.
pub struct RectSparseLayerPlugin<T>
where
    T: Tile,
{
    config: RectLayerConfig<T>,
    with_spawn_initialized: bool,
    with_sync: bool,
}

impl<T> RectSparseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: RectLayerConfig<T>) -> Self {
        Self {
            config,
            with_spawn_initialized: false,
            with_sync: false,
        }
    }
    pub fn with_spawn_initialized(mut self, with_spawn_initialized: bool) -> Self {
        self.with_spawn_initialized = with_spawn_initialized;
        self
    }

    pub fn with_sync(mut self, with_sync: bool) -> Self {
        self.with_sync = with_sync;
        self
    }
}

impl<T> Plugin for RectSparseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        app.add_map_layer::<RectSparseLayer<T>>(self.config.clone(), self.with_spawn_initialized);
        if self.with_sync {
            app.add_map_sync_event_processing::<RectSparseLayer<T>>();
        }
    }
}
