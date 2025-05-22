use crate::map::{ChunkId, MapChunkTracker};
use bevy::ecs::{
    event::{Event, EventReader},
    system::{Commands, ResMut},
};

/// Event to request chunk loading and unloading
#[derive(Event, Debug)]
pub enum MapEvent {
    Load(ChunkId),
    Unload(ChunkId),
}

/// Process TileMapEvent and perform chunk spawn/despawn
pub fn process_map_event_system(
    mut tile_map: ResMut<MapChunkTracker>,
    mut ev: EventReader<MapEvent>,
    mut commands: Commands,
) {
    for event in ev.read() {
        log::debug!("Processing TileMapEvent: {:?}", event);
        match event {
            MapEvent::Load(chunk_id) => {
                tile_map.load_chunk(*chunk_id, &mut commands);
            }
            MapEvent::Unload(chunk_id) => {
                tile_map.unload_chunk(*chunk_id, &mut commands);
            }
        }
    }
}
