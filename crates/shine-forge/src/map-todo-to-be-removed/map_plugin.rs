use crate::map::{process_map_event_system, MapChunkTracker, MapConfig, MapEvent};
use bevy::app::{App, Plugin, PreUpdate};
use std::marker::PhantomData;

/// Trait to setup a single layer of the map
pub trait LayerSetup<CFG>: Send + Sync + 'static
where
    CFG: MapConfig,
{
    fn build(&self, app: &mut App);
}

impl<CFG> LayerSetup<CFG> for ()
where
    CFG: MapConfig,
{
    fn build(&self, _app: &mut App) {}
}

/// Trait to setup a chain of layers of the map
#[doc(hidden)]
pub trait LayerSetupChain<CFG>: LayerSetup<CFG>
where
    CFG: MapConfig,
{
    fn add_layer<L>(self, layer: L) -> impl LayerSetupChain<CFG>
    where
        L: LayerSetup<CFG>;
}

/// The setup chain for the map layers
pub struct MapLayersPlugin<CFG, B = (), N = ()>
where
    CFG: MapConfig,
    B: LayerSetup<CFG>,
    N: LayerSetup<CFG>,
{
    builder: B,
    next: N,
    ph: PhantomData<CFG>,
}

impl<CFG, B> MapLayersPlugin<CFG, B, ()>
where
    CFG: MapConfig,
    B: LayerSetup<CFG>,
{
    pub fn new(builder: B) -> Self {
        Self {
            builder,
            next: (),
            ph: PhantomData,
        }
    }
}

impl<CFG, B, N> LayerSetup<CFG> for MapLayersPlugin<CFG, B, N>
where
    CFG: MapConfig,
    B: LayerSetup<CFG>,
    N: LayerSetup<CFG>,
{
    fn build(&self, app: &mut App) {
        self.builder.build(app);
        self.next.build(app);
    }
}

impl<CFG, B, N> LayerSetupChain<CFG> for MapLayersPlugin<CFG, B, N>
where
    CFG: MapConfig,
    B: LayerSetup<CFG>,
    N: LayerSetup<CFG>,
{
    fn add_layer<C>(self, layer: C) -> impl LayerSetupChain<CFG>
    where
        C: LayerSetup<CFG>,
    {
        MapLayersPlugin {
            builder: self.builder,
            next: MapLayersPlugin::new(layer),
            ph: PhantomData,
        }
    }
}

pub struct MapPlugin<CFG, L = MapLayersPlugin<CFG>>
where
    CFG: MapConfig,
    L: LayerSetupChain<CFG>,
{
    config: CFG,
    layers: L,
}

impl<CFG> MapPlugin<CFG, MapLayersPlugin<CFG>>
where
    CFG: MapConfig,
{
    pub fn new(config: CFG) -> Self {
        Self {
            config,
            layers: MapLayersPlugin::<CFG>::new(()),
        }
    }
}

impl<CFG, L> MapPlugin<CFG, L>
where
    CFG: MapConfig,
    L: LayerSetupChain<CFG>,
{
    pub fn with_layer<S>(self, layer: S) -> MapPlugin<CFG, impl LayerSetupChain<CFG>>
    where
        S: LayerSetup<CFG>,
    {
        MapPlugin {
            config: self.config,
            layers: self.layers.add_layer(layer),
        }
    }
}

impl<CFG, L> Plugin for MapPlugin<CFG, L>
where
    CFG: MapConfig,
    L: LayerSetupChain<CFG>,
{
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone());
        app.insert_resource(MapChunkTracker::new());
        app.add_event::<MapEvent>();

        self.layers.build(app);

        app.add_systems(PreUpdate, process_map_event_system);
    }
}
