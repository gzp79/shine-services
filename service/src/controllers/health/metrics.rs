use crate::controllers::{ApiKind, AppState};
use axum::{extract::State, http::StatusCode};
use shine_service::axum::{ApiEndpoint, ApiMethod};

async fn get_metrics(State(state): State<AppState>) -> String {
    state.telemetry_service().metrics()
}

pub fn ep_get_metrics() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/telemetry/metrics"), get_metrics)
        .with_operation_id("get_metrics")
        .with_tag("health")
        .with_status_response(StatusCode::OK, "System metrics.")
}
