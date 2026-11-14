use crate::repositories::command_log::{
    pg::{pg_command_log_db::PgCommandLogDb, PgCommandLogError},
    StreamHead,
};
use postgres_from_row::FromRow;
use shine_game::replica::StreamId;
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
            version: row.try_get("version")?,
            chain_hash: row.try_get::<_, i64>("chain_hash")? as u64,
        })
    }
}

pg_query!( pub EnsureStream =>
    in = kind: &str, id: &str;
    out = StreamHead;
    sql = r#"
        INSERT INTO cl_streams (kind, id) VALUES ($1, $2)
        ON CONFLICT (kind, id) DO UPDATE SET id = cl_streams.id
        RETURNING version, chain_hash
    "#
);

pg_query!( pub GetHead =>
    in = kind: &str, id: &str;
    out = StreamHead;
    sql = "SELECT version, chain_hash FROM cl_streams WHERE kind = $1 AND id = $2"
);

pub async fn ensure_stream(db: &PgCommandLogDb, stream_id: &StreamId) -> Result<StreamHead, PgCommandLogError> {
    let client = db.client().await?;
    let head: StreamHead = db
        .stmts()
        .ensure_stream
        .query_one(&client, stream_id.kind().as_str(), stream_id.id())
        .await?;
    Ok(head)
}

pub async fn head(db: &PgCommandLogDb, stream_id: &StreamId) -> Result<Option<StreamHead>, PgCommandLogError> {
    let client = db.client().await?;
    get_head(db, &client, stream_id).await
}

/// Reads the head on a connection the caller already holds, e.g. to validate before opening a transaction.
pub async fn get_head<T>(
    db: &PgCommandLogDb,
    client: &PgConnection<T>,
    stream_id: &StreamId,
) -> Result<Option<StreamHead>, PgCommandLogError>
where
    T: PgRawConnection,
{
    db.stmts()
        .get_head
        .query_opt(client, stream_id.kind().as_str(), stream_id.id())
        .await
        .map_err(PgCommandLogError::from)
}
