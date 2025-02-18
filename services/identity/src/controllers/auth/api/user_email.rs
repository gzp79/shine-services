use crate::app_state::AppState;
use axum::{extract::State, Extension};
use serde::Deserialize;
use shine_core::web::{
    CheckedCurrentUser, IntoProblemResponse, Problem, ProblemConfig, ProblemResponse, SiteInfo, ValidatedQuery,
};
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    #[validate(email)]
    email: String,
}

/// Validate email address.
#[utoipa::path(
    post,
    path = "/api/auth/user/email/validate",
    tag = "auth",
    responses(
        (status = OK, description="Email validation request is sent")
    )
)]
pub async fn validate_user_email(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    site_info: SiteInfo,
) -> Result<(), ProblemResponse> {
    let ttl = state.settings().token.ttl_email_token;

    let token = state
        .token_service()
        .create_email_token(user.user_id, &ttl, &site_info)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    state
        .mailer_service()
        .send_confirmation_email(&token.email, &token.token)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(())
}

/// Change email address.
#[utoipa::path(
    post,
    path = "/api/auth/user/email",
    tag = "auth",
    responses(
        (status = OK, description="Email validation request is sent")
    )
)]
pub async fn change_user_email(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    ValidatedQuery(query): ValidatedQuery<QueryParams>,
    user: CheckedCurrentUser,
) -> Result<(), Problem> {
    // get email from identity
    // check if new email is already in use
    // send email:
    //  send email about changed request (passive, no link, for this the credentials had to be stolen, no our issue)
    //  with EmailChange token bound to the new email
    todo!()
}

/// Delete email address.
#[utoipa::path(
    delete,
    path = "/api/auth/user/email",
    tag = "auth",
    responses(
        (status = OK, description="Email validation request is sent")
    )
)]
pub async fn delete_user_email(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    ValidatedQuery(query): ValidatedQuery<QueryParams>,
    user: CheckedCurrentUser,
) -> Result<(), Problem> {
    // get email from identity
    // send email:
    //  send email to change email to empty
    todo!()
}
