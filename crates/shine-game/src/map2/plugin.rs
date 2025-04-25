use crate::map2::{complete_chunk_load_system, start_chunk_load_system, ChunkFactory, TileMap, TileMapConfig};
use bevy::{
    app::{App, Plugin, Update},
    ecs::schedule::IntoScheduleConfigs,
};
use std::sync::Arc;

use super::process_commands_system;

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
        app.add_systems(Update, start_chunk_load_system::<C>);
        app.add_systems(
            Update,
            complete_chunk_load_system::<C>.after(start_chunk_load_system::<C>),
        );
        app.add_systems(
            Update,
            process_commands_system::<C>.after(complete_chunk_load_system::<C>),
        );
    }
}
