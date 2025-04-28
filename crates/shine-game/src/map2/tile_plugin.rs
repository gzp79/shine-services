use crate::map2::{
    complete_chunk_load_system, process_commands_system, process_map_events, process_map_refresh,
    start_chunk_load_system, ChunkFactory, TileMap, TileMapConfig,
};
use bevy::{
    app::{App, Plugin, Startup, Update},
    ecs::schedule::IntoScheduleConfigs,
    platform::sync::Arc,
};

use super::{startup_map_refresh, TileMapEvent, TileMapRefresh};

pub struct TileMapPlugin<C>
where
    C: TileMapConfig,
{
    pub(crate) config: C,
    pub(crate) factory: Arc<dyn ChunkFactory<C>>,
}

impl<C> TileMapPlugin<C>
where
    C: TileMapConfig,
{
    pub fn new<F>(config: C, factory: F) -> Self
    where
        F: ChunkFactory<C> + 'static,
    {
        Self {
            config,
            factory: Arc::new(factory),
        }
    }
}

impl<C> Plugin for TileMapPlugin<C>
where
    C: TileMapConfig,
{
    fn build(&self, app: &mut App) {
        app.insert_resource(TileMap::new(self.config.clone(), self.factory.clone()));
        app.insert_resource(TileMapRefresh::<C>::new(Vec::new()));
        app.add_event::<TileMapEvent<C>>();
        app.add_systems(Startup, startup_map_refresh::<C>);
        app.add_systems(
            Update,
            (
                process_map_events::<C>,
                process_map_refresh::<C>,
                start_chunk_load_system::<C>,
                complete_chunk_load_system::<C>,
                process_commands_system::<C>,
            )
                .chain(),
        );
    }
}
