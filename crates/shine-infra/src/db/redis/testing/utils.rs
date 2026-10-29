use std::env;

pub fn get_redis_cns() -> Option<String> {
    match env::var("SHINE_TEST_REDIS_CNS") {
        Ok(cns) => Some(cns),
        Err(_) => {
            tracing::warn!("Missing SHINE_TEST_REDIS_CNS, skipping test");
            None
        }
    }
}
