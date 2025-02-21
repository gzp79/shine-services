use crate::app_state::AppState;
use axum::{extract::State, Extension};
use serde::Deserialize;
use shine_core::{
    consts::Language,
    web::{CheckedCurrentUser, IntoProblemResponse, ProblemConfig, ProblemResponse, SiteInfo, ValidatedQuery},
};
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    lang: Option<Language>,
}

/// Validate email address.
#[utoipa::path(
    post,
    path = "/api/auth/user/email/validate",
    tag = "auth",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Email validation request is sent")
    )
)]
pub async fn validate_user_email(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    ValidatedQuery(query): ValidatedQuery<QueryParams>,
    user: CheckedCurrentUser,
    site_info: SiteInfo,
) -> Result<(), ProblemResponse> {
    let ttl = state.settings().token.ttl_email_token;

    let token = state
        .token_service()
        .create_email_verify_token(user.user_id, &ttl, &site_info)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    state
        .mailer_service()
        .send_confirmation_email(&token.email, &token.token, query.lang, &user.name)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(())
}
