use crate::{
    db::{DBPool, IdentityManager, NameGenerator},
    services,
};
use axum::{
    routing::{delete, get, put},
    Router,
};
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
            .route("/health", get(services::ep_health))
            .route("/user-name", get(services::ep_generate_user_name))
            .route("/identities", get(services::ep_search_identity))
            .route("/identities/:id/roles", put(services::ep_add_user_role))
            .route("/identities/:id/roles", delete(services::ep_delete_user_role))
            .with_state(self.state)
    }
}
