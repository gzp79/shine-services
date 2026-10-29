use std::env;

pub fn get_postgres_cns() -> Option<String> {
    match env::var("SHINE_TEST_PG_CNS") {
        Ok(cns) => Some(cns),
        Err(_) => {
            tracing::warn!("Missing SHINE_TEST_PG_CNS, skipping test");
            None
        }
    }
}
