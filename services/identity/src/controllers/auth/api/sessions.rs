use crate::app_state::AppState;
use axum::{extract::State, Extension, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use shine_core::web::{CheckedCurrentUser, IntoProblemResponse, ProblemConfig, ProblemResponse};
use utoipa::ToSchema;
use uuid::Uuid;

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
pub struct ActiveSessions {
    sessions: Vec<ActiveSession>,
}

#[utoipa::path(
    get,
    path = "/api/auth/user/sessions",
    tag = "auth",
    responses(
        (status = OK, body = ActiveSessions)
    )
)]
pub async fn list_sessions(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
) -> Result<Json<ActiveSessions>, ProblemResponse> {
    let sessions = state
        .session_service()
        .find_all(user.user_id)
        .await
        .map_err(|err| err.into_response(&problem_config))?
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
    Ok(Json(ActiveSessions { sessions }))
}
