use crate::repositories::{create_postgres_pool, DBConfig};
use shine_infra::db::command_log::{PgCommandLogDb, StreamId};
use shine_test::{db::get_pg_cns, test};
use uuid::Uuid;

fn db_config(sql_cns: Option<String>) -> DBConfig {
    DBConfig {
        redis_cns: "redis://localhost:6379".to_string(),
        sql_cns,
    }
}

#[test]
async fn the_builder_creates_its_schema_on_startup() {
    let Some(cns) = get_pg_cns() else { return };
    let _ = rustls::crypto::ring::default_provider().install_default();

    // The schema is applied by the pool creation, and applying it again is harmless.
    let pool = create_postgres_pool(&db_config(Some(cns.clone()))).await.unwrap();
    let pool_again = create_postgres_pool(&db_config(Some(cns))).await.unwrap();
    drop(pool_again);

    let log = PgCommandLogDb::new(&pool).await.unwrap();
    let stream_id = StreamId::parse(&format!("test-{}", Uuid::new_v4())).unwrap();
    assert_eq!(log.ensure_stream(&stream_id, "chat").await.unwrap().version, 0);
}

#[test]
async fn the_postgres_pool_needs_a_connection_string() {
    let err = create_postgres_pool(&db_config(None)).await.unwrap_err();
    assert!(
        err.to_string().contains("sqlCns"),
        "the error names the missing key: {err}"
    );
}
