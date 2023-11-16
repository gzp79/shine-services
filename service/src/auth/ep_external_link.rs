use crate::{auth::AuthServiceState, openapi::ApiKind, repositories::ExternalLink};
use axum::{body::HttpBody, extract::State, http::StatusCode, BoxError, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem},
    service::CheckedCurrentUser,
};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LinkedExternalProvider {
    pub user_id: Uuid,
    pub provider: String,
    pub provider_user_id: String,
    pub linked_at: DateTime<Utc>,
    pub name: Option<String>,
    pub email: Option<String>,
}

impl From<ExternalLink> for LinkedExternalProvider {
    fn from(link: ExternalLink) -> Self {
        Self {
            user_id: link.user_id,
            provider: link.provider,
            provider_user_id: link.provider_id,
            linked_at: link.linked_at,
            name: link.name,
            email: link.email,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LinkedExternalProviders {
    links: Vec<LinkedExternalProvider>,
}

async fn external_link_list(
    State(state): State<AuthServiceState>,
    user: CheckedCurrentUser,
) -> Result<Json<LinkedExternalProviders>, Problem> {
    let links = state
        .identity_manager()
        .list_find_links(user.user_id)
        .await
        .map_err(Problem::internal_error_from)?
        .into_iter()
        .map(LinkedExternalProvider::from)
        .collect();
    Ok(Json(LinkedExternalProviders { links }))
}

pub fn ep_external_link_list<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/links"), external_link_list)
        .with_operation_id("external_link_list")
        .with_tag("auth")
        .with_schema::<LinkedExternalProvider>()
        .with_json_response::<LinkedExternalProviders>(StatusCode::OK)
}
