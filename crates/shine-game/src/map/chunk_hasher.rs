use crate::map::MapChunk;
use bevy::ecs::{component::Component, resource::Resource};
use serde::{de::DeserializeOwned, Serialize};
use std::{collections::BTreeMap, marker::PhantomData};

/// Hash the content of a chunk to fast compare for changes.
pub trait ChunkHasher<C>: Resource + Clone
where
    C: MapChunk,
{
    type Hash: PartialEq + Clone + Serialize + DeserializeOwned + Send + Sync + 'static;

    fn hash(&self, chunk: &C) -> Self::Hash;
}

/// A default no-hash implementation that returns an empty hash when hashing is not needed.
#[derive(Resource, Clone, Default)]
pub struct NullHasher;

impl<C> ChunkHasher<C> for NullHasher
where
    C: MapChunk,
{
    type Hash = ();

    fn hash(&self, _chunk: &C) -> Self::Hash {}
}

/// Track chunk hashes for a range of versions.
#[derive(Component)]
pub struct ChunkHashTrack<C, H>
where
    C: MapChunk,
    H: ChunkHasher<C>,
{
    hash: BTreeMap<usize, H::Hash>,
    ph: PhantomData<C>,
}

impl<C, H> Default for ChunkHashTrack<C, H>
where
    C: MapChunk,
    H: ChunkHasher<C>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C, H> ChunkHashTrack<C, H>
where
    C: MapChunk,
    H: ChunkHasher<C>,
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
