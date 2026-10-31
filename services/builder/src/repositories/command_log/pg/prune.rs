use crate::repositories::command_log::pg::{pg_command_log_db::PgCommandLogDb, PgCommandLogError};
use shine_game::replica::StreamId;
use shine_infra::pg_query;

pg_query!( pub Prune =>
    in = kind: &str, id: &str, kept_snapshot_count: i64;
    out = version: Option<i64>;
    sql = r#"
        WITH horizon AS (
            SELECT version FROM cl_snapshots WHERE kind = $1 AND id = $2
            ORDER BY version DESC OFFSET ($3 - 1) LIMIT 1
        ),
        old_snapshots AS (
            DELETE FROM cl_snapshots
            WHERE kind = $1 AND id = $2 AND version < (SELECT version FROM horizon)
            RETURNING 1
        ),
        old_commands AS (
            DELETE FROM cl_commands
            WHERE kind = $1 AND id = $2 AND version < (SELECT version FROM horizon)
            RETURNING 1
        )
        SELECT (SELECT version FROM horizon) AS version
    "#
);

/// Keeps the latest `kept_snapshot_count` snapshots and the commands from the oldest of them on,
/// and deletes the rest in one transaction. Nothing is deleted while the stream has fewer than
/// `kept_snapshot_count` snapshots. The horizon always comes from the kept snapshots, never from
/// the caller: a snapshot row is deleted with its command row, so a caller-chosen cutoff could
/// silently drop snapshots. Pruning never touches `cl_streams`, so the chain hash an append is
/// checked against is unaffected by what was deleted.
pub async fn prune(
    db: &PgCommandLogDb,
    stream_id: &StreamId,
    kept_snapshot_count: i64,
) -> Result<Option<i64>, PgCommandLogError> {
    let client = db.client().await?;
    db.stmts()
        .prune
        .query_one(
            &client,
            stream_id.kind().as_str(),
            stream_id.id(),
            kept_snapshot_count.max(1),
        )
        .await
        .map_err(PgCommandLogError::from)
}
