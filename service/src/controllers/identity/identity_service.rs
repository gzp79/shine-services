use crate::{
    identity,
    repositories::{
        AutoNameManager, DBPool, IdentityManager, Permission, PermissionError, PermissionSet, SessionManager,
    },
};
use axum::Router;
use shine_service::{
    axum::{telemetry::TelemetryManager, ApiRoute},
    service::CurrentUser,
};
use std::sync::Arc;
use utoipa::openapi::OpenApi;

struct Inner {
    db: DBPool,
    telemetry_manager: TelemetryManager,
    identity_manager: IdentityManager,
    session_manager: SessionManager,
    auto_name_manager: AutoNameManager,
    master_api_key_hash: Option<String>,
}

#[derive(Clone)]
pub struct IdentityServiceState(Arc<Inner>);

impl IdentityServiceState {
    pub fn telemetry_manager(&self) -> &TelemetryManager {
        &self.0.telemetry_manager
    }

    pub fn identity_manager(&self) -> &IdentityManager {
        &self.0.identity_manager
    }

    pub fn session_manager(&self) -> &SessionManager {
        &self.0.session_manager
    }

    pub fn auto_name_manager(&self) -> &AutoNameManager {
        &self.0.auto_name_manager
    }

    pub fn master_api_key_hash(&self) -> Option<&str> {
        self.0.master_api_key_hash.as_deref()
    }

    pub fn db(&self) -> &DBPool {
        &self.0.db
    }

    pub async fn require_permission(
        &self,
        current_user: &CurrentUser,
        permission: Permission,
    ) -> Result<(), PermissionError> {
        PermissionSet::from(current_user).require(permission)?;
        Ok(())
    }
}

pub struct IdentityServiceDependencies {
    pub db: DBPool,
    pub telemetry_manager: TelemetryManager,
    pub identity_manager: IdentityManager,
    pub session_manager: SessionManager,
    pub auto_name_manager: AutoNameManager,
}

pub struct IdentityServiceBuilder {
    state: IdentityServiceState,
}

impl IdentityServiceBuilder {
    pub fn new(dependencies: IdentityServiceDependencies, master_api_key_hash: Option<&str>) -> Self {
        let state = IdentityServiceState(Arc::new(Inner {
            db: dependencies.db,
            telemetry_manager: dependencies.telemetry_manager,
            identity_manager: dependencies.identity_manager,
            session_manager: dependencies.session_manager,
            auto_name_manager: dependencies.auto_name_manager,
            master_api_key_hash: master_api_key_hash.map(|x| x.to_owned()),
        }));

        Self { state }
    }

    pub fn into_router<S>(self, doc: &mut OpenApi) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        Router::new()
            .add_api(identity::ep_health(), doc)
            .add_api(identity::ep_metrics(), doc)
            .add_api(identity::ep_reconfigure_telemetry(), doc)
            .add_api(identity::ep_generate_user_name(), doc)
            .add_api(identity::ep_search_identity(), doc)
            .add_api(identity::ep_get_user_roles(), doc)
            .add_api(identity::ep_add_user_role(), doc)
            .add_api(identity::ep_delete_user_role(), doc)
            .with_state(self.state)
    }
}
