use crate::db::{DBError, IdentityManager, SearchIdentity, SearchIdentityOrder};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_service::service::CurrentUser;
use std::sync::Arc;
use thiserror::Error as ThisError;
use uuid::Uuid;

#[derive(Debug, ThisError)]
enum IdentityServiceError {
    #[error("User ({0}) not found")]
    UserNotFound(Uuid),

    #[error(transparent)]
    DBError(#[from] DBError),
}

impl IntoResponse for IdentityServiceError {
    fn into_response(self) -> Response {
        let status_code = match &self {
            IdentityServiceError::UserNotFound(_) => StatusCode::NOT_FOUND,
            IdentityServiceError::DBError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}

struct ServiceState {
    identity_manager: IdentityManager,
}

type Service = Arc<ServiceState>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInfo {
    user_id: Uuid,
    name: String,
    is_email_confirmed: bool,
    session_start: DateTime<Utc>,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the user.
async fn user_info(
    State(service): State<Service>,
    current_user: CurrentUser,
) -> Result<Json<UserInfo>, IdentityServiceError> {
    let identity = service
        .identity_manager
        .find(crate::db::FindIdentity::UserId(current_user.user_id))
        .await?
        .ok_or(IdentityServiceError::UserNotFound(current_user.user_id))?;

    Ok(Json(UserInfo {
        user_id: current_user.user_id,
        name: current_user.name,
        is_email_confirmed: identity.is_email_confirmed,
        session_start: current_user.session_start,
    }))
}

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

        Router::new()
            .route("/userinfo", get(user_info))
            .route("/", get(search_identity))
            .with_state(state)
    }
}
