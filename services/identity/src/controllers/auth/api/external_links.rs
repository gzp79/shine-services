use crate::{app_state::AppState, repositories::identity::ExternalLink};
use axum::{extract::State, Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_infra::web::{
    CheckedCurrentUser, IntoProblemResponse, Problem, ProblemConfig, ProblemResponse, ValidatedPath,
};
use url::Url;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

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

#[utoipa::path(
    get,
    path = "/api/auth/user/links",
    tag = "auth",
    responses(
        (status = OK, body = LinkedExternalProviders)
    )
)]
pub async fn list_external_links(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
) -> Result<Json<LinkedExternalProviders>, ProblemResponse> {
    let links = state
        .identity_service()
        .list_external_links_by_user(user.user_id)
        .await
        .map_err(|err| err.into_response(&problem_config))?
        .into_iter()
        .map(LinkedExternalProvider::from)
        .collect();
    Ok(Json(LinkedExternalProviders { links }))
}

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSelectPathParam {
    provider: String,
    provider_id: String,
}

#[utoipa::path(
    delete,
    path = "/api/auth/user/links/{provider}/{providerId}",
    tag = "auth",
    params(
        ProviderSelectPathParam
    ),
    responses(
        (status = OK, description = "Token revoked") 
    )
)]
pub async fn delete_external_link(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    ValidatedPath(params): ValidatedPath<ProviderSelectPathParam>,
) -> Result<(), ProblemResponse> {
    let link = state
        .identity_service()
        .delete_extern_link(user.user_id, &params.provider, &params.provider_id)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    if link.is_none() {
        let url = Url::parse(&format!(
            "{{auth_api}}/user/links/{}/{}",
            params.provider, params.provider_id
        ))
        .ok();
        Err(Problem::not_found().with_instance(url).into_response(&problem_config))
    } else {
        Ok(())
    }
}
