use crate::{
    app_state::AppState,
    repositories::identity::{TokenInfo, TokenKind},
};
use axum::{extract::State, Extension, Json};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shine_core::web::{
    CheckedCurrentUser, ClientFingerprint, IntoProblemResponse, Problem, ProblemConfig, ProblemResponse, SiteInfo,
    ValidatedJson, ValidatedPath, ValidationErrorEx,
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum ManualTokenKind {
    Persistent,
    SingleAccess,
}

impl From<ManualTokenKind> for TokenKind {
    fn from(kind: ManualTokenKind) -> Self {
        match kind {
            ManualTokenKind::Persistent => TokenKind::Persistent,
            ManualTokenKind::SingleAccess => TokenKind::SingleAccess,
        }
    }
}

#[derive(Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateTokenRequest {
    kind: ManualTokenKind,
    /// The expiration The maximum validity range is 10s .. 1 year, but server config
    /// may reduce the maximum value through the ttl parameters.
    #[validate(range(min = 10, max = 31_536_000))]
    time_to_live: usize,
    /// If set to true, the token is bound to the current site
    bind_to_site: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatedToken {
    /// The kind of the created token
    kind: TokenKind,
    /// The new token. Backend does not store the raw token and it is not possible to retrieve it.
    token: String,
    /// The unique id of the token
    token_hash: String,
    /// Authorization type
    token_type: String,
    /// Date of the expiration of the token
    expire_at: DateTime<Utc>,
}

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct TokenHash {
    hash: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveToken {
    pub token_hash: String,
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

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTokens {
    tokens: Vec<ActiveToken>,
}

impl From<TokenInfo> for ActiveToken {
    fn from(value: TokenInfo) -> Self {
        ActiveToken {
            token_hash: value.token_hash,
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

#[utoipa::path(
    post,
    path = "/api/auth/user/tokens",
    tag = "auth",
    request_body = CreateTokenRequest,
    responses(
        (status = OK, body = CreatedToken)
    )
)]
pub async fn create_token(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
    ValidatedJson(params): ValidatedJson<CreateTokenRequest>,
) -> Result<Json<CreatedToken>, ProblemResponse> {
    let time_to_live = Duration::seconds(params.time_to_live as i64);

    // validate time_to_live against server config
    let max_time_to_live = match params.kind {
        ManualTokenKind::SingleAccess => state.settings().token.ttl_single_access,
        ManualTokenKind::Persistent => state.settings().token.ttl_api_key,
    };
    if time_to_live > max_time_to_live {
        return Err(ValidationError::new("range")
            .with_param("min", &10)
            .with_param("max", &max_time_to_live.num_seconds())
            .with_param("value", &params.time_to_live)
            .into_constraint_error("time_to_live")
            .into_response(&problem_config));
    }

    let site_fingerprint = if params.bind_to_site { Some(&fingerprint) } else { None };
    let user_token = state
        .stored_token_service()
        .create_user_token(
            user.user_id,
            params.kind.into(),
            &time_to_live,
            site_fingerprint,
            None,
            &site_info,
        )
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(Json(CreatedToken {
        kind: params.kind.into(),
        token: user_token.token,
        token_hash: user_token.token_hash,
        token_type: "Bearer".into(),
        expire_at: user_token.expire_at,
    }))
}

#[utoipa::path(
    get,
    path = "/api/auth/user/tokens/{hash}",
    tag = "auth",
    params(
        TokenHash
    ),
    responses(
        (status = OK, body = ActiveToken)
    )
)]

pub async fn get_token(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    ValidatedPath(params): ValidatedPath<TokenHash>,
) -> Result<Json<ActiveToken>, ProblemResponse> {
    let token = state
        .identity_service()
        .find_token_by_hash(&params.hash)
        .await
        .map_err(|err| err.into_response(&problem_config))?
        .and_then(|t| {
            if t.user_id == user.user_id {
                Some(ActiveToken::from(t))
            } else {
                log::warn!(
                    "User {} tried to access token-hash ({}) of user {}",
                    user.user_id,
                    params.hash,
                    t.user_id
                );
                None
            }
        });

    if let Some(token) = token {
        Ok(Json(token))
    } else {
        Err(Problem::not_found()
            .with_instance_str(format!("{{auth_api}}/user/tokens/{}", params.hash))
            .into_response(&problem_config))
    }
}

#[utoipa::path(
    delete,
    path = "/api/auth/user/tokens/{hash}",
    tag = "auth",
    params(TokenHash),
    responses(
        (status = OK, description = "Token revoked")
    )
)]
pub async fn delete_token(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    ValidatedPath(params): ValidatedPath<TokenHash>,
) -> Result<(), ProblemResponse> {
    let token = state
        .identity_service()
        .delete_hashed_token_by_user(user.user_id, &params.hash)
        .await
        .map_err(|err| err.into_response(&problem_config))?;

    if token.is_none() {
        Err(Problem::not_found()
            .with_instance_str(format!("{{auth_api}}/user/tokens/{}", params.hash))
            .into_response(&problem_config))
    } else {
        Ok(())
    }
}

#[utoipa::path(
    get,
    path = "/api/auth/user/tokens",
    tag = "auth",
    responses(
        (status = OK, body = ActiveTokens)
    )
)]
pub async fn list_tokens(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
) -> Result<Json<ActiveTokens>, ProblemResponse> {
    let tokens = state
        .identity_service()
        .list_all_tokens_by_user(&user.user_id)
        .await
        .map_err(|err| err.into_response(&problem_config))?
        .into_iter()
        .map(ActiveToken::from)
        .collect();
    Ok(Json(ActiveTokens { tokens }))
}
