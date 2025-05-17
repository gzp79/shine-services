use crate::map::{ChunkEvent, ChunkId, MapChunk};
use bevy::{
    ecs::{
        event::EventReader,
        resource::Resource,
        system::{Res, ResMut},
    },
    tasks::{block_on, futures_lite::future, AsyncComputeTaskPool, Task},
};
use std::{
    collections::{hash_map::Entry, HashMap},
    future::Future,
    marker::PhantomData,
    mem,
};

/// The sender part of async client-server channels to send the bevy events to the server.
/// The receiver part is client specific and uses the ChunkCommandQueue to inject the response events into the bevy world.
pub trait SendChunkEventService<C>: Resource + Clone
where
    C: MapChunk,
{
    fn on_track(&self, chunk_id: ChunkId) -> impl Future<Output = ()> + Send + '_;
    fn on_untrack(&self, chunk_id: ChunkId) -> impl Future<Output = ()> + Send + '_;
    fn on_missing_events(&self, chunk_id: ChunkId, first: usize, last: usize) -> impl Future<Output = ()> + Send + '_;
}

#[derive(Resource, Clone)]
pub struct NullChunkEventService;

impl<C> SendChunkEventService<C> for NullChunkEventService
where
    C: MapChunk,
{
    async fn on_track(&self, chunk_id: ChunkId) {
        log::debug!("NullChunkEventService: on_track({:?})", chunk_id);
    }

    async fn on_untrack(&self, chunk_id: ChunkId) {
        log::debug!("NullChunkEventService: on_untrack({:?})", chunk_id);
    }

    async fn on_missing_events(&self, chunk_id: ChunkId, first: usize, last: usize) {
        log::debug!(
            "NullChunkEventService: on_missing_events({:?}, {}, {})",
            chunk_id,
            first,
            last
        );
    }
}

/// Store the async task for each chunk. Each chunk process a single task at a time.
#[derive(Resource)]
pub struct PendingChunkTasks<C>
where
    C: MapChunk,
{
    tasks: HashMap<ChunkId, Task<()>>,
    ph: PhantomData<C>,
}

impl<C> Default for PendingChunkTasks<C>
where
    C: MapChunk,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<C> PendingChunkTasks<C>
where
    C: MapChunk,
{
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            ph: PhantomData,
        }
    }
}

/// Create a new layer component for the chunk when a new chunk-entity is spawned.
/// The chunk is created as empty and hence it is only a placeholder. The chunk is marked completed(loaded) when
/// a ChunkCommand::Data or ChunkCommand::Empty is received.
#[allow(clippy::type_complexity)]
pub fn process_chunk_events_system<C, EH>(
    mut tasks: ResMut<PendingChunkTasks<C>>,
    handler: Res<EH>,
    mut events: EventReader<ChunkEvent<C>>,
) where
    C: MapChunk,
    EH: SendChunkEventService<C>,
{
    let thread_pool = AsyncComputeTaskPool::get();

    for event in events.read() {
        let chunk_id = match event {
            ChunkEvent::Track { id } => *id,
            ChunkEvent::Untrack { id } => *id,
            ChunkEvent::OperationGap { id, .. } => *id,
            ChunkEvent::_Phantom(_) => unreachable!(),
        };

        // check if the task has finished
        let ready = match tasks.tasks.entry(chunk_id) {
            Entry::Vacant(_) => true,
            Entry::Occupied(mut entry) => {
                if block_on(future::poll_once(entry.get_mut())).is_some() {
                    log::debug!("Chunk [{:?}]: Task finished", chunk_id);
                    // it's finished, we can safely drop the task
                    mem::drop(entry.remove());
                    true
                } else {
                    false
                }
            }
        };

        // if no task is running, spawn a new task
        if ready {
            match event {
                ChunkEvent::Track { id } => {
                    log::debug!("Chunk [{:?}]: Start track request", id);
                    let task = {
                        let handler = handler.clone();
                        let chunk_id = *id;
                        thread_pool.spawn(async move { handler.on_track(chunk_id).await })
                    };
                    tasks.tasks.insert(*id, task);
                }
                ChunkEvent::Untrack { id } => {
                    log::debug!("Chunk [{:?}]: Start untrack request", id);
                    let task = {
                        let handler = handler.clone();
                        let chunk_id = *id;
                        thread_pool.spawn(async move { handler.on_untrack(chunk_id).await })
                    };
                    tasks.tasks.insert(*id, task);
                }
                ChunkEvent::OperationGap { id, first, last } => {
                    log::debug!(
                        "Chunk [{:?}]: Start get missing operation request ({},{})",
                        id,
                        first,
                        last
                    );
                    let task = {
                        let handler = handler.clone();
                        let chunk_id = *id;
                        let first = *first;
                        let last = *last;
                        thread_pool.spawn(async move { handler.on_missing_events(chunk_id, first, last).await })
                    };
                    tasks.tasks.insert(*id, task);
                }
                _ => {}
            }
        }
    }
}
