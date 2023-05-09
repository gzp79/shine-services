use std::collections::HashMap;
use crate::{
    app_error::AppError,
    db::IdentityManager,
    oauth::{OpenIdConnect, OpenIdConnectConfig},
};
use axum::Router;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthConfig {
    pub openid: HashMap<String, OpenIdConnectConfig>,
}

pub struct OAuthConnections {
    openid_connections: Vec<OpenIdConnect>,
    //identity_manager: IdentityManager,
}

impl OAuthConnections {
    pub async fn new(config: &OAuthConfig, identity_manager: IdentityManager) -> Result<OAuthConnections, AppError> {
        let mut openid_connections = Vec::new();
        for (provider, provider_config) in &config.openid {
            let connect = OpenIdConnect::new(provider, provider_config, identity_manager.clone()).await?;
            openid_connections.push(connect);
        }

        Ok(Self {
            openid_connections,
            //identity_manager,
        })
    }

    pub fn into_router<S>(self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let mut router = Router::new();

        for connection in self.openid_connections {
            let path = format!("/{}", connection.provider());
            let connection = connection.into_router();
            router = router.nest(&path, connection);
        }

        router
    }
}
