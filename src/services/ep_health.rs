use crate::{openapi::ApiKind, services::IdentityServiceState};
use axum::{body::HttpBody, extract::State, BoxError, Json};
use bb8::State as BB8PoolState;
use serde::Serialize;
use serde_json::{json, Value};
use shine_service::axum::{ApiEndpoint, ApiMethod};

#[derive(Serialize)]
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

async fn health(State(state): State<IdentityServiceState>) -> Json<Value> {
    let json = json!
    ( {
        "postgres": DBState::from(state.db().postgres.state()),
        "redis": DBState::from(state.db().redis.state())
    });

    Json(json)
}

pub fn ep_health<B>() -> ApiEndpoint<IdentityServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Put, ApiKind::Api("/health"), health)
        .with_operation_id("ep_health")
        .with_tag("status")
}
