use crate::{openapi::ApiKind, services::IdentityServiceState};
use axum::{body::HttpBody, extract::State, http::StatusCode, BoxError, Json};
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
    state
        .require_permission(&user, crate::db::Permission::UpdateTrace)
        .await?;
    log::trace!("config: {:#?}", format);
    state
        .tracing_manager()
        .reconfigure(format.filter)
        .map_err(Problem::internal_error_from)?;

    Ok(())
}

pub fn ep_tracing_reconfigure<B>() -> ApiEndpoint<IdentityServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Put, ApiKind::Api("/tracing/config"), reconfigure)
        .with_operation_id("ep_trace_config")
        .with_tag("status")
        .with_json_request::<Request>()
        .with_status_response(StatusCode::OK, "Configuration is updated")
}
