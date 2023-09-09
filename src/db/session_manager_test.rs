use crate::db::{Identity, IdentityKind, SessionManager};
use chrono::{Duration, Utc};
use redis::AsyncCommands;
use ring::rand::SystemRandom;
use shine_service::service::{self, ClientFingerprint, RedisConnectionPool, SessionKey};
use shine_test::test;
use std::env;
use uuid::Uuid;

async fn create_manager(scope: &str) -> Option<(SessionManager, RedisConnectionPool)> {
    match env::var("SHINE_TEST_REDIS_CNS") {
        Ok(cns) => {
            let redis = service::create_redis_pool(cns.as_str()).await.unwrap();
            let session_manager = SessionManager::new(&redis, format!("{scope}_"), Duration::seconds(1000))
                .await
                .unwrap();
            Some((session_manager, redis))
        }
        _ => {
            log::warn!("Missing SHINE_TEST_REDIS_CNS, skipping test");
            None
        }
    }
}

#[test]
async fn create_get_remove() {
    let scope = &Uuid::new_v4().to_string()[..5];
    log::debug!("test scope: {scope}");
    let (session_manager, redis) = match create_manager(scope).await {
        Some(session_manager) => session_manager,
        None => return,
    };

    let identity = Identity {
        id: Uuid::new_v4(),
        kind: IdentityKind::User,
        name: "user".into(),
        email: None,
        is_email_confirmed: false,
        created: Utc::now(),
        version: 1,
    };
    let roles = vec!["R1".into(), "R2".into()];
    let fingerprint = ClientFingerprint { agent: "test".into() };

    log::info!("Creating a new session...");
    let session = session_manager
        .create(&identity, roles.clone(), &fingerprint)
        .await
        .unwrap();
    log::debug!("session: {session:#?}");
    assert_eq!(identity.id, session.user_id);
    assert_eq!(identity.name, session.name);
    assert_eq!(fingerprint.hash(), session.fingerprint_hash);
    assert_eq!(roles, session.roles);

    log::info!("Finding the session...");
    let found_session = session_manager
        .find(identity.id, session.key)
        .await
        .unwrap()
        .expect("Session should have been found");
    log::debug!("found_session: {found_session:#?}");
    assert_eq!(session.key, found_session.key);
    assert_eq!(identity.id, found_session.user_id);
    assert_eq!(identity.name, found_session.name);
    assert_eq!(fingerprint.hash(), found_session.fingerprint_hash);
    assert_eq!(roles, found_session.roles);

    log::info!("Remove session...");
    session_manager.remove(identity.id, session.key).await.unwrap();
    {
        let (_, key) = session_manager.keys(identity.id, &session.key);
        let client = &mut *redis.get().await.unwrap();
        let versions: Vec<String> = client.hkeys(&key).await.unwrap();
        assert!(
            versions.is_empty(),
            "without concurrency after remove, no session data shall remain"
        );
    }

    log::info!("Finding after remove...");
    let found_session = session_manager.find(identity.id, session.key).await.unwrap();
    assert!(found_session.is_none());
}

#[test]
async fn no_create_update() {
    let scope = &Uuid::new_v4().to_string()[..5];
    log::debug!("test scope: {scope}");
    let (session_manager, _) = match create_manager(scope).await {
        Some(session_manager) => session_manager,
        None => return,
    };

    let random = SystemRandom::new();

    let identity = Identity {
        id: Uuid::new_v4(),
        kind: IdentityKind::User,
        name: "user".into(),
        email: None,
        is_email_confirmed: false,
        created: Utc::now(),
        version: 1,
    };
    let roles = vec!["R1".into(), "R2".into()];

    let session = session_manager
        .update(SessionKey::new_random(&random).unwrap(), &identity, roles.clone())
        .await
        .unwrap();
    assert!(session.is_none());
}

