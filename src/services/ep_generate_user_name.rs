use crate::{openapi::ApiKind, services::IdentityServiceState};
use axum::{body::HttpBody, extract::State, BoxError, Json};
use serde::Serialize;
use shine_service::axum::{ApiEndpoint, ApiMethod, Problem};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    name: String,
}

async fn generate_user_name(State(state): State<IdentityServiceState>) -> Result<Json<Response>, Problem> {
    let name = state
        .name_generator()
        .generate_name()
        .await
        .map_err(Problem::internal_error_from)?;

    Ok(Json(Response { name }))
}

pub fn ep_generate_user_name<B>() -> ApiEndpoint<IdentityServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Post, ApiKind::Api("/user-name"), generate_user_name)
        .with_operation_id("ep_generate_user_name")
        .with_tag("identity")
}
