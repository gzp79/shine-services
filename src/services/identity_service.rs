use crate::{
    db::{DBPool, IdentityManager, SettingsManager},
    services::ep_health,
    services::ep_search_identity,
};
use axum::{routing::get, Router};

#[derive(Clone)]
pub struct IdentityServiceState {
    pub settings_manager: SettingsManager,
    pub identity_manager: IdentityManager,
    pub db: DBPool,
}

pub struct IdentityServiceBuilder {
    state: IdentityServiceState,
}

impl IdentityServiceBuilder {
    pub fn new(state: IdentityServiceState) -> Self {
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
