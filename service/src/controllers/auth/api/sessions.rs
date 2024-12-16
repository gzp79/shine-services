use axum::{extract::State, http::StatusCode, Extension, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem, ProblemConfig},
    service::CheckedCurrentUser,
};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::controllers::{ApiKind, AppState};

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSession {
    user_id: Uuid,
    fingerprint: String,
    token_hash: String,
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

async fn list_sessions(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
) -> Result<Json<Response>, Problem> {
    let sessions = state
        .session_service()
        .find_all(user.user_id)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "DBError", err))?
        .into_iter()
        .map(|s| ActiveSession {
            user_id: user.user_id,
            fingerprint: s.fingerprint,
            token_hash: s.key_hash,
            created_at: s.created_at,
            agent: s.site_info.agent,
            country: s.site_info.country,
            region: s.site_info.region,
            city: s.site_info.city,
        })
        .collect();
    Ok(Json(Response { sessions }))
}

pub fn ep_list_sessions() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/sessions"), list_sessions)
        .with_operation_id("list_sessions")
        .with_tag("auth")
        .with_schema::<ActiveSession>()
        .with_json_response::<Response>(StatusCode::OK)
}
