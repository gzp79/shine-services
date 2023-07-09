use crate::{
    db::{DBPool, IdentityManager, NameGenerator},
    services::{ep_generate_user_name, ep_health, ep_search_identity},
};
use axum::{routing::get, Router};
use std::sync::Arc;

struct Inner {
    identity_manager: IdentityManager,
    name_generator: NameGenerator,
    db: DBPool,
}

#[derive(Clone)]
pub struct IdentityServiceState(Arc<Inner>);

impl IdentityServiceState {
    pub fn identity_manager(&self) -> &IdentityManager {
        &self.0.identity_manager
    }

    pub fn name_generator(&self) -> &NameGenerator {
        &self.0.name_generator
    }

    pub fn db(&self) -> &DBPool {
        &self.0.db
    }
}

pub struct IdentityServiceDependencies {
    pub identity_manager: IdentityManager,
    pub name_generator: NameGenerator,
    pub db: DBPool,
}

pub struct IdentityServiceBuilder {
    state: IdentityServiceState,
}

impl IdentityServiceBuilder {
    pub fn new(dependencies: IdentityServiceDependencies) -> Self {
        let state = IdentityServiceState(Arc::new(Inner {
            identity_manager: dependencies.identity_manager,
            name_generator: dependencies.name_generator,
            db: dependencies.db,
        }));

        Self { state }
    }

    pub fn into_router<S>(self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        Router::new()
            .route("/identities", get(ep_search_identity::search_identity))
            .route("/health", get(ep_health::status))
            .route("/user-name", get(ep_generate_user_name::get_username))
            .with_state(self.state)
    }
}
