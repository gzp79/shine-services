use crate::{
    app_state::AppState,
    controllers::auth::{AuthError, AuthPage, AuthSession, PageUtils},
};
use axum::extract::State;
use serde::Deserialize;
use shine_infra::{
    language::Language,
    web::{ErrorResponse, InputError, SiteInfo, ValidatedQuery},
};
use url::Url;
use utoipa::IntoParams;
use validator::Validate;

#[derive(Debug, Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct QueryParams {
    #[validate(email)]
    email: String,
    redirect_url: Option<Url>,
    error_url: Option<Url>,
    remember_me: Option<bool>,
    captcha: Option<String>,
    lang: Option<Language>,
}

/// Login with token using query, auth and cookie as sources.
#[utoipa::path(
    get,
    path = "/auth/email/login",
    tag = "page",
    params(
        QueryParams
    ),
    responses(
        (status = OK, description="Start an email login flow")
    )
)]
pub async fn email_login(
    State(state): State<AppState>,
    query: Result<ValidatedQuery<QueryParams>, ErrorResponse<InputError>>,
    auth_session: AuthSession,
    site_info: SiteInfo,
) -> AuthPage {
    let query = match query {
        Ok(ValidatedQuery(query)) => query,
        Err(error) => return PageUtils::new(&state).error(auth_session, error.problem, None, None),
    };

    log::debug!("Query: {:#?}", query);

    if let Err(err) = state.captcha_validator().validate(query.captcha.as_deref()).await {
        return PageUtils::new(&state).error(auth_session, err, query.error_url.as_ref(), query.redirect_url.as_ref());
    };

    log::debug!("Email registration flow triggered...");
    let auth_session = auth_session
        .with_external_login(None)
        .revoke_access(&state)
        .await
        .revoke_session(&state)
        .await;

    let identity = match state
        .login_email_handler()
        .send_login_email(
            &query.email,
            query.remember_me,
            query.redirect_url.as_ref(),
            &site_info,
            query.lang,
        )
        .await
    {
        Ok(identity) => identity,
        Err(err) => {
            return PageUtils::new(&state).error(
                auth_session,
                err,
                query.error_url.as_ref(),
                query.redirect_url.as_ref(),
            )
        }
    };

    log::info!("Email flow completed for: {}", identity.id);
    PageUtils::new(&state).error(auth_session, AuthError::EmailLogin, query.error_url.as_ref(), None)
}
