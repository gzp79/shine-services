use crate::repositories::command_log::{
    pg::{pg_command_log_db::PgCommandLogDb, stream, PgCommandLogError},
    StoredCommand, StreamHead,
};
use postgres_types::ToSql;
use shine_game::replica::StreamId;
use shine_infra::{
    db::postgres::{PgValueTypeUnknown, ToPgType},
    pg_query,
    sync::ExponentialBackoff,
};
use std::time::Duration;
use tokio_postgres::IsolationLevel;
use uuid::Uuid;

/// Bounds of an append transaction: a dead or stuck holder must not keep the stream's row lock.
const APPEND_TIMEOUTS: &str = "SET LOCAL lock_timeout = '5s'; SET LOCAL idle_in_transaction_session_timeout = '30s'";

const MAX_ATTEMPTS: u32 = 8;

#[derive(Debug, Clone, ToSql)]
#[postgres(name = "cl_command_input")]
struct CommandInput<'a> {
    version: i64,
    command_id: Uuid,
    inbox_id: Option<&'a str>,
    user_id: Uuid,
    data: &'a str,
    command_hash: i64,
    chain_hash: i64,
}

impl ToPgType for CommandInput<'_> {
    type PgValueType = PgValueTypeUnknown;
}

impl<'a> From<&'a StoredCommand> for CommandInput<'a> {
    fn from(command: &'a StoredCommand) -> Self {
        Self {
            version: command.version,
            command_id: command.command_id,
            inbox_id: command.inbox_id.as_deref(),
            user_id: command.user_id,
            data: command.data.as_str(),
            command_hash: command.command_hash as i64,
            chain_hash: command.chain_hash as i64,
        }
    }
}

// One round trip for the whole batch: the array of commands is unnested into rows instead of one
// execute() per command.
pg_query!( pub InsertCommands =>
    in = kind: &str, id: &str, commands: &[CommandInput<'_>];
    sql = r#"
        INSERT INTO cl_commands (kind, id, version, command_id, inbox_id, user_id, data, command_hash, chain_hash)
        SELECT $1, $2, (cmd).* FROM UNNEST($3::cl_command_input[]) AS cmd
        ON CONFLICT (kind, id, version) DO NOTHING
    "#
);

pg_query!( pub CasHeadVersion =>
    in = kind: &str, id: &str, expected_version: i64, expected_chain_hash: i64, new_version: i64, new_chain_hash: i64;
    sql = r#"
        UPDATE cl_streams SET version = $5, chain_hash = $6
        WHERE kind = $1 AND id = $2 AND version = $3 AND chain_hash = $4
    "#
);

pub async fn append(
    db: &PgCommandLogDb,
    stream_id: &StreamId,
    commands: &[StoredCommand],
    baseline_hash: u64,
) -> Result<StreamHead, PgCommandLogError> {
    debug_assert!(
        commands.windows(2).all(|w| w[1].version == w[0].version + 1),
        "commands must be a dense, contiguous run of versions"
    );
    let stmts = db.stmts();
    let kind = stream_id.kind().as_str();
    let id = stream_id.id();

    let Some(first) = commands.first() else {
        return stream::head(db, stream_id)
            .await?
            .ok_or(PgCommandLogError::StreamNotFound);
    };
    let first_version = first.version;
    let last_version = commands.last().map_or(first_version, |c| c.version);

    let mut backoff = ExponentialBackoff::new(Duration::from_micros(100), Duration::from_millis(100));
    for attempt in 1..=MAX_ATTEMPTS {
        let mut client = db.client().await?;

        // Read and validate outside any transaction: a verify-only (or rejected) append never
        // needs to write at all, and the write below only needs the row for as long as its own
        // statements take.
        let head: StreamHead = stream::get_head(db, &client, stream_id)
            .await?
            .ok_or(PgCommandLogError::StreamNotFound)?;

        if head.version < first_version - 1 {
            return Err(PgCommandLogError::LogDiverged { head });
        }
        // The chain hash I can claim at head.version, if it falls within what I hold. When the head
        // is already ahead of my own tip, I have nothing of mine to compare: the caller sees it from
        // the returned head being beyond the batch.
        let my_hash_at_head = if head.version == first_version - 1 {
            Some(baseline_hash)
        } else if head.version <= last_version {
            Some(commands[(head.version - first_version) as usize].chain_hash)
        } else {
            None
        };
        if let Some(expected) = my_hash_at_head {
            if expected != head.chain_hash {
                return Err(PgCommandLogError::LogDiverged { head });
            }
        }

        // `commands` is a dense, contiguous run from `first_version` (asserted above), so the
        // rows above `head.version` are always a suffix, not an arbitrary subset.
        let new_start = ((head.version - first_version + 1) as usize).min(commands.len());
        let new = &commands[new_start..];
        let Some(new_first) = new.first() else {
            return Ok(head);
        };
        if new_first.version != head.version + 1 || new.windows(2).any(|w| w[1].version != w[0].version + 1) {
            return Err(PgCommandLogError::LogDiverged { head });
        }

        let tx = client.transaction(Some(IsolationLevel::ReadCommitted)).await?;
        tx.batch_execute(APPEND_TIMEOUTS).await?;

        // Idempotent: a concurrent attempt inserting the same rows at the same versions is harmless
        // (`ON CONFLICT DO NOTHING`); only advancing the head below needs a conflict check. No
        // calculation here: every value is already decided above, outside the transaction. One
        // batch insert, not one round trip per command.
        let commands: Vec<CommandInput<'_>> = new.iter().map(CommandInput::from).collect();
        stmts
            .insert_commands
            .execute(&tx, kind, id, commands.as_slice())
            .await?;

        let new_version = new.last().map_or(head.version, |c| c.version);
        let new_chain_hash = new.last().map_or(head.chain_hash, |c| c.chain_hash);
        let moved = stmts
            .cas_head_version
            .execute(
                &tx,
                kind,
                id,
                head.version,
                head.chain_hash as i64,
                new_version,
                new_chain_hash as i64,
            )
            .await?;

        if moved == 1 {
            tx.commit().await?;
            return Ok(StreamHead {
                version: new_version,
                chain_hash: new_chain_hash,
            });
        }
        tx.rollback().await?;
        if attempt == MAX_ATTEMPTS {
            return Err(PgCommandLogError::Contention);
        }
        backoff.delay().await;
    }
    unreachable!()
}
