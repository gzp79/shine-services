use crate::{
    app_session::{AppSession, ExternalLoginSession},
    auth::{OIDCBuildError, OIDCConfig, OIDCServiceBuilder},
    db::{IdentityManager, SessionManager},
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Extension, Router,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tera::Tera;
use thiserror::Error as ThisError;
use url::Url;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub openid: HashMap<String, OIDCConfig>,
}

#[derive(Debug, ThisError)]
enum AuthError {
    #[error(transparent)]
    TeraError(#[from] tera::Error),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            AuthError::TeraError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}

struct ServiceState {
    home_url: String,
}

type Service = Arc<ServiceState>;

// reset all the
async fn logout(
    Extension(tera): Extension<Arc<Tera>>,
    State(service): State<Service>,
    mut session: AppSession,
    mut external_login: ExternalLoginSession,
) -> Result<Response, AuthError> {
    let _ = session.take();
    let _ = external_login.take();

    let mut context = tera::Context::new();
    context.insert("title", &"Logout");
    context.insert("target", &"home");
    context.insert("redirect_url", &service.home_url.to_string());
    let html = Html(tera.render("redirect.html", &context)?);

    Ok((session, external_login, html).into_response())
}

#[derive(Debug, ThisError)]
pub enum AuthBuildError {
    #[error(transparent)]
    OIDCError(#[from] OIDCBuildError),
}

pub struct AuthServiceBuilder {
    home_url: String,
    openid_connections: Vec<OIDCServiceBuilder>,
    //identity_manager: IdentityManager,
}

impl AuthServiceBuilder {
    pub async fn new(
        config: &Config,
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

        Ok(Self {
            home_url: home_url.to_string(),
            openid_connections,
            //identity_manager,
        })
    }

    pub fn into_router<S>(self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let state = Arc::new(ServiceState {
            home_url: self.home_url,
        });

        let mut router = Router::new().route("/logout", get(logout));

        for connection in self.openid_connections {
            let path = format!("/{}", connection.provider());
            let connection = connection.into_router();
            router = router.nest(&path, connection);
        }

        router.with_state(state)
    }
}
