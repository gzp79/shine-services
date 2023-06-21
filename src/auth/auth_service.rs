use crate::{
    auth::{ExternalLoginMeta, ExternalLoginSession, OIDCConfig, OIDCServiceBuilder},
    db::{DBError, DBSessionError, IdentityManager, SessionManager},
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Extension, Router,
};
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::session::SessionError,
    service::{CurrentUser, UserSession, DOMAIN_NAME},
};
use std::{collections::HashMap, sync::Arc};
use tera::Tera;
use thiserror::Error as ThisError;
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthConfig {
    pub openid: HashMap<String, OIDCConfig>,
}

#[derive(Debug, ThisError)]
pub enum AuthServiceError {
    #[error("Session cookie was missing or corrupted")]
    MissingSession,
    #[error("Cross Server did not return an ID token")]
    InvalidCsrfState,
    #[error("Session and external login cookies are not matching")]
    InconsistentSession,
    #[error("Failed to exchange authorization code to access token: {0}")]
    FailedTokenExchange(String),
    #[error("Cross-Site Request Forgery (Csrf) check failed")]
    MissingIdToken,
    #[error("Failed to verify id token: {0}")]
    FailedIdVerification(String),

    #[error("Failed to create session")]
    DBSessionError(#[from] DBSessionError),
    #[error(transparent)]
    DBError(#[from] DBError),
    #[error(transparent)]
    TeraError(#[from] tera::Error),
}

impl IntoResponse for AuthServiceError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            AuthServiceError::MissingSession => StatusCode::BAD_REQUEST,
            AuthServiceError::InconsistentSession => StatusCode::BAD_REQUEST,
            AuthServiceError::InvalidCsrfState => StatusCode::BAD_REQUEST,
            AuthServiceError::FailedTokenExchange(_) => StatusCode::BAD_REQUEST,
            AuthServiceError::MissingIdToken => StatusCode::BAD_REQUEST,
            AuthServiceError::FailedIdVerification(_) => StatusCode::BAD_REQUEST,
            AuthServiceError::DBError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AuthServiceError::TeraError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            AuthServiceError::DBSessionError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}

#[derive(Debug, ThisError)]
pub enum AuthBuildError {
    #[error(transparent)]
    InvalidSessionMeta(#[from] SessionError),

    #[error("Invalid issuer url: {0}")]
    InvalidIssuer(String),
    #[error("Invalid auth url: {0}")]
    InvalidAuth(String),
    #[error("Invalid token url: {0}")]
    InvalidToken(String),
    #[error("Invalid user info url: {0}")]
    InvalidUserInfo(String),
    #[error("Missing OpenId discover or endpoint configuration")]
    InvalidEndpoints,
    #[error("Invalid redirect url: {0}")]
    RedirectUrl(String),
    #[error("Failed to discover open id: {0}")]
    Discovery(String),
}

struct ServiceState {
    home_url: String,
    session_manager: SessionManager,
}

type Service = Arc<ServiceState>;

#[derive(Deserialize)]
struct LogoutRequest {
    terminate_all: Option<bool>,
}

async fn logout(
    Extension(tera): Extension<Arc<Tera>>,
    State(service): State<Service>,
    Query(query): Query<LogoutRequest>,
    mut user_session: UserSession,
    mut external_login: ExternalLoginSession,
) -> Response {
    let user_session_data = user_session.take();
    let _ = external_login.take();

    let (status, template, context) =
        if let Err(err) = perform_logout(&service, user_session_data, query.terminate_all.unwrap_or(false)).await {
            let mut context = tera::Context::new();
            context.insert("error", &format!("{err:?}"));
            (StatusCode::INTERNAL_SERVER_ERROR, "error.html", context)
        } else {
            let mut context = tera::Context::new();
            context.insert("title", &"Logout");
            context.insert("target", &"home");
            context.insert("redirect_url", &service.home_url.to_string());
            (StatusCode::OK, "redirect.html", context)
        };

    // make sure dispite of having any server error, the session cookies are cleared
    match tera.render(template, &context) {
        Ok(html) => (status, user_session, external_login, Html(html)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            user_session,
            external_login,
            format!("template error: {err:?}"),
        )
            .into_response(),
    }
}

async fn perform_logout(service: &Service, current_user: Option<CurrentUser>, remove_all: bool) -> Result<(), DBError> {
    if let Some(current_user) = current_user {
        if remove_all {
            service.session_manager.remove_all(current_user.user_id).await?;
        } else {
            service
                .session_manager
                .remove(current_user.user_id, current_user.key)
                .await?;
        }
    }

    Ok(())
}

pub struct AuthServiceBuilder {
    external_login_cookie_builder: ExternalLoginMeta,
    home_url: String,
    openid_connections: Vec<OIDCServiceBuilder>,
    //todo:  user_query: IdentityServiceBuilder, - find/fix existing users
    session_manager: SessionManager,
}

impl AuthServiceBuilder {
    pub async fn new(
        config: &AuthConfig,
        cookie_secret: &str,
        home_url: &Url,
        identity_manager: &IdentityManager,
        session_manager: &SessionManager,
    ) -> Result<Self, AuthBuildError> {
        let mut openid_connections = Vec::new();
        for (provider, provider_config) in &config.openid {
            let connect =
                OIDCServiceBuilder::new(provider, provider_config, home_url, identity_manager, session_manager).await?;
            openid_connections.push(connect);
        }

        let external_login_cookie_builder = ExternalLoginMeta::new(cookie_secret)?
            .with_cookie_name("exl")
            .with_domain(DOMAIN_NAME);

        Ok(Self {
            home_url: home_url.to_string(),
            external_login_cookie_builder,
            openid_connections,
            session_manager: session_manager.clone(),
        })
    }

    pub fn into_router<S>(self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let state = Arc::new(ServiceState {
            home_url: self.home_url,
            session_manager: self.session_manager,
        });

        let mut router = Router::new().route("/logout", get(logout));

        for connection in self.openid_connections {
            let path = format!("/{}", connection.provider());
            let connection = connection.into_router();
            router = router.nest(&path, connection);
        }

        router
            .layer(self.external_login_cookie_builder.into_layer())
            .with_state(state)
    }
}
