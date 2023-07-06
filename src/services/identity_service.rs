use crate::{
    db::{DBPool, IdentityManager},
    services::ep_health,
    services::ep_search_identity,
};
use axum::{routing::get, Router};
use std::sync::Arc;

struct Inner {
    identity_manager: IdentityManager,
    db: DBPool,
}

#[derive(Clone)]
pub struct IdentityServiceState(Arc<Inner>);

impl IdentityServiceState {
    pub fn identity_manager(&self) -> &IdentityManager {
        &self.0.identity_manager
    }

    pub fn db(&self) -> &DBPool {
        &self.0.db
    }
}

pub struct IdentityServiceDependencies {
    pub identity_manager: IdentityManager,
    pub db: DBPool,
}

pub struct IdentityServiceBuilder {
    state: IdentityServiceState,
}

impl IdentityServiceBuilder {
    pub fn new(dependencies: IdentityServiceDependencies) -> Self {
        let state = IdentityServiceState(Arc::new(Inner {
            identity_manager: dependencies.identity_manager,
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
            .with_state(self.state)
    }
}
