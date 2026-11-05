//! Proves the generic escape hatch for parameter types whose OID isn't a compile-time constant
//! (`PgValueTypeUnknown`, `Type::UNKNOWN`): an array of a Postgres composite type, bound from a
//! `#[derive(ToSql)]` Rust struct and unnested back into rows.

use postgres_from_row::FromRow;
use postgres_types::ToSql;
use shine_infra::{
    db::postgres::{create_postgres_pool, PgValueTypeUnknown, ToPgType},
    pg_query,
};
use shine_infra_testing::db::postgres::get_postgres_cns;
use shine_test::test;

#[derive(Debug, Clone, ToSql)]
#[postgres(name = "shine_test_pair")]
struct TestPair {
    label: String,
    value: i64,
}

impl ToPgType for TestPair {
    type PgValueType = PgValueTypeUnknown;
}

#[derive(FromRow)]
struct PairRow {
    label: String,
    value: i64,
}

pg_query!( SelectPairs =>
    in = pairs: &[TestPair];
    out = PairRow;
    sql = r#"
        SELECT label, value FROM UNNEST($1::shine_test_pair[]) AS t(label, value)
        ORDER BY value
    "#
);

#[test]
async fn an_array_of_a_composite_type_binds_and_unnests() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let Some(cns) = get_postgres_cns() else { return };
    let pool = create_postgres_pool(&cns).await.unwrap();
    let client = pool.get().await.unwrap();

    // Kept independent of any migration: Postgres has no `CREATE TYPE IF NOT EXISTS`, so
    // check the catalog first and create it only if this is the first run against this DB.
    client
        .batch_execute(
            r#"
                    DO $$
                    BEGIN
                        IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'shine_test_pair') THEN
                            CREATE TYPE shine_test_pair AS (label text, value bigint);
                        END IF;
                    END
                    $$;
                    "#,
        )
        .await
        .unwrap();

    let stmt = SelectPairs::new(&client).await.unwrap();
    let pairs = vec![
        TestPair { label: "b".into(), value: 2 },
        TestPair { label: "a".into(), value: 1 },
    ];
    let rows = stmt.query(&client, pairs.as_slice()).await.unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!((rows[0].label.as_str(), rows[0].value), ("a", 1));
    assert_eq!((rows[1].label.as_str(), rows[1].value), ("b", 2));
}
