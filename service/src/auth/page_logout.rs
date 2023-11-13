use crate::{
    auth::{AuthError, AuthPage, AuthServiceState, AuthSession},
    openapi::ApiKind,
    repositories::TokenKind,
};
use axum::{body::HttpBody, extract::State};
use serde::Deserialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, ValidatedQuery, ValidationError},
    service::APP_NAME,
};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct Query {
    terminate_all: Option<bool>,
    redirect_url: Option<Url>,
    error_url: Option<Url>,
}

async fn logout(
    State(state): State<AuthServiceState>,
    mut auth_session: AuthSession,
    query: Result<ValidatedQuery<Query>, ValidationError>,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return state.page_error(auth_session, AuthError::ValidationError(error), None),
    };

    if let Some((user_id, user_key)) = auth_session.user.as_ref().map(|u| (u.user_id, u.key)) {
        match query.terminate_all.unwrap_or(false) {
            true => {
                //remove all non-api-key tokens
                if let Err(err) = state
                    .identity_manager()
                    .delete_all_tokens(user_id, &[TokenKind::AutoRenewal, TokenKind::SingleAccess])
                    .await
                {
                    return state.page_internal_error(auth_session, err, query.error_url.as_ref());
                }

                // from this point there is no reason to keep session
                // errors beyond these points are irrelevant for the users and mostly just warnings.
                auth_session.clear();
                if let Err(err) = state.session_manager().remove_all(user_id).await {
                    log::warn!("Failed to clear all sessions for user {}: {:?}", user_id, err);
                }
            }
            false => {
                if let Some(token) = auth_session.token_login.as_ref().map(|t| t.token.clone()) {
                    if let Err(err) = state.identity_manager().delete_token(user_id, &token).await {
                        return state.page_internal_error(auth_session, err, query.error_url.as_ref());
                    }
                }

                // from this point there is no reason to keep session
                // errors beyond these points are irrelevant for the users and mostly just warnings.
                auth_session.clear();
                if let Err(err) = state.session_manager().remove(user_id, user_key).await {
                    log::warn!("Failed to clear session for user {}: {:?}", user_id, err);
                }
            }
        };
    }

    state.page_redirect(auth_session, APP_NAME, query.redirect_url.as_ref())
}

pub fn page_logout<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Page("/auth/logout"), logout)
        .with_operation_id("page_logout")
        .with_tag("page")
        .with_query_parameter::<Query>()
        .with_page_response("Html page to update clear client cookies and complete user logout")
}
