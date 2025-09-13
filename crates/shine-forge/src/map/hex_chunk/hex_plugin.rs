use crate::map::{HexConfig, MapPlugin, Tile};
use bevy::app::{App, Plugin};

pub struct MapHexDenseLayerPlugin<T>
where
    T: Tile,
{
    config: HexConfig<T>,
}

impl<T> MapHexDenseLayerPlugin<T>
where
    T: Tile,
{
    pub fn new(config: HexConfig<T>) -> Self {
        Self { config }
    }
}

impl<T> Plugin for MapHexDenseLayerPlugin<T>
where
    T: Tile,
{
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<MapPlugin>() {
            app.add_plugins(MapPlugin::default());
        }

        app.insert_resource(self.config.clone());
        //app.add_resource(MapL);
    }
}
