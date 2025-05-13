use crate::map::ChunkStore;
use bevy::ecs::component::Component;
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::BTreeMap, marker::PhantomData};

pub trait ChunkHasher: 'static + Clone + Send + Sync {
    type Chunk: ChunkStore;
    type Hash: Clone + Send + Sync + Serialize + DeserializeOwned;

    fn hash(&self, chunk: &Self::Chunk) -> Self::Hash;
}

///  A default no-hash implementation that returns an empty hash.
pub struct NullHasher<C>
where
    C: ChunkStore,
{
    ph: PhantomData<C>,
}

impl<C> Default for NullHasher<C>
where
    C: ChunkStore,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C> Clone for NullHasher<C>
where
    C: ChunkStore,
{
    fn clone(&self) -> Self {
        Self::new()
    }
}

impl<C> NullHasher<C>
where
    C: ChunkStore,
{
    pub fn new() -> Self {
        Self { ph: PhantomData }
    }
}

impl<C> ChunkHasher for NullHasher<C>
where
    C: ChunkStore,
{
    type Chunk = C;
    type Hash = ();

    fn hash(&self, _chunk: &Self::Chunk) -> Self::Hash {
        ()
    }
}

/// Store the chunk hash for a range of versions.
#[derive(Component)]
pub struct ChunkHashTrack<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    hash: BTreeMap<usize, H::Hash>,
    ph: PhantomData<C>,
}

impl<C, H> ChunkHashTrack<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    pub fn new() -> Self {
        Self {
            hash: BTreeMap::new(),
            ph: PhantomData,
        }
    }

    pub fn clear(&mut self) {
        self.hash.clear();
    }

    pub fn set(&mut self, version: usize, hash: H::Hash) {
        log::debug!(
            "ChunkHashTrack: set version {} hash {}",
            version,
            serde_json::to_string(&hash).unwrap()
        );
        self.hash.insert(version, hash);
    }

    pub fn get(&self, version: usize) -> Option<&H::Hash> {
        self.hash.get(&version)
    }
}
