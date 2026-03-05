use crate::{
    app_state::AppState,
    repositories::identity::TokenKind,
    routes::auth::{AuthPage, AuthPageRequest, AuthSession, PageUtils},
};
use axum::extract::State;
use serde::Deserialize;
use shine_infra::web::{
    extracts::{InputError, ValidatedQuery},
    responses::ErrorResponse,
};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    terminate_all: Option<bool>,
    #[param(value_type=Option<String>)]
    redirect_url: Option<Url>,
    #[param(value_type=Option<String>)]
    error_url: Option<Url>,
}

#[utoipa::path(
    get,
    path = "/auth/logout",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Html page to update clear client cookies and complete user logout")
    )
)]
pub async fn logout(
    State(state): State<AppState>,
    auth_session: AuthSession,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
) -> AuthPage {
    // 1. Create request helper
    let req = AuthPageRequest::new(&state, auth_session);

    // 2. Validate query
    let query = match req.validate_query(query) {
        Ok(q) => q,
        Err(page) => return page,
    };

    // 3. Validate redirect URLs
    if let Some(page) = req.validate_redirect_urls(query.redirect_url.as_ref(), query.error_url.as_ref()) {
        return page;
    }

    log::debug!("Query: {query:#?}");

    // 4. Business logic - logout
    if let Some((user_id, session_key)) = req.auth_session().user_session().map(|u| (u.user_id, u.key)) {
        match query.terminate_all.unwrap_or(false) {
            true => {
                log::debug!("Removing all the (non-api-key) tokens for user {user_id}");
                //remove all non-api-key tokens
                if let Err(err) = req
                    .state()
                    .token_service()
                    .delete_all_by_user(user_id, &[TokenKind::Access, TokenKind::SingleAccess])
                    .await
                {
                    return req.error_page(err, query.error_url.as_ref());
                }

                log::debug!("Removing all the session for user {user_id}");
                if let Err(err) = req.state().session_service().remove_all(user_id).await {
                    log::warn!("Failed to clear all sessions for user {user_id}: {err:?}");
                }
            }
            false => {
                log::debug!("Removing remember me token for user, if cookie is present {user_id}");
                if let Some(token) = req.auth_session().access().map(|t| t.key.clone()) {
                    log::debug!("Removing token {token} for user {user_id}");
                    if let Err(err) = req.state().token_service().delete(TokenKind::Access, &token).await {
                        return req.error_page(err, query.error_url.as_ref());
                    }
                }

                log::debug!("Removing session for user {user_id}");
                if let Err(err) = req.state().session_service().remove(user_id, &session_key).await {
                    log::warn!("Failed to clear session for user {user_id}: {err:?}");
                }
            }
        };
    }

    // 5. Return response
    let response_session = req.into_auth_session().cleared();
    PageUtils::new(&state).redirect(response_session, query.redirect_url.as_ref(), None)
}
