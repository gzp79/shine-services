use crate::{
    repositories::{command_inbox::CommandInboxDb, command_log::CommandLogDb},
    services::command_stream::{
        CommandStreamConfig, CommandStreamService, RegistryError, StreamEgress, StreamRegistry, WorkerConfig,
    },
};
use shine_game::replica::{ChatState, PixelMapState};

/// A registry that serves the kinds of stream.
pub fn standard_registry<I, L, E>(
    inbox_db: I,
    log_db: L,
    egress: &E,
    service_config: CommandStreamConfig,
    worker_config: WorkerConfig,
) -> Result<StreamRegistry<I, L>, RegistryError>
where
    I: CommandInboxDb + 'static,
    L: CommandLogDb + 'static,
    E: StreamEgress + Clone + 'static,
{
    let service = || CommandStreamService::new(inbox_db.clone(), log_db.clone(), service_config.clone());
    StreamRegistry::builder(inbox_db.clone(), log_db.clone())
        .register::<ChatState, _>(service(), egress.clone(), worker_config.clone())
        .register::<PixelMapState, _>(service(), egress.clone(), worker_config)
        .build()
}
