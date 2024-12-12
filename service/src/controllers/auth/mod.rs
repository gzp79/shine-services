mod auth_error;
pub use self::auth_error::*;
mod auth_session;
pub use self::auth_session::*;
mod auth_page;
pub use self::auth_page::*;
mod page_utils;
pub use self::page_utils::*;
mod captcha_utils;
pub use self::captcha_utils::*;
mod oauth2_client;
pub use self::oauth2_client::*;
mod oidc_client;
pub use self::oidc_client::*;

mod api;
mod pages;

use super::AppState;
use crate::app_config::AppConfig;
use anyhow::Error as AnyError;
use axum::Router;
use shine_service::axum::ApiRoute;
use utoipa::openapi::OpenApi;

pub struct AuthController {
    auth_session_meta: AuthSessionMeta,
}

impl AuthController {
    pub fn new(config: &AppConfig) -> Result<Self, AnyError> {
        let auth_session_meta = AuthSessionMeta::new(
            config.auth.home_url.clone(),
            config.auth.auth_base_url.clone(),
            &config.auth.auth_session,
        )?;

        Ok(Self { auth_session_meta })
    }

    pub fn into_router(self, doc: &mut OpenApi) -> Router<AppState> {
        let auth_routes = Router::new()
            .add_api(pages::page_token_login(), doc)
            .add_api(pages::page_validate(), doc)
            .add_api(pages::page_logout(), doc)
            .add_api(pages::page_delete_user(), doc)
            .layer(self.auth_session_meta.into_layer());

        let api_routes = Router::new()
            .add_api(api::ep_get_user_info(), doc)
            .add_api(api::ep_create_token(), doc)
            .add_api(api::ep_get_token(), doc)
            .add_api(api::ep_list_tokens(), doc)
            .add_api(api::ep_delete_token(), doc)
            .add_api(api::ep_list_external_providers(), doc)
            .add_api(api::ep_list_external_links(), doc)
            .add_api(api::ep_delete_external_link(), doc)
            .add_api(api::ep_list_sessions(), doc);

        Router::new().merge(auth_routes).merge(api_routes)
    }
}
