use crate::{
    app_state::AppState,
    controllers::auth::{AuthError, AuthSession},
};
use axum::{extract::State, Extension};
use serde::Deserialize;
use shine_core::web::{
    ClientFingerprint, IntoProblemResponse, Problem, ProblemConfig, ProblemResponse, SiteInfo, ValidatedQuery,
};
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    #[validate(email)]
    email: String,
    captcha: String,
}

/// Email login and registration.
/// Send an email with a verification token to the provided email address.
/// If email is not found in the database, a new user is created with an unconfirmed email address.
#[utoipa::path(
    get,
    path = "/auth/email/login",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Email with verification token is sent")
    )
)]
pub async fn email_login(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    ValidatedQuery(query): ValidatedQuery<QueryParams>,
    auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
) -> Result<(), ProblemResponse> {
    state
        .captcha_validator()
        .validate(Some(&query.captcha))
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    if auth_session.user_session().is_some() {
        return Err(AuthError::LogoutRequired.into_response(&problem_config));
    }

    state
        .mailer_service()
        .send_confirmation_email(&query.email, "token")
        .await
        .map_err(|err| err.into_response(&problem_config))?;
    // create a user with an unconfirmed email address
    // send emailVerification (?token=[token]&email_hash=[email_hash]) to the provided email
    Ok(())
}
