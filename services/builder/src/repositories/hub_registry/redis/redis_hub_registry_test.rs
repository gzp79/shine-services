//! The registry rows on a real Redis. Tests share the database and stay apart by using users of their own.

use crate::repositories::hub_registry::{
    redis::{hub_connection_key, RedisHubConnectionDb},
    HubConnection, HubConnectionDb, HubRegistry,
};
use shine_infra_testing::db::redis::TestRedis;
use shine_test::test;
use uuid::Uuid;

const TTL: u64 = 120;

struct TestRegistry {
    redis: TestRedis,
    db: RedisHubConnectionDb,
}

async fn registry() -> TestRegistry {
    let redis = TestRedis::from_env().await;
    let db = RedisHubConnectionDb::new(&redis.direct_pool, TTL).await.unwrap();
    TestRegistry { redis, db }
}

impl TestRegistry {
    async fn create(&self, user_id: Uuid, connection_id: Uuid) {
        let mut context = self.db.create_context().await.unwrap();
        context.create_connection(user_id, connection_id).await.unwrap();
    }

    async fn heartbeat(&self, user_id: Uuid, connection_id: Uuid) -> bool {
        let mut context = self.db.create_context().await.unwrap();
        context.heartbeat_connection(user_id, connection_id).await.unwrap()
    }

    async fn heartbeat_all(&self, connections: &[HubConnection]) -> Vec<HubConnection> {
        let mut context = self.db.create_context().await.unwrap();
        context.heartbeat_connections(connections).await.unwrap()
    }

    async fn remove(&self, user_id: Uuid, connection_id: Uuid) -> bool {
        let mut context = self.db.create_context().await.unwrap();
        context
            .remove_connection_if_active(user_id, connection_id)
            .await
            .unwrap()
    }

    async fn find(&self, user_id: Uuid) -> Option<Uuid> {
        let mut context = self.db.create_context().await.unwrap();
        context
            .find_connection_by_user(user_id)
            .await
            .unwrap()
            .map(|connection| connection.connection_id)
    }

    async fn ttl(&self, user_id: Uuid) -> i64 {
        self.redis.ttl(&hub_connection_key(user_id)).await
    }

    async fn clear(&self, user_id: Uuid) {
        self.redis.del(&hub_connection_key(user_id)).await;
    }
}

#[test]
async fn a_heartbeat_extends_the_ttl_of_the_active_connection() {
    let registry = registry().await;
    let (user, connection) = (Uuid::new_v4(), Uuid::new_v4());
    registry.create(user, connection).await;
    registry.redis.expire(&hub_connection_key(user), 5).await;

    assert!(registry.heartbeat(user, connection).await);

    assert!(registry.ttl(user).await > 5, "the TTL was not extended");
    assert_eq!(registry.find(user).await, Some(connection));
    registry.clear(user).await;
}

/// Redis is the truth about who holds the user, and only a connect writes the row: a heartbeat finding it gone
/// reports the connection and does not bring the row back.
#[test]
async fn a_heartbeat_reports_a_connection_whose_row_is_gone_and_does_not_write_it() {
    let registry = registry().await;
    let (user, connection) = (Uuid::new_v4(), Uuid::new_v4());
    registry.create(user, connection).await;
    registry.clear(user).await;

    assert!(!registry.heartbeat(user, connection).await);

    assert_eq!(registry.find(user).await, None);
}

#[test]
async fn a_heartbeat_reports_a_connection_another_one_replaced_and_leaves_the_row() {
    let registry = registry().await;
    let (user, old, new) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    registry.create(user, old).await;
    registry.create(user, new).await;

    assert!(!registry.heartbeat(user, old).await);

    assert_eq!(registry.find(user).await, Some(new));
    registry.clear(user).await;
}

#[test]
async fn the_batched_heartbeat_refreshes_the_live_rows_and_reports_the_others() {
    let registry = registry().await;
    let live = HubConnection {
        user_id: Uuid::new_v4(),
        connection_id: Uuid::new_v4(),
    };
    let gone = HubConnection {
        user_id: Uuid::new_v4(),
        connection_id: Uuid::new_v4(),
    };
    let replaced = HubConnection {
        user_id: Uuid::new_v4(),
        connection_id: Uuid::new_v4(),
    };
    let winner = Uuid::new_v4();
    registry.create(live.user_id, live.connection_id).await;
    registry.redis.expire(&hub_connection_key(live.user_id), 5).await;
    registry.create(replaced.user_id, winner).await;

    let stale = registry.heartbeat_all(&[live, gone, replaced]).await;

    assert_eq!(stale, vec![gone, replaced]);
    assert!(registry.ttl(live.user_id).await > 5, "the live row was not refreshed");
    assert_eq!(registry.find(gone.user_id).await, None, "the missing row was written");
    assert_eq!(registry.find(replaced.user_id).await, Some(winner));
    for user in [live.user_id, gone.user_id, replaced.user_id] {
        registry.clear(user).await;
    }
}

#[test]
async fn the_batched_heartbeat_of_nothing_reports_nothing() {
    let registry = registry().await;

    assert!(registry.heartbeat_all(&[]).await.is_empty());
}

#[test]
async fn a_connection_is_removed_only_by_its_own_id() {
    let registry = registry().await;
    let (user, connection, other) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    registry.create(user, connection).await;

    assert!(!registry.remove(user, other).await);
    assert_eq!(
        registry.find(user).await,
        Some(connection),
        "another id removed the row"
    );
    assert!(registry.remove(user, connection).await);
    assert_eq!(registry.find(user).await, None);
    assert!(!registry.remove(user, connection).await, "nothing left to remove");
}
