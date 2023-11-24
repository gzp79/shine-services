use crate::{
    auth::AuthServiceState,
    openapi::ApiKind,
    repositories::{TokenInfo, TokenKind},
};
use axum::{body::HttpBody, extract::State, http::StatusCode, BoxError, Json};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem, SiteInfo, ValidatedPath, ValidatedQuery},
    service::{CheckedCurrentUser, ClientFingerprint},
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
#[validate(schema(function = "validate_token_spec"))]
struct CreateQuery {
    /// The kind of token to create, Allowed kinds are persistent or singleAccess
    #[validate(custom = "validate_allowed_kind")]
    kind: TokenKind,
    /// If set a persistent token will be created with the given timeout,
    /// otherwise a single access token is created
    #[validate(range(min = 30, max = 7_890_000))]
    timeout: usize,
    /// If set to true, the token is bound to the current site
    bind_to_site: bool,
}

fn validate_allowed_kind(kind: &TokenKind) -> Result<(), ValidationError> {
    match kind {
        TokenKind::SingleAccess => Ok(()),
        TokenKind::Persistent => Ok(()),
        TokenKind::Access => Err(ValidationError::new("Access tokens are not allowed")),
    }
}

fn validate_token_spec(query: &CreateQuery) -> Result<(), ValidationError> {
    if query.kind == TokenKind::SingleAccess && !(30..30 * 60).contains(&query.timeout) {
        return Err(ValidationError::new("timeout is required for persistent tokens"));
    }
    Ok(())
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreatedToken {
    /// The kind of the created token
    kind: TokenKind,
    /// The token
    token: String,
    /// Authorization type
    token_type: String,
    /// Date of the expiration of the token
    expire_at: DateTime<Utc>,
}

/// Get the information about the current user. The cookie is not accessible
/// from javascript, thus this endpoint can be used to get details about the current user.
async fn token_create(
    State(state): State<AuthServiceState>,
    user: CheckedCurrentUser,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
    ValidatedQuery(query): ValidatedQuery<CreateQuery>,
) -> Result<Json<CreatedToken>, Problem> {
    // check if session is still valid
    let _ = state
        .session_manager()
        .find(user.user_id, user.key)
        .await
        .map_err(Problem::internal_error_from)?
        .ok_or(Problem::unauthorized())?;

    // create a new persistent or single access token
    let ttl = Duration::seconds(query.timeout as i64);
    let fingerprint = if query.bind_to_site { Some(&fingerprint) } else { None };
    let token_cookie = state
        .create_token_with_retry(user.user_id, fingerprint, &site_info, query.kind, ttl)
        .await
        .map_err(Problem::internal_error_from)?;

    Ok(Json(CreatedToken {
        kind: query.kind,
        token: token_cookie.token,
        token_type: "Bearer".into(),
        expire_at: token_cookie.expire_at,
    }))
}

pub fn ep_token_create<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/token"), token_create)
        .with_operation_id("token_create")
        .with_tag("auth")
        //.with_checked_user()
        .with_query_parameter::<CreateQuery>()
        .with_json_response::<CreatedToken>(StatusCode::OK)
}

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct TokenPathParam {
    token: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveToken {
    pub token_fingerprint: String,
    pub user_id: Uuid,
    pub kind: TokenKind,
    pub created_at: DateTime<Utc>,
    pub expire_at: DateTime<Utc>,
    pub is_expired: bool,
    pub agent: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
}

impl From<TokenInfo> for ActiveToken {
    fn from(value: TokenInfo) -> Self {
        ActiveToken {
            token_fingerprint: value.token_hash,
            user_id: value.user_id,
            kind: value.kind,
            created_at: value.created_at,
            expire_at: value.expire_at,
            is_expired: value.is_expired,
            agent: value.agent,
            country: value.country,
            region: value.region,
            city: value.city,
        }
    }
}

async fn token_get(
    State(state): State<AuthServiceState>,
    user: CheckedCurrentUser,
    ValidatedPath(params): ValidatedPath<TokenPathParam>,
) -> Result<Json<ActiveToken>, Problem> {
    let token = state
        .identity_manager()
        .find_token_hash(&params.token)
        .await
        .map_err(Problem::internal_error_from)?
        .and_then(|(i, t)| {
            if i.id == user.user_id {
                Some(ActiveToken::from(t))
            } else {
                log::warn!(
                    "User {} tried to access token ({}) of user {}",
                    user.user_id,
                    params.token,
                    i.id
                );
                None
            }
        });

    if let Some(token) = token {
        Ok(Json(token))
    } else {
        Err(Problem::not_found().with_instance(format!("{{auth_api}}/user/tokens/{}", params.token)))
    }
}

pub fn ep_token_get<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/tokens/:token"), token_get)
        .with_operation_id("token_get")
        .with_tag("auth")
        .with_schema::<ActiveToken>()
        .with_path_parameter::<TokenPathParam>()
        .with_json_response::<ActiveTokens>(StatusCode::OK)
}

async fn token_delete(
    State(state): State<AuthServiceState>,
    user: CheckedCurrentUser,
    ValidatedPath(params): ValidatedPath<TokenPathParam>,
) -> Result<(), Problem> {
    let token = state
        .identity_manager()
        .delete_token_hash(user.user_id, &params.token)
        .await
        .map_err(Problem::internal_error_from)?;

    if token.is_none() {
        Err(Problem::not_found().with_instance(format!("{{auth_api}}/user/tokens/{}", params.token)))
    } else {
        Ok(())
    }
}

pub fn ep_token_delete<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(
        ApiMethod::Delete,
        ApiKind::Api("/auth/user/tokens/:token"),
        token_delete,
    )
    .with_operation_id("token_delete")
    .with_tag("auth")
    .with_path_parameter::<TokenPathParam>()
    .with_status_response(StatusCode::OK, "Token revoked")
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTokens {
    tokens: Vec<ActiveToken>,
}

async fn token_list(
    State(state): State<AuthServiceState>,
    user: CheckedCurrentUser,
) -> Result<Json<ActiveTokens>, Problem> {
    let tokens = state
        .identity_manager()
        .list_all_tokens(&user.user_id)
        .await
        .map_err(Problem::internal_error_from)?
        .into_iter()
        .map(ActiveToken::from)
        .collect();
    Ok(Json(ActiveTokens { tokens }))
}

pub fn ep_token_list<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/tokens"), token_list)
        .with_operation_id("token_list")
        .with_tag("auth")
        .with_schema::<ActiveToken>()
        .with_json_response::<ActiveTokens>(StatusCode::OK)
}
