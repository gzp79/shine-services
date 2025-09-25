use crate::map::{
    HexBitsetLayer, HexDenseLayer, HexLayerConfig, HexSparseLayer, MapAppExt, MapShard, MapShardSystemConfig, Tile,
};
use bevy::app::{App, Plugin};

pub struct HexShard<T>
where
    T: Tile,
{
    system_config: MapShardSystemConfig<Self>,
    layer_config: HexLayerConfig<T>,
}

impl<T> HexShard<T>
where
    T: Tile,
{
    pub fn server(radius: u32) -> Self {
        Self {
            system_config: MapShardSystemConfig::server(),
            layer_config: HexLayerConfig::new(radius),
        }
    }

    pub fn client(radius: u32) -> Self {
        Self {
            system_config: MapShardSystemConfig::client(),
            layer_config: HexLayerConfig::new(radius),
        }
    }
}

impl<T> MapShard for HexShard<T>
where
    T: Tile,
{
    type Tile = T;
    type Config = HexLayerConfig<T>;

    type Primary = HexDenseLayer<T>;
    type Overlay = HexSparseLayer<T>;
    type Audit = HexBitsetLayer<T>;
}

impl<T> Plugin for HexShard<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        app.add_map_shard(self.system_config.clone(), self.layer_config.clone());
    }
}
