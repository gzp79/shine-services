use bevy::ecs::{
    component::{Component, Mutable},
    resource::Resource,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// Unique identifier for a chunk in the world
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkId(pub usize, pub usize);

/// Mark the root of a chunk in the ECS
/// Layers of a chunk are either stored as a component or as a child of the root entity.
#[derive(Component)]
pub struct ChunkRoot {
    pub id: ChunkId,
}

/// Store the version of a layer of the chunk.
#[derive(Component)]
pub struct ChunkVersion<C>
where
    C: MapChunk,
{
    pub version: usize,
    ph: PhantomData<C>,
}

impl<C> Default for ChunkVersion<C>
where
    C: MapChunk,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C> ChunkVersion<C>
where
    C: MapChunk,
{
    pub fn new() -> Self {
        Self { version: 0, ph: PhantomData }
    }

    pub fn with_version(version: usize) -> Self {
        Self { version, ph: PhantomData }
    }
}

impl<C> Deref for ChunkVersion<C>
where
    C: MapChunk,
{
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.version
    }
}

impl<C> DerefMut for ChunkVersion<C>
where
    C: MapChunk,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.version
    }
}

/// Update operation for a chunk stored in the event-stream.
pub trait ChunkOperation<C>: Serialize + DeserializeOwned + Send + Sync + 'static {
    fn check_precondition(&self, chunk: &C) -> bool;
    fn apply(self, chunk: &mut C);
}

/// Configuration for the map that applies to all chunks
pub trait MapConfig: Resource + Clone {}

/// Trait to define a layer of a chunk in the map.
pub trait MapChunk: Component<Mutability = Mutable> {
    /// Name of the layer.
    fn name() -> &'static str;

    /// Create an empty chunk component indicating that the chunk is not loaded yet.
    fn new_empty() -> Self
    where
        Self: Sized;

    /*/// Create a new chunk default component with the given configuration.
    fn new(config: &CFG) -> Self
    where
        Self: Sized;*/

    /// Return if the chunk is empty (on-loaded) or not.
    fn is_empty(&self) -> bool;

    fn hash(&self) -> u64 {
        0 // Default implementation, can be overridden
    }
}
