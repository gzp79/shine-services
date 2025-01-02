use crate::{
    repositories::{
        identity::{Identity, IdentityKind},
        session::{redis::RedisSessionDb, SessionDb},
    },
    services::SessionService,
};
use chrono::{Duration, Utc};
use ring::rand::SystemRandom;
use shine_core::{
    db,
    web::{ClientFingerprint, SessionKey, SiteInfo},
};
use shine_test::test;
use std::env;
use uuid::Uuid;

async fn create_db(scope: &str) -> Option<impl SessionDb + Clone> {
    match env::var("SHINE_TEST_REDIS_CNS") {
        Ok(cns) => {
            let redis = db::create_redis_pool(cns.as_str()).await.unwrap();
            let db = RedisSessionDb::new(&redis, format!("{scope}_"), Duration::seconds(1000))
                .await
                .unwrap();
            Some(db)
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
    let session_manager = match create_db(scope).await {
        Some(db) => SessionService::new(db),
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
    let fingerprint = ClientFingerprint::from_agent("test".into()).unwrap();
    let site_info = SiteInfo {
        agent: "test".into(),
        country: None,
        region: None,
        city: None,
    };

    log::info!("Creating a new session...");
    let (session, session_key) = session_manager
        .create(&identity, roles.clone(), &fingerprint, &site_info)
        .await
        .unwrap();
    log::debug!("session: {session:#?}");
    assert_eq!(identity.id, session.info.user_id);
    assert_eq!(fingerprint.as_str(), session.info.fingerprint);
    assert_eq!(identity.name, session.user.name);
    assert_eq!(roles, session.user.roles);

    log::info!("Finding the session...");
    let found_session = session_manager
        .find(identity.id, &session_key)
        .await
        .unwrap()
        .expect("Session should have been found");
    log::debug!("found_session: {found_session:#?}");
    assert_eq!(session.info.key_hash, found_session.info.key_hash);
    assert_eq!(identity.id, found_session.info.user_id);
    assert_eq!(fingerprint.as_str(), found_session.info.fingerprint);
    assert_eq!(identity.name, found_session.user.name);
    assert_eq!(roles, found_session.user.roles);

    log::info!("Remove session...");
    session_manager.remove(identity.id, &session_key).await.unwrap();
    {
        let sessions = session_manager.find_all(identity.id).await.unwrap();
        assert!(
            sessions.is_empty(),
            "without concurrency after remove, no session data shall remain"
        );
    }

    log::info!("Finding after remove...");
    let found_session = session_manager.find(identity.id, &session_key).await.unwrap();
    assert!(found_session.is_none());
}

#[test]
async fn update_invalid_key() {
    let scope = &Uuid::new_v4().to_string()[..5];
    log::debug!("test scope: {scope}");
    let session_manager = match create_db(scope).await {
        Some(db) => SessionService::new(db),
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
        .update_user_info(&SessionKey::new_random(&random).unwrap(), &identity, &roles)
        .await
        .unwrap();
    assert!(session.is_none());
}

#[test]
async fn create_update() {
    let scope = &Uuid::new_v4().to_string()[..5];
    log::debug!("test scope: {scope}");
    let session_manager = match create_db(scope).await {
        Some(db) => SessionService::new(db),
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
    let fingerprint = ClientFingerprint::from_agent("test".into()).unwrap();
    let site_info = SiteInfo {
        agent: "test".into(),
        country: None,
        region: None,
        city: None,
    };

    log::info!("Creating a new session...");
    let (session, session_key) = session_manager
        .create(&identity1, roles1.clone(), &fingerprint, &site_info)
        .await
        .unwrap();

    log::info!("Update to version 5");
    let mut identity5 = identity1.clone();
    identity5.version = 5;
    let roles5 = vec!["R2".into(), "R5".into()];
    let updated_session = session_manager
        .update_user_info(&session_key, &identity5, &roles5)
        .await
        .unwrap();
    let updated_session = updated_session.expect("Session should be available");
    assert_eq!(session.info.key_hash, updated_session.info.key_hash);
    assert_eq!(identity5.id, updated_session.info.user_id);
    assert_eq!(fingerprint.as_str(), updated_session.info.fingerprint);
    assert_eq!(identity5.version, updated_session.user_version);
    assert_eq!(identity5.name, updated_session.user.name);
    assert_eq!(roles5, updated_session.user.roles);

    {
        log::info!("Finding the session with version 5 ...");
        let found_session = session_manager
            .find(identity5.id, &session_key)
            .await
            .unwrap()
            .expect("Session should have been found");
        log::debug!("found_session: {found_session:#?}");
        assert_eq!(session.info.key_hash, found_session.info.key_hash);
        assert_eq!(identity5.id, found_session.info.user_id);
        assert_eq!(fingerprint.as_str(), found_session.info.fingerprint);
        assert_eq!(identity5.version, found_session.user_version);
        assert_eq!(identity5.name, found_session.user.name);
        assert_eq!(roles5, found_session.user.roles);
    }

    {
        log::info!("Update to version 3 should have no effect");
        let mut identity3 = identity1.clone();
        let roles3 = vec!["R2".into(), "R3".into()];
        identity3.version = 3;
        let updated_session = session_manager
            .update_user_info(&session_key, &identity3, &roles3)
            .await
            .unwrap();
        let updated_session = updated_session.expect("Session should be available");
        // it should have no effect on the update
        assert_eq!(session.info.key_hash, updated_session.info.key_hash);
        assert_eq!(identity5.id, updated_session.info.user_id);
        assert_eq!(fingerprint.as_str(), updated_session.info.fingerprint);
        assert_eq!(identity5.version, updated_session.user_version);
        assert_eq!(identity5.name, updated_session.user.name);
        assert_eq!(roles5, updated_session.user.roles);

        log::info!("Finding the session with version 5 after storing version 3 ...");
        let found_session = session_manager
            .find(identity5.id, &session_key)
            .await
            .unwrap()
            .expect("Session should have been found");
        log::debug!("found_session: {found_session:#?}");
        assert_eq!(session.info.key_hash, found_session.info.key_hash);
        assert_eq!(identity5.id, found_session.info.user_id);
        assert_eq!(fingerprint.as_str(), found_session.info.fingerprint);
        assert_eq!(identity5.version, found_session.user_version);
        assert_eq!(identity5.name, found_session.user.name);
        assert_eq!(roles5, found_session.user.roles);
    }

    {
        log::info!("Update to version 5 again with different roles should have no effect");
        let roles5b = vec!["R2".into(), "R52".into()];
        let updated_session = session_manager
            .update_user_info(&session_key, &identity5, &roles5b)
            .await
            .unwrap();
        let updated_session = updated_session.expect("Session should be available");
        assert_eq!(session.info.key_hash, updated_session.info.key_hash);
        assert_eq!(identity5.id, updated_session.info.user_id);
        assert_eq!(fingerprint.as_str(), updated_session.info.fingerprint);
        assert_eq!(identity5.version, updated_session.user_version);
        assert_eq!(identity5.name, updated_session.user.name);
        assert_eq!(roles5, updated_session.user.roles);

        log::info!("Finding the session with version 5 after storing version 5 with altered roles ...");
        let found_session = session_manager
            .find(identity5.id, &session_key)
            .await
            .unwrap()
            .expect("Session should have been found");
        log::debug!("found_session: {found_session:#?}");
        assert_eq!(session.info.key_hash, found_session.info.key_hash);
        assert_eq!(identity5.id, found_session.info.user_id);
        assert_eq!(fingerprint.as_str(), found_session.info.fingerprint);
        assert_eq!(identity5.name, found_session.user.name);
        assert_eq!(identity5.version, found_session.user_version);
        assert_eq!(roles5, found_session.user.roles);
    }
}

#[test]
async fn create_many_remove_all() {
    let scope = &Uuid::new_v4().to_string()[..5];
    log::debug!("test scope: {scope}");
    let session_manager = match create_db(scope).await {
        Some(db) => SessionService::new(db),
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
    let fingerprint = ClientFingerprint::from_agent("test".into()).unwrap();
    let site_info = SiteInfo {
        agent: "test".into(),
        country: None,
        region: None,
        city: None,
    };

    // generate a few sessions for user1
    let mut keys = vec![];
    for _ in 0..10 {
        let (_, session_key) = session_manager
            .create(&identity, roles.clone(), &fingerprint, &site_info)
            .await
            .unwrap();
        keys.push(session_key);
    }

    // create a session for another user
    let mut identity2 = identity.clone();
    identity2.id = Uuid::new_v4();
    let (session2, session2_key) = session_manager
        .create(&identity2, roles.clone(), &fingerprint, &site_info)
        .await
        .unwrap();

    // delete sessions of user1
    session_manager.remove_all(identity.id).await.unwrap();
    let keys = session_manager.find_all(identity.id).await.unwrap();
    assert!(
        keys.is_empty(),
        "without concurrency after remove, no session data shall remain"
    );

    // check session of user2, it shall not be deleted
    let found_session = session_manager
        .find(identity2.id, &session2_key)
        .await
        .unwrap()
        .expect("Session should have been found");
    assert_eq!(session2.info.key_hash, found_session.info.key_hash);
    assert_eq!(identity2.id, found_session.info.user_id);
}
