use crate::map::MapChunkId;
use bevy::ecs::message::Message;

/// Message to request chunk loading and unloading
#[derive(Message, Debug)]
pub enum MapMessage {
    Load(MapChunkId),
    Unload(MapChunkId),
}
