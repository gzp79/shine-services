use crate::map::MapConfig;
use bevy::ecs::component::{Component, Mutable};
use serde::{de::DeserializeOwned, Serialize};

/// Unique identifier for a chunk in the world
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkId(pub usize, pub usize);

/// Mark the root of a chunk in the ECS
/// Layers of a chunk are either stored as a component or as a child of the root entity.
#[derive(Component)]
pub struct ChunkRoot {
    pub id: ChunkId,
}

/// Update operation for a chunk stored in the event-stream.
pub trait ChunkOperation<C>: Serialize + DeserializeOwned + Send + Sync + 'static {
    fn apply(self, chunk: &mut C);
}

/// Trait to define a layer of a chunk in the map.
pub trait MapChunk: Component<Mutability = Mutable> {
    /// Name of the layer.
    fn name() -> &'static str;

    /// Create an empty chunk component indicating that the chunk is not loaded yet.
    fn new_empty() -> Self
    where
        Self: Sized;

    /// Create a new chunk default component with the given configuration.
    fn new(config: &MapConfig) -> Self
    where
        Self: Sized;

    /// Return if the chunk is empty (on-loaded) or not.
    fn is_empty(&self) -> bool;

    /// Get the current (event-stream) version of the chunk.
    fn version(&self) -> usize;
    /// Set the current (event-stream) version of the chunk.
    fn set_version(&mut self, version: usize);
}
