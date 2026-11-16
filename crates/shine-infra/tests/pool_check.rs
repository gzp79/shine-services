use rustls::crypto::ring;
use shine_infra::db::{postgres::create_postgres_pool, redis::create_redis_pool};
use shine_infra_testing::db::{postgres::get_postgres_cns, redis::get_redis_cns};
use shine_test::test;

const POOLED: usize = 4;
const CUT: usize = 2;

// Only the connections of the pool under test are cut, other clients of the server stay untouched.
//
// A check tries one connection, a dead one is dropped by the pool. So the checks walk through the pool,
// and once they went around it, every following one has to pass.
fn assert_walked_around(results: &[bool]) {
    assert!(
        results[POOLED..].iter().all(|ok| *ok),
        "the pool still hands out cut connections: {results:?}"
    );
}

#[test(serial = "pool-check")]
async fn test_pg_pool_check_recovers_from_cut_connections() {
    let _ = ring::default_provider().install_default();

    let cns = get_postgres_cns();
    let pool = create_postgres_pool(&cns).await.unwrap();
    let observer = create_postgres_pool(&cns).await.unwrap();

    let mut pids = Vec::new();
    {
        let mut held = Vec::new();
        for _ in 0..POOLED {
            held.push(pool.get().await.unwrap());
        }
        for conn in &held {
            let row = conn.query_one("SELECT pg_backend_pid()", &[]).await.unwrap();
            pids.push(row.get::<_, i32>(0));
        }
    }
    assert!(pool.check().await, "a pool of working connections shall be healthy");

    let helper = observer.get().await.unwrap();
    for pid in &pids[..CUT] {
        helper.execute("SELECT pg_terminate_backend($1)", &[pid]).await.unwrap();
    }

    let mut results = Vec::new();
    for _ in 0..2 * POOLED {
        results.push(pool.check().await);
    }
    assert_walked_around(&results);
}

#[test(serial = "pool-check")]
async fn test_redis_pool_check_recovers_from_cut_connections() {
    let cns = get_redis_cns();
    let pool = create_redis_pool(&cns).await.unwrap();
    let observer = create_redis_pool(&cns).await.unwrap();

    let mut ids = Vec::new();
    {
        let mut held = Vec::new();
        for _ in 0..POOLED {
            held.push(pool.get().await.unwrap());
        }
        for conn in &mut held {
            let id: i64 = redis::cmd("CLIENT").arg("ID").query_async(&mut **conn).await.unwrap();
            ids.push(id);
        }
    }
    assert!(pool.check().await, "a pool of working connections shall be healthy");

    let mut helper = observer.get().await.unwrap();
    for id in &ids[..CUT] {
        let _: i64 = redis::cmd("CLIENT")
            .arg("KILL")
            .arg("ID")
            .arg(*id)
            .query_async(&mut *helper)
            .await
            .unwrap();
    }

    let mut results = Vec::new();
    for _ in 0..2 * POOLED {
        results.push(pool.check().await);
    }
    assert_walked_around(&results);
}
