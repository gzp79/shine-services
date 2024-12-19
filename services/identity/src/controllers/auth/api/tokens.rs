use crate::{
    controllers::{ApiKind, AppState},
    repositories::identity::{TokenInfo, TokenKind},
};
use axum::{extract::State, http::StatusCode, Extension, Json};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use shine_service::{
    axum::{
        ApiEndpoint, ApiMethod, IntoProblem, Problem, ProblemConfig, SiteInfo, ValidatedJson, ValidatedPath,
        ValidationErrorEx,
    },
    service::{CheckedCurrentUser, ClientFingerprint},
};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use validator::{Validate, ValidationError};

#[derive(Deserialize, Validate, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreateTokenRequest {
    /// The kind of token to create, Allowed kinds are apiKey or singleAccess.
    /// access token can be created only through the login endpoint with enabled remember-me.
    #[validate(custom(function = "validate_allowed_kind"))]
    kind: TokenKind,
    /// The expiration The maximum validity range is 10s .. 1 year, but server config
    /// may reduce the maximum value through the ttl parameters.
    #[validate(range(min = 10, max = 31_536_000))]
    time_to_live: usize,
    /// If set to true, the token is bound to the current site
    bind_to_site: bool,
}

fn validate_allowed_kind(kind: &TokenKind) -> Result<(), ValidationError> {
    match kind {
        TokenKind::SingleAccess => Ok(()),
        TokenKind::Persistent => Ok(()),
        TokenKind::Access => Err(ValidationError::new("oneOf").with_message("Access tokens are not allowed".into())),
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct CreatedToken {
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

async fn create_token(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    fingerprint: ClientFingerprint,
    site_info: SiteInfo,
    ValidatedJson(params): ValidatedJson<CreateTokenRequest>,
) -> Result<Json<CreatedToken>, Problem> {
    let time_to_live = Duration::seconds(params.time_to_live as i64);

    // validate time_to_live against server config
    let max_time_to_live = match params.kind {
        TokenKind::SingleAccess => state.settings().token.ttl_single_access,
        TokenKind::Persistent => state.settings().token.ttl_api_key,
        TokenKind::Access => unreachable!(),
    };
    if time_to_live > max_time_to_live {
        return Err(ValidationError::new("range")
            .with_param("min", &10)
            .with_param("max", &max_time_to_live.num_seconds())
            .with_param("value", &params.time_to_live)
            .into_constraint_error("time_to_live")
            .into_problem(&problem_config));
    }

    let site_fingerprint = if params.bind_to_site { Some(&fingerprint) } else { None };
    let user_token = state
        .token_service()
        .create_user_token(user.user_id, params.kind, &time_to_live, site_fingerprint, &site_info)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to create token", err))?;

    Ok(Json(CreatedToken {
        kind: params.kind,
        token: user_token.token,
        token_hash: user_token.token_hash,
        token_type: "Bearer".into(),
        expire_at: user_token.expire_at,
    }))
}

pub fn ep_create_token() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Post, ApiKind::Api("/auth/user/tokens"), create_token)
        .with_operation_id("create_token")
        .with_tag("auth")
        //.with_checked_user()
        .with_json_request::<CreateTokenRequest>()
        .with_json_response::<CreatedToken>(StatusCode::OK)
}

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
struct TokenHash {
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

async fn get_token(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    ValidatedPath(params): ValidatedPath<TokenHash>,
) -> Result<Json<ActiveToken>, Problem> {
    let token = state
        .identity_service()
        .find_token_by_hash(&params.hash)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "Failed to find token", err))?
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
        Err(Problem::not_found().with_instance_str(format!("{{auth_api}}/user/tokens/{}", params.hash)))
    }
}

pub fn ep_get_token() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/tokens/:hash"), get_token)
        .with_operation_id("get_token")
        .with_tag("auth")
        .with_path_parameter::<TokenHash>()
        .with_json_response::<ActiveToken>(StatusCode::OK)
}

async fn delete_token(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    ValidatedPath(params): ValidatedPath<TokenHash>,
) -> Result<(), Problem> {
    let token = state
        .identity_service()
        .delete_token_by_user(user.user_id, &params.hash)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "Could not revoke token", err))?;

    if token.is_none() {
        Err(Problem::not_found().with_instance_str(format!("{{auth_api}}/user/tokens/{}", params.hash)))
    } else {
        Ok(())
    }
}

pub fn ep_delete_token() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Delete, ApiKind::Api("/auth/user/tokens/:hash"), delete_token)
        .with_operation_id("delete_token")
        .with_tag("auth")
        .with_path_parameter::<TokenHash>()
        .with_status_response(StatusCode::OK, "Token revoked")
}

async fn list_tokens(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
) -> Result<Json<ActiveTokens>, Problem> {
    let tokens = state
        .identity_service()
        .list_all_tokens_by_user(&user.user_id)
        .await
        .map_err(|err| Problem::internal_error(&problem_config, "Could not find tokens", err))?
        .into_iter()
        .map(ActiveToken::from)
        .collect();
    Ok(Json(ActiveTokens { tokens }))
}

pub fn ep_list_tokens() -> ApiEndpoint<AppState> {
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/tokens"), list_tokens)
        .with_operation_id("list_tokens")
        .with_tag("auth")
        .with_schema::<ActiveToken>()
        .with_json_response::<ActiveTokens>(StatusCode::OK)
}
