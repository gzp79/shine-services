use crate::{
    app_state::AppState,
    models::{IdentityKind, SearchIdentity, MAX_SEARCH_RESULT_COUNT},
    services::{permissions, IdentityPermissions},
};
use axum::{extract::State, Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_infra::{
    session::CheckedCurrentUser,
    web::{
        extracts::ValidatedQuery,
        responses::{IntoProblemResponse, ProblemConfig, ProblemResponse},
    },
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    /// Maximum number of items to return (server hard cap: MAX_SEARCH_RESULT_COUNT)
    #[validate(range(min = 1, max = "MAX_SEARCH_RESULT_COUNT"))]
    count: Option<usize>,

    /// Comma-separated user UUIDs (exact match, OR within list)
    #[serde(default, deserialize_with = "shine_infra::serde::deserialize_optional_comma_list")]
    user_id: Option<Vec<Uuid>>,

    /// Comma-separated emails (exact match, OR within list)
    #[serde(default, deserialize_with = "shine_infra::serde::deserialize_optional_comma_list")]
    email: Option<Vec<String>>,

    /// Comma-separated name fragments (case-insensitive contains, OR within list)
    #[serde(default, deserialize_with = "shine_infra::serde::deserialize_optional_comma_list")]
    name: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct IdentityInfo {
    id: Uuid,
    kind: String,
    name: String,
    email: Option<String>,
    is_email_confirmed: bool,
    creation: DateTime<Utc>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdentitySearchPage {
    identities: Vec<IdentityInfo>,
    is_partial: bool,
}

#[utoipa::path(
    get,
    path = "/api/identities",
    tag = "identity",
    params(QueryParams),
    responses(
        (status = OK, body = IdentitySearchPage)
    )
)]
pub async fn search_identity(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    ValidatedQuery(query): ValidatedQuery<QueryParams>,
    user: CheckedCurrentUser,
) -> Result<Json<IdentitySearchPage>, ProblemResponse> {
    user.identity_permissions()
        .check(permissions::READ_ANY_IDENTITY)
        .map_err(|err| err.into_response(&problem_config))?;

    let count = query
        .count
        .unwrap_or(MAX_SEARCH_RESULT_COUNT)
        .min(MAX_SEARCH_RESULT_COUNT);

    // Fetch one extra to detect whether results were truncated
    let mut identities = state
        .user_service()
        .search(SearchIdentity {
            count: Some(count + 1),
            user_ids: query.user_id.as_deref(),
            emails: query.email.as_deref(),
            names: query.name.as_deref(),
        })
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    let is_partial = identities.len() > count;
    identities.truncate(count);

    let identities = identities
        .into_iter()
        .map(|x| IdentityInfo {
            id: x.id,
            name: x.name,
            kind: match x.kind {
                IdentityKind::User => "user".to_string(),
                IdentityKind::Studio => "studio".to_string(),
            },
            email: x.email.map(|e| e.as_str().to_string()),
            is_email_confirmed: x.is_email_confirmed,
            creation: x.created,
        })
        .collect();

    Ok(Json(IdentitySearchPage { identities, is_partial }))
}
