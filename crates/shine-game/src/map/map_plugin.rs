use crate::map::{process_map_event_system, MapChunkTracker, MapConfig, MapEvent};
use bevy::app::{App, Plugin, PreUpdate};

/// Trait to setup a single layer of the map
pub trait LayerSetup: Send + Sync + 'static {
    fn build(&self, app: &mut App);
}

impl LayerSetup for () {
    fn build(&self, _app: &mut App) {}
}

/// Trait to setup a chain of layers of the map
#[doc(hidden)]
pub trait LayerSetupChain: LayerSetup {
    fn add_layer<L>(self, layer: L) -> impl LayerSetupChain
    where
        L: LayerSetup;
}

/// The setup chain for the map layers
pub struct MapLayersPlugin<B = (), N = ()>
where
    B: LayerSetup,
    N: LayerSetup,
{
    builder: B,
    next: N,
}

impl<B> MapLayersPlugin<B, ()>
where
    B: LayerSetup,
{
    pub fn new(builder: B) -> Self {
        Self { builder, next: () }
    }
}

impl<B, N> LayerSetup for MapLayersPlugin<B, N>
where
    B: LayerSetup,
    N: LayerSetup,
{
    fn build(&self, app: &mut App) {
        self.builder.build(app);
        self.next.build(app);
    }
}

impl<B, N> LayerSetupChain for MapLayersPlugin<B, N>
where
    B: LayerSetup,
    N: LayerSetup,
{
    fn add_layer<C>(self, layer: C) -> impl LayerSetupChain
    where
        C: LayerSetup,
    {
        MapLayersPlugin {
            builder: self.builder,
            next: MapLayersPlugin::new(layer),
        }
    }
}

pub struct MapPlugin<L = MapLayersPlugin>
where
    L: LayerSetupChain,
{
    config: MapConfig,
    layers: L,
}

impl MapPlugin<MapLayersPlugin> {
    pub fn new(config: MapConfig) -> Self {
        Self {
            config,
            layers: MapLayersPlugin::new(()),
        }
    }
}

impl<L> MapPlugin<L>
where
    L: LayerSetupChain,
{
    pub fn with_layer<C>(self, layer: C) -> MapPlugin<impl LayerSetupChain>
    where
        C: LayerSetup,
    {
        MapPlugin {
            config: self.config,
            layers: self.layers.add_layer(layer),
        }
    }
}

impl<L> Plugin for MapPlugin<L>
where
    L: LayerSetupChain,
{
    fn build(&self, app: &mut App) {
        app.insert_resource(self.config.clone());
        app.insert_resource(MapChunkTracker::new());
        app.add_event::<MapEvent>();

        self.layers.build(app);

        app.add_systems(PreUpdate, process_map_event_system);
    }
}
