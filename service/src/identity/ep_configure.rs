use crate::{identity::IdentityServiceState, openapi::ApiKind, repositories::Permission};
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem},
    service::CheckedCurrentUser,
};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
#[schema(as=UpdateTraceConfig)]
pub struct Request {
    filter: String,
}

async fn reconfigure(
    State(state): State<IdentityServiceState>,
    user: CheckedCurrentUser,
    Json(format): Json<Request>,
) -> Result<(), Problem> {
    state.require_permission(&user, Permission::UpdateTrace).await?;
    log::trace!("config: {:#?}", format);
    state
        .tracing_manager()
        .reconfigure(format.filter)
        .map_err(Problem::internal_error_from)?;

    Ok(())
}

pub fn ep_tracing_reconfigure() -> ApiEndpoint<IdentityServiceState> {
    ApiEndpoint::new(ApiMethod::Put, ApiKind::Api("/tracing/config"), reconfigure)
        .with_operation_id("ep_trace_config")
        .with_tag("status")
        .with_json_request::<Request>()
        .with_status_response(StatusCode::OK, "Configuration is updated")
}
