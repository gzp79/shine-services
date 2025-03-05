use crate::app_state::AppState;
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
    state
        .email_token_handler()
        .start_email_confirm_flow(user.user_id, query.lang)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(())
}

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ChangeQueryParams {
    #[validate(email)]
    email: String,
    lang: Option<Language>,
}

/// Start email address change flow.
#[utoipa::path(
    post,
    path = "/api/auth/user/email/change",
    tag = "auth",
    params(
        ChangeQueryParams
    ),
    responses(
        (status = OK, description="Start email address change")
    )
)]
pub async fn start_user_email_change(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    ValidatedQuery(query): ValidatedQuery<ChangeQueryParams>,
    user: CheckedCurrentUser,
) -> Result<(), ProblemResponse> {
    state
        .email_token_handler()
        .start_email_change_flow(user.user_id, &query.email, query.lang)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(())
}

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct CompleteQueryParams {
    token: String,
}

/// Complete email address operation flow.
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
    state
        .email_token_handler()
        .complete_email_flow(user.user_id, &query.token)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(())
}
