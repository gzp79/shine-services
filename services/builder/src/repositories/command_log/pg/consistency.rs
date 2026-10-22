use crate::{
    models::CommandLogError,
    repositories::command_log::{
        pg::{
            commands,
            pg_command_log_db::{db_error, PgCommandLogDb},
            snapshot, stream,
        },
        ConsistencyIssue, ConsistencyReport, StoredCommand, StreamId,
    },
};
use postgres_from_row::FromRow;
use shine_infra::pg_query;

#[derive(FromRow)]
struct SnapshotRow {
    version: i64,
    chain_hash: i64,
}

pg_query!( pub ListSnapshots =>
    in = stream_id: &str;
    out = SnapshotRow;
    sql = "SELECT version, chain_hash FROM cl_snapshots WHERE stream_id = $1 ORDER BY version"
);

/// Independently re-verifies the stream from its stored rows, outside any transaction and
/// without blocking writers: whether the commands form one unbroken chain, every snapshot
/// agrees with the command at its version, and the head agrees with the last command.
///
/// The head is read first, and every row read after is bounded to that version: a concurrent
/// append is simply outside this pass rather than a false `HeadMismatch` or `SnapshotDrift`
/// from a torn read. Reads the whole log up to the head, so this is for tests and operational
/// checks, not the hot append/snapshot path.
pub async fn check_consistency(
    db: &PgCommandLogDb,
    stream_id: &StreamId,
) -> Result<ConsistencyReport, CommandLogError> {
    let Some(head) = stream::head(db, stream_id).await? else {
        return Ok(ConsistencyReport {
            checked_up_to_version: 0,
            issues: Vec::new(),
        });
    };

    let mut issues = Vec::new();

    // Ascending order: once a version exceeds the head captured above, every row after it
    // does too, so this is exactly "up to the head", no matter what was appended since.
    let commands: Vec<StoredCommand> = commands::commands_after(db, stream_id, 0, i64::MAX)
        .await?
        .into_iter()
        .take_while(|c| c.version <= head.version)
        .collect();

    let mut prev_version = 0i64;
    let mut prev_chain_hash = 0u64;
    for (i, command) in commands.iter().enumerate() {
        if i == 0 && command.version != 1 {
            // Pruning can remove every command below the oldest kept snapshot, so the oldest
            // surviving command legitimately starts above 1. That snapshot's version is the only
            // thing that can explain the jump; anything else is a genuine gap. Either way, there is
            // nothing earlier left to fold from, so this command's own chain_hash is trusted as the
            // baseline, the same way version 1 trusts a `chain_hash` of 0.
            if snapshot::oldest_snapshot_version(db, stream_id).await? != Some(command.version) {
                issues.push(ConsistencyIssue::Gap {
                    after: 0,
                    found: command.version,
                });
            }
        } else if command.version != prev_version + 1 {
            issues.push(ConsistencyIssue::Gap {
                after: prev_version,
                found: command.version,
            });
        } else {
            let expected = shine_game::replica::fold_chain_hash(prev_chain_hash, command.version, command.command_hash);
            if expected != command.chain_hash {
                issues.push(ConsistencyIssue::CommandChainBroken { version: command.version });
            }
        }
        prev_version = command.version;
        prev_chain_hash = command.chain_hash;
    }

    let client = db.client().await?;
    let snapshots = db
        .stmts()
        .list_snapshots
        .query(&client, stream_id.as_str())
        .await
        .map_err(db_error)?;
    for snapshot in snapshots {
        if snapshot.version > head.version {
            continue;
        }
        if let Ok(index) = commands.binary_search_by_key(&snapshot.version, |c| c.version) {
            if commands[index].chain_hash != snapshot.chain_hash as u64 {
                issues.push(ConsistencyIssue::SnapshotDrift { version: snapshot.version });
            }
        }
    }

    let last_chain_hash = commands.last().map_or(0, |c| c.chain_hash);
    if head.chain_hash != last_chain_hash {
        issues.push(ConsistencyIssue::HeadMismatch {
            head_chain_hash: head.chain_hash,
            last_command_chain_hash: last_chain_hash,
        });
    }

    Ok(ConsistencyReport {
        checked_up_to_version: head.version,
        issues,
    })
}
