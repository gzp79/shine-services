use crate::map::{ChunkId, ChunkStore, Tile};
use bevy::platform::sync::{Arc, Mutex};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    collections::{hash_map::Entry, BTreeMap, HashMap},
    mem,
};

use super::ChunkHasher;

pub trait ChunkOperation: 'static + Serialize + DeserializeOwned + Send + Sync {
    type Tile: Tile;

    fn apply<C>(self, chunk: &mut C)
    where
        C: ChunkStore<Tile = Self::Tile>;
}

pub enum ChunkCommand<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    /// Indicates a missing or deleted chunk. Chunk data is reset to an empty state and any previous operations are discarded.
    Empty,
    /// Indicates a new chunk snapshot. Any operations older than the snapshot has been already applied.
    Data(C),
    /// A list of operations to be applied to the chunk in the order of the versions.
    Operations(Vec<(usize, C::Operation)>),
    /// A list of hashes corresponding to versions to detect drifts compared to the authoritative snapshot.    
    DriftDetect(Vec<(usize, H::Hash)>),
}

pub enum ChunkDataUpdate<C> {
    Empty,
    Data(C),
    None,
}

/// Pre-processed ChunkCommand thus system can use it directly.
/// - Chunk will be initialized either with an empty or a snapshot data
/// - Store only the latest data (snapshot) and discard any older data
/// - Operations older than the data are discarded
/// - Operations are sorted by version
pub struct ChunkUpdate<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    data: ChunkDataUpdate<C>,
    operations: BTreeMap<usize, C::Operation>,
    drift_detect: BTreeMap<usize, H::Hash>,
}

impl<C, H> Default for ChunkUpdate<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C, H> ChunkUpdate<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    pub fn new() -> Self {
        Self {
            data: ChunkDataUpdate::None,
            operations: BTreeMap::new(),
            drift_detect: BTreeMap::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        matches!(self.data, ChunkDataUpdate::None) && self.operations.is_empty()
    }

    /// No snapshot or data is stored.
    pub fn add_empty_data(&mut self) {
        if matches!(self.data, ChunkDataUpdate::None) {
            self.data = ChunkDataUpdate::Empty;
        }
    }

    /// Store a new snapshot and discard any older operations and data.
    pub fn add_data(&mut self, data: C) {
        let version = data.version();
        let data_version = match &self.data {
            ChunkDataUpdate::Data(data) => data.version(),
            ChunkDataUpdate::Empty => 0,
            ChunkDataUpdate::None => 0,
        };

        if version >= data_version {
            log::trace!(
                "Storing new data at version ({}), replacing version ({})",
                version,
                data_version
            );
            self.data = ChunkDataUpdate::Data(data);
            self.operations.retain(|v, _| *v > version);
        } else {
            log::trace!(
                "Data is older ({}) than the current data version ({}), ignoring",
                version,
                data_version
            );
        }
    }

    pub fn add_operation(&mut self, version: usize, operation: C::Operation) {
        let data_version = match &self.data {
            ChunkDataUpdate::Data(data) => data.version(),
            ChunkDataUpdate::Empty => 0,
            ChunkDataUpdate::None => 0,
        };

        if data_version < version {
            log::trace!(
                "Storing new operation at version ({}) for data version ({})",
                version,
                data_version
            );
            self.operations.entry(version).or_insert(operation);
        } else {
            log::trace!(
                "Operation ({}) is older than the current data version ({}), ignoring",
                version,
                data_version
            );
        }
    }

    pub fn add_hash(&mut self, version: usize, hash: H::Hash) {
        let data_version = match &self.data {
            ChunkDataUpdate::Data(data) => data.version(),
            ChunkDataUpdate::Empty => 0,
            ChunkDataUpdate::None => 0,
        };

        if data_version < version {
            log::trace!(
                "Storing new hash at version ({}) for data version ({})",
                version,
                data_version
            );
            self.drift_detect.entry(version).or_insert(hash);
        } else {
            log::trace!(
                "Hash ({}) is older than the current data version ({}), ignoring",
                version,
                data_version
            );
        }
    }

    pub fn into_parts(
        self,
    ) -> (
        ChunkDataUpdate<C>,
        BTreeMap<usize, C::Operation>,
        BTreeMap<usize, H::Hash>,
    ) {
        (self.data, self.operations, self.drift_detect)
    }
}

/// Store chunk updates grouped by chunk id.
/// This is a thread-safe structure that can be used to connect bevy and other systems.
/// TODO: due to the single lock it could be a bottleneck in the future.
pub struct ChunkCommandQueue<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    updates: Arc<Mutex<HashMap<ChunkId, ChunkUpdate<C, H>>>>,
}

impl<C, H> Clone for ChunkCommandQueue<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    fn clone(&self) -> Self {
        Self { updates: self.updates.clone() }
    }
}

impl<C, H> Default for ChunkCommandQueue<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C, H> ChunkCommandQueue<C, H>
where
    C: ChunkStore,
    H: ChunkHasher<Chunk = C>,
{
    pub fn new() -> Self {
        Self {
            updates: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn store_command(&self, chunk_id: ChunkId, command: ChunkCommand<C, H>) {
        let mut queues = self.updates.lock().unwrap();
        let update = queues.entry(chunk_id).or_default();
        match command {
            ChunkCommand::Empty => update.add_empty_data(),
            ChunkCommand::Data(data) => update.add_data(data),
            ChunkCommand::Operations(operations) => {
                for (version, operation) in operations {
                    update.add_operation(version, operation);
                }
            }
            ChunkCommand::DriftDetect(drift_detect) => {
                for (version, hash) in drift_detect {
                    update.add_hash(version, hash);
                }
            }
        }
    }

    pub fn store_operations<I>(&self, chunk_id: ChunkId, operations: I)
    where
        I: IntoIterator<Item = (usize, C::Operation)>,
    {
        let mut queues = self.updates.lock().unwrap();
        let update = queues.entry(chunk_id).or_default();
        for (version, operation) in operations {
            update.add_operation(version, operation);
        }
    }

    pub fn take_update(&self, chunk_id: ChunkId) -> ChunkUpdate<C, H> {
        let mut queues = self.updates.lock().unwrap();
        if let Entry::Occupied(mut entry) = queues.entry(chunk_id) {
            mem::take(entry.get_mut())
        } else {
            ChunkUpdate::new()
        }
    }
}
