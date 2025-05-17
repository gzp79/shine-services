use crate::map::{ChunkId, MapChunk};
use bevy::ecs::event::Event;
use std::marker::PhantomData;

#[derive(Event, Debug)]
pub enum ChunkEvent<C>
where
    C: MapChunk,
{
    /// Chunk layer is created, start tracking data changes
    Track { id: ChunkId },

    /// Chunk was unloaded, stop tracking data changes
    Untrack { id: ChunkId },

    /// Some operations are missing from the stream
    /// The first,last is an inclusive range of the missing operations
    OperationGap { id: ChunkId, first: usize, last: usize },

    #[doc(hidden)]
    _Phantom(PhantomData<C>),
}
