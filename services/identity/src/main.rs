mod app_config;
mod app_state;
mod handlers;
mod integration;
mod models;
mod repositories;
mod routes;
mod services;
mod settings;

use self::{
    app_config::AppConfig,
    app_state::AppState,
    models::events::identity::{UserEvent, UserLinkEvent},
    repositories::{create_postgres_pool, create_redis_pool},
    routes::{auth, identity},
};
use anyhow::Error as AnyError;
use shine_infra::{
    db::{PostgresPoolStatus, RedisPoolStatus},
    sync::EventHandler,
    web::{AppBuildContext, WebAppConfig, WebApplication},
};
use utoipa_axum::router::OpenApiRouter;

struct Application {}

async fn subscribe_session_refresh_events(state: &AppState) {
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
                log::error!("Failed to refresh session for user ({user_id}) after UserEvent {event:?}: {err:?}");
            }
        }
    }

    #[derive(Clone)]
    struct OnUserLinkEvent(AppState);
    impl EventHandler<UserLinkEvent> for OnUserLinkEvent {
        async fn handle(&self, event: &UserLinkEvent) {
            let user_id = match event {
                UserLinkEvent::Linked(user_id) => *user_id,
                UserLinkEvent::Unlinked(user_id) => *user_id,
            };

            if let Err(err) = self.0.user_session_handler().refresh_user_session(user_id).await {
                log::error!("Failed to refresh session for user ({user_id}) after UserLinkEvent {event:?}: {err:?}");
            }
        }
    }

    state
        .events()
        .subscribe::<UserEvent, _>(OnUserEvent(state.clone()))
        .await;
    state
        .events()
        .subscribe::<UserLinkEvent, _>(OnUserLinkEvent(state.clone()))
        .await;
}

impl WebApplication for Application {
    type AppConfig = AppConfig;
    type AppState = AppState;

    async fn create(
        &self,
        config: &WebAppConfig<Self::AppConfig>,
        context: &mut AppBuildContext<'_>,
        router: &mut OpenApiRouter<Self::AppState>,
    ) -> Result<Self::AppState, AnyError> {
        let postgres_pool = create_postgres_pool(&config.feature.db).await?;
        let redis_pool = create_redis_pool(&config.feature.db).await?;
        let state = AppState::new(config, &postgres_pool, &redis_pool).await?;

        // Register status providers
        context.add_health_provider(PostgresPoolStatus::new(postgres_pool));
        context.add_health_provider(RedisPoolStatus::new(redis_pool));

        subscribe_session_refresh_events(&state).await;

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
