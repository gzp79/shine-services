use crate::{
    db::{IdentityKind, Permission, SearchIdentity, SearchIdentityOrder, MAX_SEARCH_COUNT},
    services::IdentityServiceState,
};
use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{Problem, ValidatedQuery},
    service::CurrentUser,
};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate)]
#[serde(rename_all = "camelCase")]
pub(in crate::services) struct RequestQuery {
    #[validate(range(min = 1, max = "MAX_SEARCH_COUNT"))]
    count: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::services) struct IdentityInfo {
    id: Uuid,
    kind: String,
    name: String,
    email: Option<String>,
    is_email_confirmed: bool,
    creation: DateTime<Utc>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::services) struct Response {
    identities: Vec<IdentityInfo>,
}

pub(in crate::services) async fn ep_search_identity(
    State(state): State<IdentityServiceState>,
    ValidatedQuery(query): ValidatedQuery<RequestQuery>,
    user: CurrentUser,
) -> Result<Json<Response>, Problem> {
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
            creation: x.creation,
        })
        .collect();

    Ok(Json(Response { identities }))
}
