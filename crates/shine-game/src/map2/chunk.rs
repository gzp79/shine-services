use crate::map2::{ChunkOperation, ChunkStore};
use bevy::ecs::component::Component;
use std::{
    fmt::Debug,
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChunkId(pub usize, pub usize);

/// Scope for the chunk indicating the usage
pub trait Scope: 'static + Send + Sync + Default + Debug {}

/// Scope for the chunks components
pub mod scopes {
    use super::Scope;

    /// Scope for the data authenticated by the server
    #[derive(Default, Debug)]
    pub struct Persisted;
    impl Scope for Persisted {}

    /// Scope for the temporary, local data
    #[derive(Default, Debug)]
    pub struct Local;
    impl Scope for Local {}
}

/// A chunk operation and the version of the chunk it was created for
pub struct ChunkCommand<O>
where
    O: ChunkOperation,
{
    pub operation: O,
    pub version: usize,
}

impl<O> ChunkCommand<O>
where
    O: ChunkOperation,
{
    pub fn new(operation: O, version: usize) -> Self {
        Self { operation, version }
    }
}

/// Component to store the current version of a chunk.
#[derive(Component)]
pub struct ChunkVersion<S> {
    pub version: usize,
    pub phantom: PhantomData<S>,
}

impl<S> ChunkVersion<S>
where
    S: Scope,
{
    pub fn new(version: usize) -> Self {
        Self {
            version,
            phantom: PhantomData,
        }
    }
}

impl<S> Deref for ChunkVersion<S>
where
    S: Scope,
{
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.version
    }
}

impl<S> DerefMut for ChunkVersion<S>
where
    S: Scope,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.version
    }
}

/// Component to store updates commands for a chunk
#[derive(Component)]
pub struct ChunkUpdates<S, O>
where
    S: Scope,
    O: ChunkOperation,
{
    pub commands: Vec<ChunkCommand<O>>,
    pub phantom: PhantomData<S>,
}

impl<S, O> ChunkUpdates<S, O>
where
    S: Scope,
    O: ChunkOperation,
{
    pub fn new(commands: Vec<ChunkCommand<O>>) -> Self {
        Self {
            commands,
            phantom: PhantomData,
        }
    }
}

impl<S, O> Deref for ChunkUpdates<S, O>
where
    S: Scope,
    O: ChunkOperation,
{
    type Target = Vec<ChunkCommand<O>>;

    fn deref(&self) -> &Self::Target {
        &self.commands
    }
}

impl<S, O> DerefMut for ChunkUpdates<S, O>
where
    S: Scope,
    O: ChunkOperation,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.commands
    }
}

#[derive(Component)]
pub struct Chunk<S, C>
where
    S: Scope,
    C: ChunkStore,
{
    pub store: C,
    pub phantom: PhantomData<S>,
}

impl<S, C> Chunk<S, C>
where
    S: Scope,
    C: ChunkStore,
{
    pub fn new(store: C) -> Self {
        Self {
            store,
            phantom: PhantomData,
        }
    }

    pub fn new_empty(size: (usize, usize)) -> Self {
        Self {
            store: C::new(size.0, size.1),
            phantom: PhantomData,
        }
    }

    pub fn scope(&self) -> S {
        S::default()
    }
}

impl<S, C> Deref for Chunk<S, C>
where
    S: Scope,
    C: ChunkStore,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl<S, C> DerefMut for Chunk<S, C>
where
    S: Scope,
    C: ChunkStore,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.store
    }
}
