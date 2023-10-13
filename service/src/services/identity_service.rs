use crate::{
    db::{DBPool, IdentityManager, NameGenerator, SessionManager},
    services,
};
use axum::Router;
use shine_service::axum::{tracing::TracingManager, ApiRoute};
use std::sync::Arc;
use utoipa::openapi::OpenApi;

struct Inner {
    tracing_manager: TracingManager,
    identity_manager: IdentityManager,
    session_manager: SessionManager,
    name_generator: NameGenerator,
    master_api_key: Option<String>,
    db: DBPool,
}

#[derive(Clone)]
pub struct IdentityServiceState(Arc<Inner>);

impl IdentityServiceState {
    pub fn tracing_manager(&self) -> &TracingManager {
        &self.0.tracing_manager
    }

    pub fn identity_manager(&self) -> &IdentityManager {
        &self.0.identity_manager
    }

    pub fn session_manager(&self) -> &SessionManager {
        &self.0.session_manager
    }

    pub fn name_generator(&self) -> &NameGenerator {
        &self.0.name_generator
    }

    pub fn master_api_key(&self) -> Option<&str> {
        self.0.master_api_key.as_deref()
    }

    pub fn db(&self) -> &DBPool {
        &self.0.db
    }
}

pub struct IdentityServiceDependencies {
    pub tracing_manager: TracingManager,
    pub identity_manager: IdentityManager,
    pub session_manager: SessionManager,
    pub name_generator: NameGenerator,
    pub db: DBPool,
}

pub struct IdentityServiceBuilder {
    state: IdentityServiceState,
}

impl IdentityServiceBuilder {
    pub fn new(dependencies: IdentityServiceDependencies, master_api_key: Option<&str>) -> Self {
        let state = IdentityServiceState(Arc::new(Inner {
            tracing_manager: dependencies.tracing_manager,
            identity_manager: dependencies.identity_manager,
            session_manager: dependencies.session_manager,
            name_generator: dependencies.name_generator,
            master_api_key: master_api_key.map(|x| x.to_owned()),
            db: dependencies.db,
        }));

        Self { state }
    }

    pub fn into_router<S>(self, doc: &mut OpenApi) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        Router::new()
            .add_api(services::ep_health(), doc)
            .add_api(services::ep_tracing_reconfigure(), doc)
            .add_api(services::ep_generate_user_name(), doc)
            .add_api(services::ep_search_identity(), doc)
            .add_api(services::ep_get_user_roles(), doc)
            .add_api(services::ep_add_user_role(), doc)
            .add_api(services::ep_delete_user_role(), doc)
            .with_state(self.state)
    }
}
