use crate::{
    app_state::AppState,
    models::PurgeGuestsResult,
    services::{permissions, IdentityPermissions},
};
use axum::{extract::State, Extension, Json};
use chrono::Utc;
use iso8601_duration::Duration as IsoDuration;
use serde::Deserialize;
use shine_infra::{
    session::CheckedCurrentUser,
    web::{
        extracts::ValidatedQuery,
        responses::{IntoProblemResponse, Problem, ProblemConfig, ProblemResponse},
    },
};
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    /// ISO 8601 duration (e.g. `P1D`, `PT2H30M`). Guests created before now()-duration are deleted.
    /// Years and months are not supported.
    older_than: String,
    /// Maximum number of guests to delete per call. Range: 1–1000. Defaults to 500.
    #[validate(range(min = 1, max = 1000))]
    limit: Option<u32>,
}

/// Purge old guest users (no confirmed email, no external links) in batches.
///
/// Requires SuperAdmin role. Deletes at most `limit` guests per call.
/// Call repeatedly until `hasMore` is false to fully drain.
#[utoipa::path(
    delete,
    path = "/api/identities/guests",
    tag = "identity",
    params(QueryParams),
    responses(
        (status = OK, body = PurgeGuestsResult)
    )
)]
pub async fn purge_guests(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    ValidatedQuery(query): ValidatedQuery<QueryParams>,
    user: CheckedCurrentUser,
) -> Result<Json<PurgeGuestsResult>, ProblemResponse> {
    user.identity_permissions()
        .check(permissions::PURGE_GUEST_USERS)
        .map_err(|err| err.into_response(&problem_config))?;

    let iso: IsoDuration = query.older_than.parse().map_err(|_| {
        Problem::bad_request("invalid_duration")
            .with_detail(format!("Invalid ISO 8601 duration: '{}'", query.older_than))
            .into_response(&problem_config)
    })?;

    // Convert ISO 8601 duration to chrono::Duration.
    // Years and months are intentionally ignored (ambiguous length).
    let duration = chrono::Duration::days(iso.day as i64)
        + chrono::Duration::hours(iso.hour as i64)
        + chrono::Duration::minutes(iso.minute as i64)
        + chrono::Duration::seconds(iso.second as i64);

    let cutoff = Utc::now() - duration;
    let limit = query.limit.unwrap_or(500) as usize;

    let result = state
        .delete_user_handler()
        .purge_guests(cutoff, limit)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(Json(result))
}
