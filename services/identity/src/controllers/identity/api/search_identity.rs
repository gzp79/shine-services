use crate::{
    app_state::AppState,
    repositories::identity::{SearchIdentity, SearchIdentityOrder, MAX_SEARCH_RESULT_COUNT},
    services::{permissions, IdentityPermissions},
};
use axum::{extract::State, Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_infra::web::{CheckedCurrentUser, IntoProblemResponse, ProblemConfig, ProblemResponse, ValidatedQuery};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    /// The maximum number of items returned in a single response
    #[validate(range(min = 1, max = "MAX_SEARCH_RESULT_COUNT"))]
    count: Option<usize>,
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
}

#[utoipa::path(
    get,
    path = "/api/identities",
    tag = "identity",
    params(
        QueryParams
    ),
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

    let identities = state
        .identity_service()
        .search(SearchIdentity {
            order: SearchIdentityOrder::UserId(None),
            count: query.count,
            user_ids: None,
            emails: None,
            names: None,
        })
        .await
        .map_err(|err| err.into_response(&problem_config))?;
    log::info!("identities: {:?}", identities);
    todo!()
    /* let identities = identities
        .into_iter()
        .map(|x| IdentityInfo {
            id: x.id,
            name: x.name,
            kind: match x.kind {
                IdentityKind::User => "user".to_string(),
                IdentityKind::Studio => "studio".to_string(),
            },
            email: x.email,
            is_email_confirmed: x.is_email_confirmed,
            creation: x.created,
        })
        .collect();

    Ok(Json(IdentitySearchPage { identities }))*/
}
