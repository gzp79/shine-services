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
    routes::{auth, identity},
};
use anyhow::Error as AnyError;
use shine_infra::{
    db::{PostgresPoolStatus, RedisPoolStatus},
    health::HealthService,
    web::{WebAppConfig, WebApplication},
};
use utoipa_axum::router::OpenApiRouter;

struct Application {}

impl WebApplication for Application {
    type AppConfig = AppConfig;
    type AppState = AppState;

    async fn create(
        &self,
        config: &WebAppConfig<Self::AppConfig>,
        health_service: &mut HealthService,
        router: &mut OpenApiRouter<Self::AppState>,
    ) -> Result<Self::AppState, AnyError> {
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

                    if let Err(err) = self.0.user_session_handler().refresh_user_session(user_id).await {
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

                    if let Err(err) = self.0.user_session_handler().refresh_user_session(user_id).await {
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

        // Register status providers
        health_service.add_provider(PostgresPoolStatus::new(state.db().postgres.clone()));
        health_service.add_provider(RedisPoolStatus::new(state.db().redis.clone()));

        // Register routes
        let identity_controller = identity::IdentityRouter::new().into_router();
        let auth_controller = auth::AuthRouter::new(config).await?.into_router();
        let app_router = OpenApiRouter::new().merge(identity_controller).merge(auth_controller);
        *router = router.clone().nest(&format!("/{}", self.feature_name()), app_router);

        Ok(state)
    }
}

pub fn main() {
    let app = Application {};
    shine_infra::web::run_web_app(app);
}
