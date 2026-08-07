use super::{external_links, external_providers, sessions, tokens, user_email_confirm, user_info};
use crate::app_state::AppState;
use utoipa_axum::{router::OpenApiRouter, routes};

pub fn api_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(user_info::get_user_info))
        .routes(routes!(user_email_confirm::start_user_email_validation))
        .routes(routes!(user_email_confirm::start_user_email_change))
        .routes(routes!(user_email_confirm::complete_user_email_operation))
        .routes(routes!(tokens::create_token))
        .routes(routes!(tokens::get_token))
        .routes(routes!(tokens::list_tokens))
        .routes(routes!(tokens::delete_token))
        .routes(routes!(external_providers::list_external_providers))
        .routes(routes!(external_links::list_external_links))
        .routes(routes!(external_links::delete_external_link))
        .routes(routes!(sessions::list_sessions))
}
