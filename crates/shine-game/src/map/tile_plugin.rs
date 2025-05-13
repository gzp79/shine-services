use crate::map::{
    create_layer_system, process_layer_commands_system, process_map_event_system, remove_layer_system,
    ChunkCommandQueue, ChunkHasher, ChunkLayer, ChunkStore, NullHasher, TileMap, TileMapConfig, TileMapEvent,
};
use bevy::{
    app::{App, Plugin, PreUpdate, Update},
    ecs::schedule::IntoScheduleConfigs,
};

/// Trait to setup a layer of the tile-map
pub trait LayerSetup: 'static + Send + Sync {
    fn build(&self, app: &mut App);
}

impl LayerSetup for () {
    fn build(&self, _app: &mut App) {}
}

/// Trait to setup a chain of layers of the tile-map and simplify the TileMapPlugin type
pub trait TileMapPluginLayer: LayerSetup {
    fn add_layer<L>(self, layer: L) -> impl TileMapPluginLayer
    where
        L: LayerSetup;
}

pub struct TileMapLayerSetup<B = (), N = ()>
where
    B: LayerSetup,
    N: LayerSetup,
{
    builder: B,
    next: N,
}

impl<B> TileMapLayerSetup<B, ()>
where
    B: LayerSetup,
{
    pub fn new(builder: B) -> Self {
        Self { builder, next: () }
    }
}

impl<B, N> LayerSetup for TileMapLayerSetup<B, N>
where
    B: LayerSetup,
    N: LayerSetup,
{
    fn build(&self, app: &mut App) {
        self.builder.build(app);
        self.next.build(app);
    }
}

impl<B, N> TileMapPluginLayer for TileMapLayerSetup<B, N>
where
    B: LayerSetup,
    N: LayerSetup,
{
    fn add_layer<C>(self, layer: C) -> impl TileMapPluginLayer
    where
        C: LayerSetup,
    {
        TileMapLayerSetup {
            builder: self.builder,
            next: TileMapLayerSetup::new(layer),
        }
    }
}

pub struct ChunkLayerSetup<C, H = NullHasher<C>>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    hasher: Option<H>,
    command_queue: ChunkCommandQueue<C, H>,
}

impl<C, H> ChunkLayerSetup<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    pub fn new(command_queue: ChunkCommandQueue<C, H>) -> Self {
        Self { hasher: None, command_queue }
    }

    /// Start tracking the chunk hashes for each update operation using the given hasher.
    pub fn with_hash_tracker(mut self, hasher: H) -> Self {
        self.hasher = Some(hasher);
        self
    }
}

impl<C, H> LayerSetup for ChunkLayerSetup<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    fn build(&self, app: &mut App) {
        log::debug!("Adding map layer: {}", C::NAME);
        app.insert_resource(ChunkLayer::<C, H>::new(self.hasher.clone(), self.command_queue.clone()));

        app.add_systems(
            PreUpdate,
            (create_layer_system::<C, H>, remove_layer_system::<C, H>)
                .chain()
                .after(process_map_event_system),
        );

        app.add_systems(Update, process_layer_commands_system::<C, H>);
    }
}

pub struct TileMapPlugin<L = TileMapLayerSetup>
where
    L: TileMapPluginLayer,
{
    config: TileMapConfig,
    layers: L,
}

impl TileMapPlugin<TileMapLayerSetup> {
    pub fn new(config: TileMapConfig) -> Self {
        Self {
            config,
            layers: TileMapLayerSetup::new(()),
        }
    }
}

impl<L> TileMapPlugin<L>
where
    L: TileMapPluginLayer,
{
    pub fn with_layer<C>(self, layer: C) -> TileMapPlugin<impl TileMapPluginLayer>
    where
        C: LayerSetup,
    {
        TileMapPlugin {
            config: self.config,
            layers: self.layers.add_layer(layer),
        }
    }
}

impl<L> Plugin for TileMapPlugin<L>
where
    L: TileMapPluginLayer,
{
    fn build(&self, app: &mut App) {
        app.insert_resource(TileMap::new(self.config.clone()));
        app.add_event::<TileMapEvent>();

        self.layers.build(app);

        app.add_systems(PreUpdate, process_map_event_system);
    }
}
