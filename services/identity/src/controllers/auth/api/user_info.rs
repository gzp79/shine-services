use crate::{app_state::AppState, repositories::identity::IdentityKind};
use axum::{extract::State, Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_infra::web::{
    CheckedCurrentUser, IntoProblemResponse, Problem, ProblemConfig, ProblemResponse, ValidatedQuery,
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum GetUserInfoMode {
    Fast,
    Full,
    FullWithRefresh,
}

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    #[into_params(default = "GetUserInfoMode::Fast")]
    method: Option<GetUserInfoMode>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUserInfoDetails {
    kind: IdentityKind,
    created_at: DateTime<Utc>,
    email: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUserInfo {
    user_id: Uuid,
    name: String,
    is_email_confirmed: bool,
    is_linked: bool,
    roles: Vec<String>,
    session_length: u64,
    details: Option<CurrentUserInfoDetails>,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
#[utoipa::path(
    get,
    path = "/api/auth/user/info",
    tag = "auth",
    params(
        QueryParams
    ),
    responses(
        (status = OK, body = CurrentUserInfo)
    )
)]
pub async fn get_user_info(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    ValidatedQuery(query): ValidatedQuery<QueryParams>,
) -> Result<Json<CurrentUserInfo>, ProblemResponse> {
    let method = query.method.unwrap_or(GetUserInfoMode::Fast);

    let session_length = (user.session_end - Utc::now()).num_seconds().max(0) as u64;

    let info = match method {
        // read the user info from the session
        GetUserInfoMode::Fast => {
            let user = user.into_user();
            CurrentUserInfo {
                user_id: user.user_id,
                name: user.name,
                is_email_confirmed: user.is_email_confirmed,
                is_linked: user.is_linked,
                roles: user.roles,
                session_length,
                details: None,
            }
        }

        // read the user info from the database
        GetUserInfoMode::Full | GetUserInfoMode::FullWithRefresh => {
            let user_info = state
                .user_info_handler()
                .get_user_info(user.user_id)
                .await
                .map_err(|err| err.into_response(&problem_config))?
                .ok_or_else(|| {
                    Problem::not_found()
                        .with_instance_str(format!("{{identity_api}}/identities/{}", user.user_id))
                        .into_response(&problem_config)
                })?;

            if method == GetUserInfoMode::FullWithRefresh {
                state
                    .user_info_handler()
                    .refresh_user_session(user.user_id)
                    .await
                    .map_err(|err| err.into_response(&problem_config))?;
            }

            CurrentUserInfo {
                user_id: user_info.identity.id,
                name: user_info.identity.name,
                is_email_confirmed: user_info.identity.is_email_confirmed,
                is_linked: user_info.is_linked,
                session_length,
                roles: user_info.roles,
                details: Some(CurrentUserInfoDetails {
                    kind: user_info.identity.kind,
                    created_at: user_info.identity.created,
                    email: user_info.identity.email,
                }),
            }
        }
    };

    Ok(Json(info))
}
