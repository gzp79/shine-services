use crate::{auth::AuthServiceState, openapi::ApiKind, repositories::ExternalLink};
use axum::{extract::State, http::StatusCode, Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem, ProblemConfig, ProblemDetail, ValidatedPath},
    service::CheckedCurrentUser,
};
use std::sync::Arc;
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

async fn external_link_list(
    State(state): State<AuthServiceState>,
    Extension(problem_config): Extension<Arc<ProblemConfig>>,
    user: CheckedCurrentUser,
) -> Result<Json<LinkedExternalProviders>, ProblemDetail> {
    let links = state
        .identity_manager()
        .list_links(user.user_id)
        .await
        .map_err(|err| ProblemDetail::from(&problem_config, Problem::internal_error_from(err)))?
        .into_iter()
        .map(LinkedExternalProvider::from)
        .collect();
    Ok(Json(LinkedExternalProviders { links }))
}

pub fn ep_external_link_list() -> ApiEndpoint<AuthServiceState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/links"), external_link_list)
        .with_operation_id("external_link_list")
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

async fn external_link_delete(
    State(state): State<AuthServiceState>,
    Extension(problem_config): Extension<Arc<ProblemConfig>>,
    user: CheckedCurrentUser,
    ValidatedPath(params): ValidatedPath<ProviderSelectPathParam>,
) -> Result<(), ProblemDetail> {
    let link = state
        .identity_manager()
        .unlink_user(user.user_id, &params.provider, &params.provider_id)
        .await
        .map_err(|err| ProblemDetail::from(&problem_config, Problem::internal_error_from(err)))?;

    if link.is_none() {
        Err(ProblemDetail::from(
            &problem_config,
            Problem::not_found().with_instance(format!(
                "{{auth_api}}/user/links/{}/{}",
                params.provider, params.provider_id
            )),
        ))
    } else {
        Ok(())
    }
}

pub fn ep_external_link_delete() -> ApiEndpoint<AuthServiceState> {
    ApiEndpoint::new(
        ApiMethod::Delete,
        ApiKind::Api("/auth/user/links/:provider/:providerId"),
        external_link_delete,
    )
    .with_operation_id("external_link_delete")
    .with_tag("auth")
    .with_path_parameter::<ProviderSelectPathParam>()
    .with_status_response(StatusCode::OK, "Token revoked")
}
