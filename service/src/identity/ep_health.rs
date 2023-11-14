use crate::{identity::IdentityServiceState, openapi::ApiKind};
use axum::{body::HttpBody, extract::State, http::StatusCode, BoxError, Json};
use bb8::State as BB8PoolState;
use serde::Serialize;
use shine_service::axum::{ApiEndpoint, ApiMethod};
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
