/// Login using a token. If token is not present an Unauthorized error is returned.
pub(in crate::auth) async fn token_login(
    State(state): State<AuthServiceState>,
    Extension(oauth2_client): Extension<Arc<OAuth2Client>>,
    Query(query): Query<LoginRequestParams>,
    mut auth_session: AuthSession,
) -> Response {
    if auth_session.user.is_none() {
        let html = create_ooops_page(&state, Some("Missing token"));
        return (StatusCode::Unauthorized, html).into_response();
    }

    todo!()
}

/// Login using a token. If token is not present an Unauthorized error is returned.
pub(in crate::auth) async fn token_register(
    State(state): State<AuthServiceState>,
    Extension(oauth2_client): Extension<Arc<OAuth2Client>>,
    Query(query): Query<LoginRequestParams>,
    mut auth_session: AuthSession,
) -> Response {
    if !auth_session.is_empty() {
        let html = create_ooops_page(&state, Some("A log out is required to switch account"));
        return (StatusCode::BAD_REQUEST, html).into_response();
    }

    todo!()
}
