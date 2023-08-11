use crate::{
    db::{IdentityKind, Permission, SearchIdentity, SearchIdentityOrder, MAX_SEARCH_COUNT},
    openapi::ApiKind,
    services::IdentityServiceState,
};
use axum::{body::HttpBody, extract::State, http::StatusCode, BoxError, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem, ValidatedQuery},
    service::CurrentUser,
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
    /// The maximum number of items returned in a single response
    #[validate(range(min = 1, max = "MAX_SEARCH_COUNT"))]
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
    State(state): State<IdentityServiceState>,
    ValidatedQuery(query): ValidatedQuery<Query>,
    user: CurrentUser,
) -> Result<Json<IdentitySearchPage>, Problem> {
    state.require_permission(&user, Permission::ReadAnyIdentity).await?;

    let identities = state
        .identity_manager()
        .search(SearchIdentity {
            order: SearchIdentityOrder::UserId(None),
            count: query.count,
            user_ids: None,
            emails: None,
            names: None,
        })
        .await
        .map_err(Problem::internal_error_from)?;
    log::info!("identities: {:?}", identities);

    let identities = identities
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

    Ok(Json(IdentitySearchPage { identities }))
}

pub fn ep_search_identity<B>() -> ApiEndpoint<IdentityServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/identities"), search_identity)
        .with_operation_id("ep_search_identity")
        .with_tag("identity")
        .with_query_parameter::<Query>()
        .with_schema::<IdentityInfo>()
        .with_json_response::<IdentitySearchPage>(StatusCode::OK)
}
