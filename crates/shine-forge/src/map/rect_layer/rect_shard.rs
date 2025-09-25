use crate::map::{
    MapAppExt, MapShard, MapShardSystemConfig, RectBitsetLayer, RectDenseLayer, RectLayerConfig, RectSparseLayer, Tile,
};
use bevy::app::{App, Plugin};

pub struct RectShard<T>
where
    T: Tile,
{
    system_config: MapShardSystemConfig<Self>,
    layer_config: RectLayerConfig<T>,
}

impl<T> RectShard<T>
where
    T: Tile,
{
    pub fn server(width: u32, height: u32) -> Self {
        Self {
            system_config: MapShardSystemConfig::server(),
            layer_config: RectLayerConfig::new(width, height),
        }
    }

    pub fn client(width: u32, height: u32) -> Self {
        Self {
            system_config: MapShardSystemConfig::client(),
            layer_config: RectLayerConfig::new(width, height),
        }
    }
}

impl<T> MapShard for RectShard<T>
where
    T: Tile,
{
    type Tile = T;
    type Config = RectLayerConfig<T>;

    type Primary = RectDenseLayer<T>;
    type Overlay = RectSparseLayer<T>;
    type Audit = RectBitsetLayer<T>;
}

impl<T> Plugin for RectShard<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        app.add_map_shard(self.system_config.clone(), self.layer_config.clone());
    }
}
