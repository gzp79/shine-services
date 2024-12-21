use axum::{extract::State, http::StatusCode, Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_core::{
    axum::{ApiEndpoint, ApiMethod, Problem, ProblemConfig, ValidatedPath},
    service::CheckedCurrentUser,
};
use url::Url;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

use crate::{
    controllers::{ApiKind, AppState},
    repositories::identity::ExternalLink,
};

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

async fn list_external_links(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
) -> Result<Json<LinkedExternalProviders>, Problem> {
    let links = state
        .identity_service()
        .list_external_links_by_user(user.user_id)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to get links", err))?
        .into_iter()
        .map(LinkedExternalProvider::from)
        .collect();
    Ok(Json(LinkedExternalProviders { links }))
}

pub fn ep_list_external_links() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/links"), list_external_links)
        .with_operation_id("list_external_links")
        .with_tag("auth")
        .with_schema::<LinkedExternalProvider>()
        .with_json_response::<LinkedExternalProviders>(StatusCode::OK)
}

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct ProviderSelectPathParam {
    provider: String,
    provider_id: String,
}

async fn delete_external_link(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    ValidatedPath(params): ValidatedPath<ProviderSelectPathParam>,
) -> Result<(), Problem> {
    let link = state
        .identity_service()
        .delete_extern_link(user.user_id, &params.provider, &params.provider_id)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to delete link", err))?;

    if link.is_none() {
        let url = Url::parse(&format!(
            "{{auth_api}}/user/links/{}/{}",
            params.provider, params.provider_id
        ))
        .ok();
        Err(Problem::not_found().with_instance(url))
    } else {
        Ok(())
    }
}

pub fn ep_delete_external_link() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(
        ApiMethod::Delete,
        ApiKind::Api("/auth/user/links/:provider/:providerId"),
        delete_external_link,
    )
    .with_operation_id("delete_external_link")
    .with_tag("auth")
    .with_path_parameter::<ProviderSelectPathParam>()
    .with_status_response(StatusCode::OK, "Token revoked")
}
