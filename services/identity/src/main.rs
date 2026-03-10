mod app_config;
mod app_state;
mod handlers;
mod models;
mod repositories;
mod routes;
mod services;

use self::{
    app_config::AppConfig,
    app_state::AppState,
    routes::{auth, health::HealthRouter, identity},
};
use anyhow::Error as AnyError;
use shine_infra::web::{WebAppConfig, WebApplication};
use utoipa_axum::router::OpenApiRouter;

struct Application {}

impl WebApplication for Application {
    type AppConfig = AppConfig;
    type AppState = AppState;

    async fn create_state(&self, config: &WebAppConfig<Self::AppConfig>) -> Result<Self::AppState, AnyError> {
        use crate::services::{UserEvent, UserLinkEvent};
        use shine_infra::sync::EventHandler;

        let state = AppState::new(config).await?;

        // Subscribe to user events for session refresh
        {
            #[derive(Clone)]
            struct OnUserEvent(AppState);
            impl EventHandler<UserEvent> for OnUserEvent {
                async fn handle(&self, event: &UserEvent) {
                    let user_id = match event {
                        UserEvent::Created(user_id) => *user_id,
                        UserEvent::Updated(user_id) => *user_id,
                        UserEvent::Deleted(user_id) => *user_id,
                        UserEvent::RoleChange(user_id) => *user_id,
                    };

                    if let Err(err) = self.0.refresh_user_session(user_id).await {
                        log::error!(
                            "Failed to refresh session for user ({user_id}) after UserEvent {event:?}: {err:?}"
                        );
                    }
                }
            }
            state
                .events()
                .subscribe::<UserEvent, _>(OnUserEvent(state.clone()))
                .await;
        }

        // Subscribe to link events for session refresh
        {
            #[derive(Clone)]
            struct OnUserLinkEvent(AppState);
            impl EventHandler<UserLinkEvent> for OnUserLinkEvent {
                async fn handle(&self, event: &UserLinkEvent) {
                    let user_id = match event {
                        UserLinkEvent::Linked(user_id) => *user_id,
                        UserLinkEvent::Unlinked(user_id) => *user_id,
                    };

                    if let Err(err) = self.0.refresh_user_session(user_id).await {
                        log::error!(
                            "Failed to refresh session for user ({user_id}) after UserLinkEvent {event:?}: {err:?}"
                        );
                    }
                }
            }
            state
                .events()
                .subscribe::<UserLinkEvent, _>(OnUserLinkEvent(state.clone()))
                .await;
        }

        Ok(state)
    }

    async fn create_routes(
        &self,
        config: &WebAppConfig<Self::AppConfig>,
    ) -> Result<OpenApiRouter<Self::AppState>, AnyError> {
        let health_controller = HealthRouter::new().into_router();
        let identity_controller = identity::IdentityRouter::new().into_router();
        let auth_controller = auth::AuthRouter::new(config).await?.into_router();

        Ok(health_controller.merge(identity_controller).merge(auth_controller))
    }
}

pub fn main() {
    let app = Application {};
    shine_infra::web::run_web_app(app);
}
