use crate::{
    app_session::{AppSession, ExternalLoginSession, SessionData},
    auth::{OIDCBuildError, OIDCConfig, OIDCServiceBuilder},
    db::{DBError, IdentityManager, SessionManager},
};
use axum::{
    extract::{Query, State},
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
    mut session: AppSession,
    mut external_login: ExternalLoginSession,
) -> Response {
    let session_data = session.take();
    let _ = external_login.take();

    let (status, template, context) =
        if let Err(err) = perform_logout(&service, session_data, query.terminate_all.unwrap_or(false)).await {
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
        Ok(html) => (status, session, external_login, Html(html)).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            session,
            external_login,
            format!("template error: {err:?}"),
        )
            .into_response(),
    }
}

async fn perform_logout(service: &Service, session_data: Option<SessionData>, remove_all: bool) -> Result<(), DBError> {
    if let Some(session_data) = session_data {
        if remove_all {
            service.session_manager.remove_all(session_data.user_id).await?;
        } else {
            service
                .session_manager
                .remove(session_data.user_id, session_data.key)
                .await?;
        }
    }

    Ok(())
}

#[derive(Debug, ThisError)]
pub enum AuthBuildError {
    #[error(transparent)]
    OIDCError(#[from] OIDCBuildError),
}

pub struct AuthServiceBuilder {
    home_url: String,
    openid_connections: Vec<OIDCServiceBuilder>,
    //todo:  user_query: IdentityServiceBuilder, - find/fix existing users
    session_manager: SessionManager,
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

        router.with_state(state)
    }
}
