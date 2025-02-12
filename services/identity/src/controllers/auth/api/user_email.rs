use crate::{app_state::AppState, repositories::identity::IdentityKind};
use axum::{extract::State, Extension, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use shine_core::web::{CheckedCurrentUser, CurrentUser, Problem, ProblemConfig, ValidatedQuery};
use url::Url;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
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
) -> Result<(), Problem> {
    // get email from identity
    // send email:
    //  with EmailVerify token bound to the same user
    todo!()
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
