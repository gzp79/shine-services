use std::env;

/// The connection string of the test Redis, see [`get_cns`].
pub fn get_redis_cns() -> Option<String> {
    get_cns("SHINE_TEST_REDIS_CNS")
}

/// The connection string of the test Postgres, see [`get_cns`].
pub fn get_pg_cns() -> Option<String> {
    get_cns("SHINE_TEST_PG_CNS")
}

/// The connection string in the environment variable, or `None` to skip the test. On CI a missing
/// connection string fails the test instead, otherwise a run could pass without a single assertion.
fn get_cns(var: &str) -> Option<String> {
    let cns = env::var(var).ok();
    if cns.is_none() {
        assert!(
            env::var_os("CI").is_none(),
            "Missing {var} on CI, the database tests must not be skipped"
        );
        tracing::warn!("Missing {var}, skipping test");
    }
    cns
}
