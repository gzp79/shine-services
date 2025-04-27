use crate::map2::{ChunkId, TileMap, TileMapConfig};
use bevy::ecs::{
    event::{Event, EventReader},
    system::{Commands, ResMut},
};
use std::marker::PhantomData;

/// Event to request chunk loading and unloading
#[derive(Event)]
pub enum TileMapEvent<C>
where
    C: TileMapConfig,
{
    Load(ChunkId),
    Unload(ChunkId),

    __Ph(PhantomData<C>),
}

pub fn process_map_events<C>(
    mut tile_map: ResMut<TileMap<C>>,
    mut ev: EventReader<TileMapEvent<C>>,
    mut commands: Commands,
) where
    C: TileMapConfig,
{
    for event in ev.read() {
        match event {
            TileMapEvent::Load(chunk_id) => {
                tile_map.load_chunk(*chunk_id, &mut commands);
            }
            TileMapEvent::Unload(chunk_id) => {
                tile_map.unload_chunk(*chunk_id, &mut commands);
            }
            TileMapEvent::__Ph(_) => {}
        }
    }
}
