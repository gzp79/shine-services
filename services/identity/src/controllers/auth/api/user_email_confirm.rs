use crate::{app_state::AppState, repositories::identity::IdentityError, services::SignedTokenServiceError};
use axum::{extract::State, Extension};
use serde::Deserialize;
use shine_core::{
    consts::Language,
    web::{CheckedCurrentUser, IntoProblemResponse, ProblemConfig, ProblemResponse, ValidatedQuery},
};
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmQueryParams {
    lang: Option<Language>,
}

/// Start email address confirmation flow.
#[utoipa::path(
    post,
    path = "/api/auth/user/email/confirm",
    tag = "auth",
    params(
        ConfirmQueryParams
    ),
    responses(
        (status = OK, description="Start email confirmation")
    )
)]
pub async fn start_user_email_validation(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    ValidatedQuery(query): ValidatedQuery<ConfirmQueryParams>,
    user: CheckedCurrentUser,
) -> Result<(), ProblemResponse> {
    let ttl = state.settings().token.ttl_email_token;

    let email = state
        .identity_service()
        .find_by_id(user.user_id)
        .await
        .map_err(|err| err.into_response(&problem_config))?
        .ok_or(IdentityError::UserDeleted { id: user.user_id }.into_response(&problem_config))?
        .email
        .ok_or(IdentityError::MissingEmail.into_response(&problem_config))?;

    let token = state
        .signed_token_service()
        .create_email_verify_token(user.user_id, &ttl, &email)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    state
        .mailer_service()
        .send_confirmation_email(&email, &token.token, query.lang, &user.name)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(())
}

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CompleteQueryParams {
    token: String,
}

/// Complete email address confirmation and change flow.
#[utoipa::path(
    post,
    path = "/api/auth/user/email/complete",
    tag = "auth",
    params(
        CompleteQueryParams
    ),
    responses(
        (status = OK, description="Complete email operation")
    )
)]
pub async fn complete_user_email_operation(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    ValidatedQuery(query): ValidatedQuery<CompleteQueryParams>,
    user: CheckedCurrentUser,
) -> Result<(), ProblemResponse> {
    let email = state
        .identity_service()
        .find_by_id(user.user_id)
        .await
        .map_err(|err| err.into_response(&problem_config))?
        .ok_or(IdentityError::UserDeleted { id: user.user_id }.into_response(&problem_config))?
        .email
        .ok_or(SignedTokenServiceError::TokenExpired.into_response(&problem_config))?;

    state
        .signed_token_service()
        .check_email_verify_token(user.user_id, &email, &query.token)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    state
        .identity_service()
        .update(user.user_id, None, Some((&email, true)))
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(())
}
