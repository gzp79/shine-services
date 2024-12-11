use crate::controllers::{ApiKind, AppState};
use axum::http::StatusCode;
use shine_service::axum::{ApiEndpoint, ApiMethod};

async fn ready() -> String {
    "Ok".into()
}

pub fn ep_get_ready() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Absolute("/info/ready"), ready)
        .with_operation_id("get_ready")
        .with_tag("health")
        .with_status_response(StatusCode::OK, "Ok.")
}
