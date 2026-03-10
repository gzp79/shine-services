use crate::app_state::AppState;
use axum::{extract::State, Extension};
use serde::Deserialize;
use shine_infra::{
    language::Language,
    models::Email,
    web::{
        extracts::{SiteInfo, ValidatedJson, ValidatedQuery},
        responses::{IntoProblemResponse, ProblemConfig, ProblemResponse},
        session::CheckedCurrentUser,
    },
};
use utoipa::{IntoParams, ToSchema};
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
    site_info: SiteInfo,
) -> Result<(), ProblemResponse> {
    state
        .email_auth_handler()
        .start_email_confirm_flow(user.user_id, &site_info, query.lang)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(())
}

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct ChangeQueryParams {
    lang: Option<Language>,
}

#[derive(Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChangeEmailRequest {
    email: Email,
}

/// Start email address change flow.
#[utoipa::path(
    post,
    path = "/api/auth/user/email/change",
    tag = "auth",
    params(
        ChangeQueryParams
    ),
    request_body = ChangeEmailRequest,
    responses(
        (status = OK, description="Start email address change")
    )
)]
pub async fn start_user_email_change(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    ValidatedQuery(query): ValidatedQuery<ChangeQueryParams>,
    user: CheckedCurrentUser,
    site_info: SiteInfo,
    ValidatedJson(body): ValidatedJson<ChangeEmailRequest>,
) -> Result<(), ProblemResponse> {
    // Email is already validated and normalized by the Email type
    state
        .email_auth_handler()
        .start_email_change_flow(user.user_id, body.email.as_str(), &site_info, query.lang)
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
        .email_auth_handler()
        .complete_email_flow(user.user_id, &query.token)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(())
}
