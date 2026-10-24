use crate::{
    repositories::{command_inbox::CommandInboxDb, command_log::CommandLogDb},
    services::command_stream::{
        CommandStreamConfig, CommandStreamService, RegistryError, StreamEgress, StreamRegistry, WorkerConfig,
    },
};
use shine_game::replica::{ChatState, PixelMapState};

/// A registry that serves the kinds of stream of this service: the chat room and the pixel map. Every kind
/// gets its own service, so a kind can have a configuration of its own.
pub fn standard_registry<L, I, E>(
    log: L,
    inbox: I,
    egress: E,
    service_config: CommandStreamConfig,
    worker_config: WorkerConfig,
) -> Result<StreamRegistry<L, I>, RegistryError>
where
    L: CommandLogDb + 'static,
    I: CommandInboxDb + 'static,
    E: StreamEgress,
{
    let service = || CommandStreamService::new(log.clone(), inbox.clone(), service_config.clone());
    StreamRegistry::builder(log.clone(), inbox.clone())
        .register::<ChatState, _>(service(), egress.clone(), worker_config.clone())
        .register::<PixelMapState, _>(service(), egress, worker_config)
        .build()
}
