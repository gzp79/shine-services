//! State-agnostic, ordered command log per stream, with snapshots.

mod command_log_types;
mod pg_command_log;
mod snapshot_codec;
mod stream_id;

pub use self::{
    command_log_types::{
        AppendOutcome, CommandLogError, PruneOutcome, SnapshotOutcome, StoredCommand, StoredSnapshot, StreamHead,
    },
    pg_command_log::PgCommandLogDb,
    snapshot_codec::{decode_snapshot, encode_snapshot, SnapshotCodecError},
    stream_id::{StreamId, StreamIdError},
};
