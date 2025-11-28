use crate::{
    app_state::AppState,
    controllers::auth::{AuthPage, AuthSession, AuthUtils, PageUtils},
    repositories::identity::TokenKind,
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
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, error.problem, None, None),
    };
    if let Some(error_url) = &query.error_url {
        if let Err(err) = AuthUtils::new(&state).validate_redirect_url("errorUrl", error_url) {
            return PageUtils::new(&state).error(auth_session, err, None, None);
        }
    }
    if let Some(redirect_url) = &query.redirect_url {
        if let Err(err) = AuthUtils::new(&state).validate_redirect_url("redirectUrl", redirect_url) {
            return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref(), None);
        }
    }

    log::debug!("Query: {query:#?}");

    if let Some((user_id, session_key)) = auth_session.user_session().map(|u| (u.user_id, u.key)) {
        match query.terminate_all.unwrap_or(false) {
            true => {
                log::debug!("Removing all the (non-api-key) tokens for user {user_id}");
                //remove all non-api-key tokens
                if let Err(err) = state
                    .identity_service()
                    .delete_all_tokens_by_user(user_id, &[TokenKind::Access, TokenKind::SingleAccess])
                    .await
                {
                    return PageUtils::new(&state).error(
                        auth_session,
                        err,
                        query.error_url.as_ref(),
                        query.redirect_url.as_ref(),
                    );
                }

                log::debug!("Removing all the session for user {user_id}");
                if let Err(err) = state.session_service().remove_all(user_id).await {
                    log::warn!("Failed to clear all sessions for user {user_id}: {err:?}");
                }
            }
            false => {
                log::debug!("Removing remember me token for user, if cookie is present {user_id}");
                if let Some(token) = auth_session.access().map(|t| t.key.clone()) {
                    log::debug!("Removing token {token} for user {user_id}");
                    if let Err(err) = state.identity_service().delete_token(TokenKind::Access, &token).await {
                        return PageUtils::new(&state).error(
                            auth_session,
                            err,
                            query.error_url.as_ref(),
                            query.redirect_url.as_ref(),
                        );
                    }
                }

                log::debug!("Removing session for user {user_id}");
                if let Err(err) = state.session_service().remove(user_id, &session_key).await {
                    log::warn!("Failed to clear session for user {user_id}: {err:?}");
                }
            }
        };
    }

    let response_session = auth_session.cleared();
    PageUtils::new(&state).redirect(response_session, query.redirect_url.as_ref(), None)
}
