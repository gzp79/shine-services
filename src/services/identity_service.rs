use crate::{
    app_session::AppSession,
    db::{IdentityManager, SearchIdentity, SearchIdentityOrder},
};
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use std::sync::Arc;

struct ServiceState {
    identity_manager: IdentityManager,
}

type Service = Arc<ServiceState>;

#[derive(Deserialize)]
struct SearchIdentityRequest {
    count: Option<usize>,
}

async fn search_identity(
    State(service): State<Service>,
    Query(query): Query<SearchIdentityRequest>,
    //session: AppSession,
) -> Response {
    //let session_data = session.g();
    let identities = service
        .identity_manager
        .search(SearchIdentity {
            order: SearchIdentityOrder::UserId(None),
            count: query.count,
            user_ids: None,
            emails: None,
            names: None,
        })
        .await;
    log::info!("identities: {:?}", identities);

    ().into_response()
}

pub struct IdentityServiceBuilder {
    identity_manager: IdentityManager,
}

impl IdentityServiceBuilder {
    pub fn new(identity_manager: &IdentityManager) -> Self {
        Self {
            identity_manager: identity_manager.clone(),
        }
    }

    pub fn into_router<S>(self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let state = Arc::new(ServiceState {
            identity_manager: self.identity_manager,
        });

        Router::new().route("/", get(search_identity)).with_state(state)
    }
}
