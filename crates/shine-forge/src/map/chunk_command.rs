use crate::map::{ChunkEvent, ChunkHashTrack, ChunkHasher, ChunkId, ChunkOperation, ChunkRoot, MapChunk, MapConfig};
use bevy::{
    ecs::{
        event::EventWriter,
        resource::Resource,
        system::{Local, Query, Res},
    },
    platform::sync::{Arc, Mutex},
};
use std::collections::{BTreeMap, VecDeque};

use super::ChunkVersion;

/// Command to be applied to a chunk.
pub enum ChunkCommand<C, O, H>
where
    C: MapChunk,
    O: ChunkOperation<C>,
    H: ChunkHasher<C>,
{
    /// Indicates a missing or deleted chunk. Chunk data is reset to an empty state and any previous operations are discarded.
    Empty,
    /// Indicates a new chunk snapshot. Any operations older than the snapshot has been already applied.
    Data((usize, C)),
    /// A list of operations to be applied to the chunk in the order of the versions.
    Operations(Vec<(usize, O)>),
    /// A list of hashes corresponding to versions to detect drifts compared to the authoritative snapshot.    
    DriftDetect(Vec<(usize, H::Hash)>),
    /// The chunk is not available for the user.
    Rejected,
}

pub type CommandVec<C, O, H> = Vec<(ChunkId, Option<ChunkCommand<C, O, H>>)>;

/// A queue of commands to be applied to chunks in receiving order. It is used to inject updates outside of
/// the bevy world.
#[derive(Resource)]
pub struct ChunkCommandQueue<C, O, H>
where
    C: MapChunk,
    O: ChunkOperation<C>,
    H: ChunkHasher<C>,
{
    queue: Arc<Mutex<CommandVec<C, O, H>>>,
}

impl<C, O, H> Default for ChunkCommandQueue<C, O, H>
where
    C: MapChunk,
    O: ChunkOperation<C>,
    H: ChunkHasher<C>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C, O, H> Clone for ChunkCommandQueue<C, O, H>
where
    C: MapChunk,
    O: ChunkOperation<C>,
    H: ChunkHasher<C>,
{
    fn clone(&self) -> Self {
        Self { queue: self.queue.clone() }
    }
}

impl<C, O, H> ChunkCommandQueue<C, O, H>
where
    C: MapChunk,
    O: ChunkOperation<C>,
    H: ChunkHasher<C>,
{
    /// Creates a new empty command queue.
    pub fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Adds a command to the queue.
    pub fn add_command(&self, chunk_id: ChunkId, command: ChunkCommand<C, O, H>) {
        let mut queue = self.queue.lock().unwrap();
        queue.push((chunk_id, Some(command)));
    }

    /// Take commands for a specific chunk ID from the queue.
    pub fn take_commands(&self, chunk_id: ChunkId, commands: &mut VecDeque<ChunkCommand<C, O, H>>) {
        let mut queue = self.queue.lock().unwrap();
        queue.retain_mut(|(id, command)| {
            if *id == chunk_id {
                commands.push_back(command.take().expect("Command should be present"));
                false
            } else {
                true
            }
        });
    }
}

