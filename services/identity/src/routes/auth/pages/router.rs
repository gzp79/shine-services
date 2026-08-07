use super::{
    delete_user, email_login, guest_login, logout, oauth2_auth, oauth2_link, oauth2_login, oidc_auth, oidc_link,
    oidc_login, token_login, validate,
};
use crate::{
    app_state::AppState,
    routes::auth::{OAuth2Client, OIDCClient},
};
use axum::Extension;
use std::sync::Arc;
use utoipa_axum::{router::OpenApiRouter, routes};

pub fn page_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(guest_login::guest_login))
        .routes(routes!(token_login::token_login))
        .routes(routes!(email_login::email_login))
        .routes(routes!(validate::validate))
        .routes(routes!(logout::logout))
        .routes(routes!(delete_user::delete_user))
}

pub fn oauth2_provider_routes(client: OAuth2Client) -> OpenApiRouter<AppState> {
    let provider = client.provider.clone();
    OpenApiRouter::new()
        .nest(
            &format!("/auth/{provider}"),
            OpenApiRouter::new().routes(routes!(oauth2_login::oauth2_login)),
        )
        .nest(
            &format!("/auth/{provider}"),
            OpenApiRouter::new().routes(routes!(oauth2_link::oauth2_link)),
        )
        .nest(
            &format!("/auth/{provider}"),
            OpenApiRouter::new().routes(routes!(oauth2_auth::oauth2_auth)),
        )
        .layer(Extension(Arc::new(client)))
}

pub fn oidc_provider_routes(client: OIDCClient) -> OpenApiRouter<AppState> {
    let provider = client.provider.clone();
    OpenApiRouter::new()
        .nest(
            &format!("/auth/{provider}"),
            OpenApiRouter::new().routes(routes!(oidc_login::oidc_login)),
        )
        .nest(
            &format!("/auth/{provider}"),
            OpenApiRouter::new().routes(routes!(oidc_link::oidc_link)),
        )
        .nest(
            &format!("/auth/{provider}"),
            OpenApiRouter::new().routes(routes!(oidc_auth::oidc_auth)),
        )
        .layer(Extension(Arc::new(client)))
}
