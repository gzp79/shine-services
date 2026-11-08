use crate::{
    models::CommandStreamError,
    repositories::{command_inbox::CommandInboxDb, command_log::CommandLogDb},
    services::command_stream::{StreamConfig, StreamEgress, StreamLogService, StreamRegistry, StreamsConfig},
};
use shine_game::replica::{ChatState, PixelMapState};

/// A registry that serves the kinds of stream, each with its own configuration.
pub fn standard_registry<I, L, E>(
    inbox_db: I,
    log_db: L,
    egress: &E,
    config: StreamsConfig,
) -> Result<StreamRegistry<I, L>, CommandStreamError>
where
    I: CommandInboxDb + 'static,
    L: CommandLogDb + 'static,
    E: StreamEgress + Clone + 'static,
{
    let service = |config: &StreamConfig| StreamLogService::new(inbox_db.clone(), log_db.clone(), config.clone());
    StreamRegistry::builder(inbox_db.clone(), log_db.clone())
        .register::<ChatState, _>(service(&config.chat), egress.clone(), config.chat)
        .register::<PixelMapState, _>(service(&config.pixel_map), egress.clone(), config.pixel_map)
        .build()
}
