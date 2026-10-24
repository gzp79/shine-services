// Not called by anything outside this module's own tests yet: the stream worker that polls the
// inbox is a later round.
#![allow(dead_code)]

mod command_inbox_db;
mod command_inbox_types;

pub mod redis;

pub use self::{
    command_inbox_db::CommandInboxDb,
    command_inbox_types::{InboxBatch, InboxEntry, InboxId, InboxWake},
};
pub use shine_game::replica::StreamId;

#[cfg(test)]
pub mod tests;