/// Consume the ChunkCommand queue and integrate the commands into the chunk data.
#[allow(clippy::type_complexity)]
pub fn process_layer_commands_system<CFG, C, O, H>(
    map_config: Res<CFG>,
    hasher: Option<Res<H>>,
    chunk_command_queue: Res<ChunkCommandQueue<C, O, H>>,
    mut chunks: Query<(
        &ChunkRoot,
        &mut ChunkVersion<C>,
        &mut C,
        Option<&mut ChunkHashTrack<C, H>>,
    )>,
    mut events: EventWriter<ChunkEvent<C>>,

    // some optimization to avoid continuous memory allocation
    mut chunk_commands: Local<VecDeque<ChunkCommand<C, O, H>>>,
    mut chunk_operations: Local<BTreeMap<usize, O>>,
    mut chunk_hashes: Local<BTreeMap<usize, H::Hash>>,
) where
    CFG: MapConfig,
    C: MapChunk + From<CFG>,
    O: ChunkOperation<C>,
    H: ChunkHasher<C>,
{
    for (chunk_root, mut chunk_version, mut chunk, mut hash_track) in chunks.iter_mut() {
        let chunk_id = chunk_root.id;

        assert!(chunk_commands.is_empty());
        assert!(chunk_operations.is_empty());
        chunk_command_queue.take_commands(chunk_id, &mut chunk_commands);

        while let Some(command) = chunk_commands.pop_front() {
            let is_chunk_data = match command {
                ChunkCommand::Empty => {
                    log::debug!("Chunk [{:?}]: Reset to empty", chunk_root.id);
                    *chunk = C::from(map_config.clone());
                    true
                }
                ChunkCommand::Data((data_version, data)) => {
                    log::debug!(
                        "Chunk [{:?}]: Replace with a new data at version ({})",
                        chunk_root.id,
                        data_version
                    );
                    *chunk = data;
                    chunk_version.version = data_version;
                    true
                }
                ChunkCommand::Operations(operations) => {
                    chunk_operations.extend(operations);
                    false
                }
                ChunkCommand::DriftDetect(drift_detect) => {
                    if hash_track.is_some() {
                        chunk_hashes.extend(drift_detect);
                    }
                    false
                }
                ChunkCommand::Rejected => {
                    log::debug!("Chunk [{:?}]: Rejected", chunk_root.id);
                    events.write(ChunkEvent::TrackRejected { id: chunk_id });
                    false
                }
            };

            if is_chunk_data {
                // when a chunk is replaced, we clear any pending operations as they are declared obsolete
                chunk_commands.clear();
                chunk_hashes.clear();
                if let (Some(hasher), Some(hash_track)) = (&hasher, &mut hash_track) {
                    hash_track.clear();
                    hash_track.set(chunk_version.version, hasher.hash(&chunk));
                    log::debug!(
                        "Chunk [{:?}]: Hash cleared and stored [{}] -> [{}]",
                        chunk_id,
                        chunk_version.version,
                        serde_json::to_string(hash_track.get(chunk_version.version).unwrap()).unwrap()
                    );
                }
            }
        }

        // apply operations by version
        if !chunk_commands.is_empty() {
            log::debug!("Chunk [{:?}]: Applying {} operations", chunk_id, chunk_operations.len());
            while let Some((version, operation)) = chunk_operations.pop_first() {
                if version <= chunk_version.version {
                    log::trace!("Chunk [{:?}]: Operation is too old {}, ignoring", chunk_id, version);
                } else if version == chunk_version.version + 1 {
                    if operation.check_precondition(&chunk) {
                        operation.apply(&mut *chunk);
                    }
                    chunk_version.version = version;
                    if let (Some(hasher), Some(hash_track)) = (&hasher, &mut hash_track) {
                        hash_track.set(chunk_version.version, hasher.hash(&chunk));
                        log::debug!(
                            "Chunk [{:?}]: Hash stored [{}] -> [{}]",
                            chunk_id,
                            chunk_version.version,
                            serde_json::to_string(hash_track.get(chunk_version.version).unwrap()).unwrap()
                        );
                    }
                } else {
                    log::debug!(
                        "Chunk [{:?}]: Operation version gap detected: [{}..{})",
                        chunk_id,
                        chunk_version.version + 1,
                        version
                    );
                    events.write(ChunkEvent::OperationGap {
                        id: chunk_id,
                        first: chunk_version.version + 1,
                        last: version,
                    });
                    break;
                }
            }

            if !chunk_operations.is_empty() {
                log::debug!(
                    "Chunk [{:?}]: Storing {} for future operations",
                    chunk_id,
                    chunk_operations.len()
                );
                let command = {
                    let mut operations = Vec::with_capacity(chunk_operations.len());
                    while let Some(cmd) = chunk_operations.pop_first() {
                        operations.push(cmd);
                    }
                    ChunkCommand::Operations(operations)
                };
                chunk_command_queue.add_command(chunk_id, command);
            }
        }

        /*if !drift_detect.is_empty() {
            log::debug!(
                "Chunk [{:?}]: Applying {} drift detection hashes",
                chunk_id.id,
                drift_detect.len()
            );
            // todo: when drift detected, request a full reload
        }*/
    }
}
