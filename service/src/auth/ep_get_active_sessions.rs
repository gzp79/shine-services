use crate::{auth::AuthServiceState, openapi::ApiKind};
use axum::{body::HttpBody, extract::State, http::StatusCode, BoxError, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem},
    service::CheckedCurrentUser,
};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSession {
    created_at: DateTime<Utc>,
    agent: String,
    country: Option<String>,
    region: Option<String>,
    city: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(as=ActiveSessions)]
pub struct Response {
    sessions: Vec<ActiveSession>,
}

async fn get_active_sessions(
    State(state): State<AuthServiceState>,
    user: CheckedCurrentUser,
) -> Result<Json<Response>, Problem> {
    let sessions = state
        .session_manager()
        .find_all(user.user_id)
        .await
        .map_err(Problem::internal_error_from)?
        .into_iter()
        .map(|s| ActiveSession {
            created_at: s.created_at,
            agent: s.agent,
            country: s.country,
            region: s.region,
            city: s.city,
        })
        .collect();
    Ok(Json(Response { sessions }))
}

pub fn ep_get_active_sessions<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/sessions"), get_active_sessions)
        .with_operation_id("get_active_sessions")
        .with_tag("auth")
        .with_schema::<ActiveSession>()
        .with_json_response::<Response>(StatusCode::OK)
}
