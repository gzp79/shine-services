use crate::{
    controllers::{
        auth::{AuthError, AuthPage, AuthSession, PageUtils},
        ApiKind, AppState,
    },
    repositories::identity::TokenKind,
};
use axum::extract::State;
use serde::Deserialize;
use shine_core::axum::{ApiEndpoint, ApiMethod, ConfiguredProblem, InputError, ValidatedQuery};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct QueryParams {
    terminate_all: Option<bool>,
    #[param(value_type=Option<String>)]
    redirect_url: Option<Url>,
    #[param(value_type=Option<String>)]
    error_url: Option<Url>,
}

async fn logout(
    State(state): State<AppState>,
    mut auth_session: AuthSession,
    query: Result<ValidatedQuery<QueryParams>, ConfiguredProblem<InputError>>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, AuthError::InputError(error.problem), None),
    };

    if let Some((user_id, session_key)) = auth_session.user_session.as_ref().map(|u| (u.user_id, u.key)) {
        match query.terminate_all.unwrap_or(false) {
            true => {
                log::debug!("Removing all the (non-api-key) tokens for user {}", user_id);
                //remove all non-api-key tokens
                if let Err(err) = state
                    .identity_service()
                    .delete_all_tokens_by_user(user_id, &[TokenKind::Access, TokenKind::SingleAccess])
                    .await
                {
                    return PageUtils::new(&state).internal_error(auth_session, err, query.error_url.as_ref());
                }

                log::debug!("Removing all the session for user {}", user_id);
                if let Err(err) = state.session_service().remove_all(user_id).await {
                    log::warn!("Failed to clear all sessions for user {}: {:?}", user_id, err);
                }
            }
            false => {
                log::debug!("Removing remember me token for user, if cookie is present {}", user_id);
                if let Some(token) = auth_session.token_cookie.as_ref().map(|t| t.key.clone()) {
                    log::debug!("Removing token {} for user {}", token, user_id);
                    if let Err(err) = state.identity_service().delete_token(user_id, &token).await {
                        return PageUtils::new(&state).internal_error(auth_session, err, query.error_url.as_ref());
                    }
                }

                log::debug!("Removing session for user {}", user_id);
                if let Err(err) = state.session_service().remove(user_id, &session_key).await {
                    log::warn!("Failed to clear session for user {}: {:?}", user_id, err);
                }
            }
        };
    }

    auth_session.clear();
    PageUtils::new(&state).redirect(auth_session, None, query.redirect_url.as_ref())
}

pub fn page_logout() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Page("/auth/logout"), logout)
        .with_operation_id("page_logout")
        .with_tag("page")
        .with_query_parameter::<QueryParams>()
        .with_page_response("Html page to update clear client cookies and complete user logout")
}
