use ring::rand::SystemRandom;
use shine_infra::session::SessionKey;
use shine_infra_testing::db::redis::TestRedis;
use uuid::Uuid;

/// User sessions as the hub's session check sees them.
pub struct TestSession;

impl TestSession {
    pub fn new_random_key() -> SessionKey {
        SessionKey::new_random(&SystemRandom::new()).unwrap()
    }

    /// Create a valid session for the user and returns the key.
    pub async fn seed(redis: &TestRedis, user_id: Uuid) -> SessionKey {
        let session_key = Self::new_random_key();
        let prefix = format!("session:{}:{}", user_id.as_simple(), session_key.key_hash());
        let sentinel = serde_json::json!({ "createdAt": "2026-01-01T00:00:00Z", "fingerprint": "" });
        let data = serde_json::json!({ "name": "test", "isEmailConfirmed": false, "isLinked": false, "roles": [] });
        let mut conn = redis.direct_pool.get().await.unwrap();
        let _: () = redis::pipe()
            .set_ex(format!("{prefix}:sentinel"), sentinel.to_string(), 120)
            .set_ex(format!("{prefix}:data"), data.to_string(), 120)
            .query_async(&mut *conn)
            .await
            .unwrap();
        session_key
    }
}
