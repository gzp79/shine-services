use crate::{app_state::AppState, controllers::auth::AuthSession};
use axum::extract::State;
use serde::Deserialize;
use shine_core::web::{
    ClientFingerprint, ConfiguredProblem, CurrentUser, InputError, Problem, SiteInfo, ValidatedQuery,
};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    #[validate(email)]
    email: String,
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
    ValidatedQuery(query): ValidatedQuery<QueryParams>,
    auth_session: AuthSession,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
) -> Result<(), Problem> {
    // create a user with an unconfirmed email address
    // send emailVerification (?token=[token]&email_hash=[email_hash]) to the provided email
    todo!()
}
