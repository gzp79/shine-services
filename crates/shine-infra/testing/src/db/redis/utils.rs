use std::env;

/// The connection string of the service under test. Panics if `SHINE_TEST_REDIS_CNS` is not set.
pub fn get_redis_cns() -> String {
    env::var("SHINE_TEST_REDIS_CNS").expect("SHINE_TEST_REDIS_CNS must be set")
}

/// The connection string for the test to inspect and damage data directly, bypassing any fault
/// injecting proxy. Falls back to [`get_redis_cns`] if `SHINE_TEST_REDIS_CNS_DIRECT` is not set.
pub fn get_redis_cns_direct() -> String {
    env::var("SHINE_TEST_REDIS_CNS_DIRECT").unwrap_or_else(|_| {
        tracing::warn!("Missing SHINE_TEST_REDIS_CNS_DIRECT, falling back to upstream");
        get_redis_cns()
    })
}