#[test]
async fn create_update() {
    let scope = &Uuid::new_v4().to_string()[..5];
    log::debug!("test scope: {scope}");
    let (session_manager, _) = match create_manager(scope).await {
        Some(session_manager) => session_manager,
        None => return,
    };

    let identity1 = Identity {
        id: Uuid::new_v4(),
        kind: IdentityKind::User,
        name: "user".into(),
        email: None,
        is_email_confirmed: false,
        created: Utc::now(),
        version: 1,
    };
    let roles1 = vec!["R1".into(), "R2".into()];
    let fingerprint = ClientFingerprint { agent: "test".into() };

    log::info!("Creating a new session...");
    let session = session_manager
        .create(&identity1, roles1.clone(), &fingerprint)
        .await
        .unwrap();

    log::info!("Update to version 5");
    let mut identity5 = identity1.clone();
    identity5.version = 5;
    let roles5 = vec!["R2".into(), "R5".into()];
    let updated_session = session_manager
        .update(session.key, &identity5, roles5.clone())
        .await
        .unwrap();
    let updated_session = updated_session.expect("Session should be available");
    assert_eq!(session.key, updated_session.key);
    assert_eq!(identity5.id, updated_session.user_id);
    assert_eq!(identity5.name, updated_session.name);
    assert_eq!(identity5.version, updated_session.version);
    assert_eq!(fingerprint.hash(), updated_session.fingerprint_hash);
    assert_eq!(roles5, updated_session.roles);

    {
        log::info!("Finding the session with version 5 ...");
        let found_session = session_manager
            .find(identity5.id, session.key)
            .await
            .unwrap()
            .expect("Session should have been found");
        log::debug!("found_session: {found_session:#?}");
        assert_eq!(session.key, found_session.key);
        assert_eq!(identity5.id, found_session.user_id);
        assert_eq!(identity5.name, found_session.name);
        assert_eq!(fingerprint.hash(), found_session.fingerprint_hash);
        assert_eq!(roles5, found_session.roles);
    }

    {
        log::info!("Update to version 3 should have no effect");
        let mut identity3 = identity1.clone();
        let roles3 = vec!["R2".into(), "R3".into()];
        identity3.version = 3;
        let updated_session = session_manager.update(session.key, &identity3, roles3).await.unwrap();
        let updated_session = updated_session.expect("Session should be available");
        // it should have no effect on the update
        assert_eq!(session.key, updated_session.key);
        assert_eq!(identity5.id, updated_session.user_id);
        assert_eq!(identity5.name, updated_session.name);
        assert_eq!(identity5.version, updated_session.version);
        assert_eq!(fingerprint.hash(), updated_session.fingerprint_hash);
        assert_eq!(roles5, updated_session.roles);

        log::info!("Finding the session with version 5 after storing version 3 ...");
        let found_session = session_manager
            .find(identity5.id, session.key)
            .await
            .unwrap()
            .expect("Session should have been found");
        log::debug!("found_session: {found_session:#?}");
        assert_eq!(session.key, found_session.key);
        assert_eq!(identity5.id, found_session.user_id);
        assert_eq!(identity5.name, found_session.name);
        assert_eq!(fingerprint.hash(), found_session.fingerprint_hash);
        assert_eq!(roles5, found_session.roles);
    }

    {
        log::info!("Update to version 5 again with different roles should have no effect");
        let roles5b = vec!["R2".into(), "R52".into()];
        let updated_session = session_manager.update(session.key, &identity5, roles5b).await.unwrap();
        let updated_session = updated_session.expect("Session should be available");
        assert_eq!(session.key, updated_session.key);
        assert_eq!(identity5.id, updated_session.user_id);
        assert_eq!(identity5.name, updated_session.name);
        assert_eq!(identity5.version, updated_session.version);
        assert_eq!(fingerprint.hash(), updated_session.fingerprint_hash);
        assert_eq!(roles5, updated_session.roles);

        log::info!("Finding the session with version 5 after storing version 3 ...");
        let found_session = session_manager
            .find(identity5.id, session.key)
            .await
            .unwrap()
            .expect("Session should have been found");
        log::debug!("found_session: {found_session:#?}");
        assert_eq!(session.key, found_session.key);
        assert_eq!(identity5.id, found_session.user_id);
        assert_eq!(identity5.name, found_session.name);
        assert_eq!(fingerprint.hash(), found_session.fingerprint_hash);
        assert_eq!(roles5, found_session.roles);
    }
}

#[test]
async fn create_many_remove_all() {
    let scope = &Uuid::new_v4().to_string()[..5];
    log::debug!("test scope: {scope}");
    let (session_manager, redis) = match create_manager(scope).await {
        Some(session_manager) => session_manager,
        None => return,
    };

    let identity = Identity {
        id: Uuid::new_v4(),
        kind: IdentityKind::User,
        name: "user".into(),
        email: None,
        is_email_confirmed: false,
        created: Utc::now(),
        version: 1,
    };
    let roles = vec!["R1".into(), "R2".into()];
    let fingerprint = ClientFingerprint { agent: "test".into() };

    // generate a few sessions for user1
    let mut keys = vec![];
    for _ in 0..10 {
        let session = session_manager
            .create(&identity, roles.clone(), &fingerprint)
            .await
            .unwrap();
        keys.push(session.key);
    }

    // create a session for another user
    let mut identity2 = identity.clone();
    identity2.id = Uuid::new_v4();
    let session2 = session_manager
        .create(&identity2, roles.clone(), &fingerprint)
        .await
        .unwrap();

    // delete sessions of user1
    session_manager.remove_all(identity.id).await.unwrap();
    for key in keys {
        let (_, key) = session_manager.keys(identity.id, &key);
        let client = &mut *redis.get().await.unwrap();
        let versions: Vec<String> = client.hkeys(&key).await.unwrap();
        assert!(
            versions.is_empty(),
            "without concurrency after remove_all, no session should be present"
        );
    }

    // check session of user2, it shall not be deleted
    let found_session = session_manager
        .find(identity2.id, session2.key)
        .await
        .unwrap()
        .expect("Session should have been found");
    assert_eq!(session2.key, found_session.key);
    assert_eq!(session2.user_id, identity2.id);
}
