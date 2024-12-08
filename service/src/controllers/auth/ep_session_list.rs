use crate::{auth::AuthServiceState, openapi::ApiKind};
use axum::{extract::State, http::StatusCode, Extension, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem, ProblemConfig},
    service::CheckedCurrentUser,
};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSession {
    user_id: Uuid,
    fingerprint: String,
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

async fn session_list(
    State(state): State<AuthServiceState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
) -> Result<Json<Response>, Problem> {
    let sessions = state
        .session_manager()
        .find_all(user.user_id)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "DBError", err))?
        .into_iter()
        .map(|s| ActiveSession {
            user_id: user.user_id,
            fingerprint: s.fingerprint,
            created_at: s.created_at,
            agent: s.agent,
            country: s.country,
            region: s.region,
            city: s.city,
        })
        .collect();
    Ok(Json(Response { sessions }))
}

pub fn ep_session_list() -> ApiEndpoint<AuthServiceState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/sessions"), session_list)
        .with_operation_id("session_list")
        .with_tag("auth")
        .with_schema::<ActiveSession>()
        .with_json_response::<Response>(StatusCode::OK)
}
