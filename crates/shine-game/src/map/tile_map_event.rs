use crate::map::{ChunkId, TileMap};
use bevy::ecs::{
    event::{Event, EventReader},
    system::{Commands, ResMut},
};

/// Event to request chunk loading and unloading
#[derive(Event, Debug)]
pub enum TileMapEvent {
    Load(ChunkId),
    Unload(ChunkId),
}

/// Process TileMapEvent and perform chunk spawn/despawn
pub fn process_map_event_system(
    mut tile_map: ResMut<TileMap>,
    mut ev: EventReader<TileMapEvent>,
    mut commands: Commands,
) {
    for event in ev.read() {
        log::debug!("Processing TileMapEvent: {:?}", event);
        match event {
            TileMapEvent::Load(chunk_id) => {
                tile_map.load_chunk(*chunk_id, &mut commands);
            }
            TileMapEvent::Unload(chunk_id) => {
                tile_map.unload_chunk(*chunk_id, &mut commands);
            }
        }
    }
}
