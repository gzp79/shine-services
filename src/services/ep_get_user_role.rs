use crate::{db::Permission, openapi::ApiKind, services::IdentityServiceState};
use axum::{body::HttpBody, extract::State, http::StatusCode, BoxError, Json};
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem, ValidatedPath},
    service::CurrentUser,
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct RequestPath {
    #[serde(rename = "id")]
    user_id: Uuid,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(example = json!({
    "roles": ["Role1", "Role2"]
}))]
pub struct Response {
    roles: Vec<String>,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
async fn get_user_roles(
    State(state): State<IdentityServiceState>,
    user: CurrentUser,
    ValidatedPath(path): ValidatedPath<RequestPath>,
) -> Result<Json<Response>, Problem> {
    state.require_permission(&user, Permission::ReadAnyUserRole).await?;
    let roles = state
        .identity_manager()
        .get_roles(path.user_id)
        .await
        .map_err(Problem::internal_error_from)?;
    Ok(Json(Response { roles }))
}

pub fn ep_get_user_roles<B>() -> ApiEndpoint<IdentityServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    log::info!("schema: {:?}", Response::schema().0);

    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/identities/:id/roles"), get_user_roles)
        .with_operation_id("ep_get_user_roles")
        .with_tag("identity")
        .with_parameters(RequestPath::into_params(|| None))
    .with_json_response::<Response>(StatusCode::OK)
}
