use crate::{app_state::AppState, repositories::identity::IdentityKind};
use axum::{extract::State, Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_infra::web::{
    CheckedCurrentUser, CurrentUser, IntoProblemResponse, Problem, ProblemConfig, ProblemResponse, ValidatedQuery,
};
use url::Url;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    refresh: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurrentUserInfo {
    user_id: Uuid,
    kind: IdentityKind,
    name: String,
    created_at: DateTime<Utc>,
    email: Option<String>,
    is_email_confirmed: bool,
    is_linked: bool,
    session_length: u64,
    roles: Vec<String>,
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
    //todo: move it into some handler

    // find extra information not present in the session data
    let identity = state
        .identity_service()
        .find_by_id(user.user_id)
        .await
        .map_err(|err| err.into_response(&problem_config))?
        //in the very unlikely case, when the identity is deleted just after session validation, a not found is returned.
        .ok_or_else(|| {
            Problem::not_found()
                .with_instance_str(format!("{{auth_api}}/identities/{}", user.user_id))
                .into_response(&problem_config)
        })?;

    // make sure the redis is up to date
    let user_info = if query.refresh.unwrap_or(false) {
        let roles = state
            .identity_service()
            .get_roles(user.user_id)
            .await
            .map_err(|err| err.into_response(&problem_config))?
            .ok_or_else(|| {
                Problem::not_found()
                    .with_instance_str(format!("{{auth_api}}/identities/{}", user.user_id))
                    .into_response(&problem_config)
            })?;

        let session = state
            .session_service()
            .update_user_info(&user.key, &identity, &roles)
            .await
            .map_err(|err| err.into_response(&problem_config))?
            .ok_or_else(|| {
                let url = Url::parse(&format!("{{auth_api}}/identities/{}", user.user_id)).ok();
                Problem::not_found().with_instance(url).into_response(&problem_config)
            })?;
        CurrentUser {
            user_id: session.info.user_id,
            key: user.key,
            session_start: session.info.created_at,
            fingerprint: session.info.fingerprint,
            version: session.user_version,
            name: session.user.name,
            roles: session.user.roles,
        }
    } else {
        user.into_user()
    };

    let is_linked = state
        .identity_service()
        .is_linked(user_info.user_id)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    let session_length = (Utc::now() - user_info.session_start).num_seconds();
    let session_length = if session_length < 0 { 0 } else { session_length as u64 };
    Ok(Json(CurrentUserInfo {
        user_id: user_info.user_id,
        kind: identity.kind,
        name: user_info.name,
        created_at: identity.created,
        email: identity.email,
        is_email_confirmed: identity.is_email_confirmed,
        is_linked,
        session_length,
        roles: user_info.roles,
    }))
}
