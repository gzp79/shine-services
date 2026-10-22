use crate::{
    models::CommandLogError,
    repositories::command_log::{
        pg::pg_command_log_db::{db_error, PgCommandLogDb},
        StoredCommand, StreamId,
    },
};
use postgres_from_row::FromRow;
use shine_infra::pg_query;
use tokio_postgres::Row;

// `command_hash`/`chain_hash` are stored as Postgres `BIGINT` (`i64`); the cast to `u64` happens
// here, at the boundary.
impl FromRow for StoredCommand {
    fn from_row(row: &Row) -> Self {
        Self::try_from_row(row).expect("missing column in row")
    }

    fn try_from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            version: row.try_get("version")?,
            command_id: row.try_get("command_id")?,
            user_id: row.try_get("user_id")?,
            data: row.try_get("data")?,
            command_hash: row.try_get::<_, i64>("command_hash")? as u64,
            chain_hash: row.try_get::<_, i64>("chain_hash")? as u64,
        })
    }
}

pg_query!( pub CommandsAfter =>
    in = stream_id: &str, after: i64, limit: i64;
    out = StoredCommand;
    sql = r#"
        SELECT version, command_id, user_id, data, command_hash, chain_hash FROM cl_commands
        WHERE stream_id = $1 AND version > $2
        ORDER BY version
        LIMIT $3
    "#
);

pg_query!( pub LastCommands =>
    in = stream_id: &str, count: i64;
    out = StoredCommand;
    sql = r#"
        SELECT * FROM (
            SELECT version, command_id, user_id, data, command_hash, chain_hash FROM cl_commands
            WHERE stream_id = $1
            ORDER BY version DESC
            LIMIT $2
        ) tail ORDER BY version
    "#
);

pub async fn commands_after(
    db: &PgCommandLogDb,
    stream_id: &StreamId,
    after: i64,
    limit: i64,
) -> Result<Vec<StoredCommand>, CommandLogError> {
    let client = db.client().await?;
    db.stmts()
        .commands_after
        .query(&client, stream_id.as_str(), after, limit)
        .await
        .map_err(db_error)
}

pub async fn last_commands(
    db: &PgCommandLogDb,
    stream_id: &StreamId,
    count: i64,
) -> Result<Vec<StoredCommand>, CommandLogError> {
    let client = db.client().await?;
    db.stmts()
        .last_commands
        .query(&client, stream_id.as_str(), count)
        .await
        .map_err(db_error)
}
