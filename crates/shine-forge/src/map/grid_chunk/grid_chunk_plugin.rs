use crate::map::{
    client, create_layer_system, process_layer_commands_system, process_map_event_system, remove_layer_system,
    remove_rejected_chunks_system, ChunkCommandQueue, ChunkEvent, ChunkHasher, ChunkLayer, ChunkOperation, GridChunk,
    GridConfig, LayerSetup, NullHasher,
};
use bevy::{
    app::{App, PostUpdate, PreUpdate, Update},
    ecs::schedule::IntoScheduleConfigs,
};

pub struct ChunkLayerSetup<C, O, H = NullHasher, EH = client::NullChunkEventService>
where
    C: GridChunk,
    O: ChunkOperation<C>,
    H: ChunkHasher<C>,
    EH: client::SendChunkEventService<C>,
{
    hasher: Option<H>,
    client_send_service: Option<EH>,
    command_queue: ChunkCommandQueue<C, O, H>,
}

impl<C, O, H, EH> ChunkLayerSetup<C, O, H, EH>
where
    C: GridChunk,
    O: ChunkOperation<C>,
    H: ChunkHasher<C>,
    EH: client::SendChunkEventService<C>,
{
    pub fn new(command_queue: ChunkCommandQueue<C, O, H>) -> Self {
        Self {
            hasher: None,
            client_send_service: None,
            command_queue,
        }
    }

    /// Start tracking the chunk hashes for each update operation using the given hasher.
    pub fn with_hash_tracker(mut self, hasher: H) -> Self {
        self.hasher = Some(hasher);
        self
    }

    pub fn with_client_send_service(mut self, client_send_service: EH) -> Self {
        self.client_send_service = Some(client_send_service);
        self
    }
}

impl<CFG, C, O, H, EH> LayerSetup<CFG> for ChunkLayerSetup<C, O, H, EH>
where
    CFG: GridConfig,
    C: GridChunk + From<CFG>,
    O: ChunkOperation<C>,
    H: ChunkHasher<C>,
    EH: client::SendChunkEventService<C>,
{
    fn build(&self, app: &mut App) {
        log::debug!("Adding map layer: {}", C::name());
        if let Some(hasher) = &self.hasher {
            app.insert_resource(hasher.clone());
        }
        app.insert_resource(self.command_queue.clone());
        app.insert_resource(ChunkLayer::<C>::new());

        app.add_event::<ChunkEvent<C>>();

        app.add_systems(
            PreUpdate,
            (create_layer_system::<C, H>, remove_layer_system::<C>)
                .chain()
                .after(process_map_event_system),
        );

        app.add_systems(Update, process_layer_commands_system::<CFG, C, O, H>);

        app.add_systems(PostUpdate, remove_rejected_chunks_system::<C>);

        if let Some(client_send_service) = &self.client_send_service {
            app.insert_resource(client_send_service.clone());
            app.insert_resource(client::PendingChunkTasks::<C>::new());
            app.add_systems(PostUpdate, client::process_chunk_events_system::<C, EH>);
        }
    }
}
