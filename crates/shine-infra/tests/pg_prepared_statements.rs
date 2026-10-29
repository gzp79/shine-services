use shine_infra::{db::postgres::create_postgres_pool, pg_prepared_statement};
use shine_infra_testing::db::postgres::get_postgres_cns;
use shine_test::test;

pg_prepared_statement!(TestQuery => "select 1", []);

#[test]
async fn test_stored_statements() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let Some(cns) = get_postgres_cns() else { return };
    let pool = create_postgres_pool(&cns).await.unwrap();
    let c1 = pool.get().await.unwrap();
    let c2 = pool.get().await.unwrap();

    let stmt = TestQuery::new(&c1).await.unwrap();

    let p1 = c1.query_one(&stmt.statement(&c1).await.unwrap(), &[]).await.unwrap();
    let p2 = c2.query_one(&stmt.statement(&c2).await.unwrap(), &[]).await.unwrap();

    // make sure the prepared statements are unique to the connection and the test checks the prepared statements handling
    assert!(c1.query_one(&stmt.statement(&c2).await.unwrap(), &[]).await.is_err());
    assert!(c2.query_one(&stmt.statement(&c1).await.unwrap(), &[]).await.is_err());

    let p1: i32 = p1.get(0);
    assert_eq!(p1, 1);
    let p2: i32 = p2.get(0);
    assert_eq!(p1, p2);
}
