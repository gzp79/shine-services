use crate::services::IdentityServiceState;
use axum::{extract::State, Json};
use bb8::State as BB8PoolState;
use serde::Serialize;
use serde_json::{json, Value};

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

pub(in crate::services) async fn status(State(state): State<IdentityServiceState>) -> Json<Value> {
    let json = json!
    ( {
        "postgres": DBState::from(state.db.postgres.state()),
        "redis": DBState::from(state.db.redis.state())
    });

    Json(json)
}
