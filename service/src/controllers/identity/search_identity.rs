use crate::{
    controllers::{ApiKind, AppState},
    repositories::identity::{SearchIdentity, SearchIdentityOrder, MAX_SEARCH_RESULT_COUNT},
    services::Permission,
};
use axum::{extract::State, http::StatusCode, Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem, ProblemConfig, ValidatedQuery},
    service::CheckedCurrentUser,
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
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
struct IdentitySearchPage {
    identities: Vec<IdentityInfo>,
}

async fn search_identity(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    ValidatedQuery(query): ValidatedQuery<Query>,
    user: CheckedCurrentUser,
) -> Result<Json<IdentitySearchPage>, Problem> {
    state.check_permission(&user, Permission::ReadAnyIdentity).await?;

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
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to find identities", err))?;
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

pub fn ep_search_identity() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/identities"), search_identity)
        .with_operation_id("search_identity")
        .with_tag("identity")
        .with_query_parameter::<Query>()
        .with_schema::<IdentityInfo>()
        .with_json_response::<IdentitySearchPage>(StatusCode::OK)
}
