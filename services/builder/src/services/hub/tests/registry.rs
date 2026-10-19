//! Direct tests of the registry operations: each one is a single atomic step on Redis, so the
//! interesting cases are the ownership checks and the batch boundaries.

use crate::{
    repositories::hub_registry::{HubConnection, HubConnectionDb, HubRegistry},
    services::hub_test_utils::{create_test_hub, delete_key, find_registry_connection, registry_key, TestHubOptions},
};
use shine_test::test;
use uuid::Uuid;

#[test(serial = "hub-service")]
async fn remove_only_deletes_an_entry_owned_by_the_connection() {
    let Some(test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let (user, owner) = (Uuid::new_v4(), Uuid::new_v4());
    let mut context = test.registry.create_context().await.unwrap();

    assert!(!context.remove_connection_if_active(user, owner).await.unwrap());

    context.create_connection(user, owner).await.unwrap();
    assert!(!context.remove_connection_if_active(user, Uuid::new_v4()).await.unwrap());
    assert_eq!(find_registry_connection(&test.registry, user).await, Some(owner));

    assert!(context.remove_connection_if_active(user, owner).await.unwrap());
    assert_eq!(find_registry_connection(&test.registry, user).await, None);
    assert!(!context.remove_connection_if_active(user, owner).await.unwrap());
}

#[test(serial = "hub-service")]
async fn heartbeat_restores_a_missing_entry_but_never_overrides_another_owner() {
    let Some(test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let (user, ours, theirs) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let mut context = test.registry.create_context().await.unwrap();

    assert!(context.heartbeat_connection(user, ours).await.unwrap());
    assert_eq!(find_registry_connection(&test.registry, user).await, Some(ours));

    context.create_connection(user, theirs).await.unwrap();
    assert!(!context.heartbeat_connection(user, ours).await.unwrap());
    assert_eq!(find_registry_connection(&test.registry, user).await, Some(theirs));

    delete_key(&test.redis, &registry_key(user)).await;
}

#[test(serial = "hub-service")]
async fn batched_heartbeat_handles_more_connections_than_one_chunk() {
    let Some(test) = create_test_hub(TestHubOptions::default()).await else {
        return;
    };
    let mut context = test.registry.create_context().await.unwrap();

    // Foreign owners at chunk edges and in the middle, the rest missing or ours.
    let connections: Vec<HubConnection> = (0..700)
        .map(|_| HubConnection {
            user_id: Uuid::new_v4(),
            connection_id: Uuid::new_v4(),
        })
        .collect();
    let foreign = [0usize, 255, 256, 511, 512, 699];
    for (i, connection) in connections.iter().enumerate() {
        if foreign.contains(&i) {
            context.create_connection(connection.user_id, Uuid::new_v4()).await.unwrap();
        } else if i % 2 == 0 {
            context
                .create_connection(connection.user_id, connection.connection_id)
                .await
                .unwrap();
        }
    }

    let mut stale = context.heartbeat_connections(&connections).await.unwrap();
    let mut expected: Vec<_> = foreign.iter().map(|i| connections[*i]).collect();
    stale.sort_by_key(|c| c.user_id);
    expected.sort_by_key(|c| c.user_id);
    assert_eq!(stale, expected);

    for (i, connection) in connections.iter().enumerate() {
        if !foreign.contains(&i) {
            assert_eq!(
                find_registry_connection(&test.registry, connection.user_id).await,
                Some(connection.connection_id),
                "entry {i} must be owned by its connection after the heartbeat"
            );
        }
        delete_key(&test.redis, &registry_key(connection.user_id)).await;
    }
}
