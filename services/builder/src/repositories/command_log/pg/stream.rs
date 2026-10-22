use crate::{
    models::CommandLogError,
    repositories::command_log::{
        pg::pg_command_log_db::{db_error, PgCommandLogDb},
        StreamHead, StreamId,
    },
};
use postgres_from_row::FromRow;
use shine_infra::{
    db::postgres::{PgConnection, PgRawConnection},
    pg_query,
};
use tokio_postgres::Row;

// `chain_hash` is stored as Postgres `BIGINT` (`i64`); the cast to `u64` happens here, at the boundary.
impl FromRow for StreamHead {
    fn from_row(row: &Row) -> Self {
        Self::try_from_row(row).expect("missing column in row")
    }

    fn try_from_row(row: &Row) -> Result<Self, tokio_postgres::Error> {
        Ok(Self {
            kind: row.try_get("kind")?,
            version: row.try_get("version")?,
            chain_hash: row.try_get::<_, i64>("chain_hash")? as u64,
        })
    }
}

pg_query!( pub EnsureStream =>
    in = stream_id: &str, kind: &str;
    out = StreamHead;
    sql = r#"
        INSERT INTO cl_streams (stream_id, kind) VALUES ($1, $2)
        ON CONFLICT (stream_id) DO UPDATE SET stream_id = cl_streams.stream_id
        RETURNING kind, version, chain_hash
    "#
);

pg_query!( pub GetHead =>
    in = stream_id: &str;
    out = StreamHead;
    sql = "SELECT kind, version, chain_hash FROM cl_streams WHERE stream_id = $1"
);

pub async fn ensure_stream(
    db: &PgCommandLogDb,
    stream_id: &StreamId,
    kind: &str,
) -> Result<StreamHead, CommandLogError> {
    let client = db.client().await?;
    let head: StreamHead = db
        .stmts()
        .ensure_stream
        .query_one(&client, stream_id.as_str(), kind)
        .await
        .map_err(db_error)?;
    if head.kind != kind {
        return Err(CommandLogError::KindMismatch {
            stored: head.kind,
            requested: kind.to_string(),
        });
    }
    Ok(head)
}

pub async fn head(db: &PgCommandLogDb, stream_id: &StreamId) -> Result<Option<StreamHead>, CommandLogError> {
    let client = db.client().await?;
    get_head(db, &client, stream_id.as_str()).await
}

/// Reads the head on a connection the caller already holds, e.g. to validate before opening a transaction.
pub async fn get_head<T>(
    db: &PgCommandLogDb,
    client: &PgConnection<T>,
    stream_id: &str,
) -> Result<Option<StreamHead>, CommandLogError>
where
    T: PgRawConnection,
{
    db.stmts().get_head.query_opt(client, stream_id).await.map_err(db_error)
}
