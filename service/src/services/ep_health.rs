use crate::{openapi::ApiKind, services::IdentityServiceState};
use axum::{body::HttpBody, extract::State, http::StatusCode, BoxError, Json};
use bb8::State as BB8PoolState;
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem},
    service::CheckedCurrentUser,
};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct DBState {
    pub connections: u32,
    pub idle_connections: u32,
}

impl From<BB8PoolState> for DBState {
    fn from(value: BB8PoolState) -> Self {
        Self {
            connections: value.connections,
            idle_connections: value.idle_connections,
        }
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct ServiceHealth {
    pub postgres: DBState,
    pub redis: DBState,
}

async fn health(State(state): State<IdentityServiceState>) -> Json<ServiceHealth> {
    Json(ServiceHealth {
        postgres: DBState::from(state.db().postgres.state()),
        redis: DBState::from(state.db().redis.state()),
    })
}

pub fn ep_health<B>() -> ApiEndpoint<IdentityServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/health"), health)
        .with_operation_id("ep_health")
        .with_tag("status")
        .with_schema::<DBState>()
        .with_json_response::<ServiceHealth>(StatusCode::OK)
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTraceConfig {
    filter: String,
}

async fn reconfigure(
    State(state): State<IdentityServiceState>,
    user: CheckedCurrentUser,
    Json(format): Json<UpdateTraceConfig>,
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
        .with_json_request::<UpdateTraceConfig>()
        .with_status_response(StatusCode::OK, "Configuration is update")
}
