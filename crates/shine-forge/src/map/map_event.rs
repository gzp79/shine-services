use crate::map::MapChunkId;
use bevy::ecs::event::Event;

/// Event to request chunk loading and unloading
#[derive(Event, Debug)]
pub enum MapEvent {
    Load(MapChunkId),
    Unload(MapChunkId),
}
