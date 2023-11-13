use crate::{auth::AuthServiceState, openapi::ApiKind, repositories::TokenKind};
use axum::{body::HttpBody, extract::State, http::StatusCode, BoxError, Json};
use chrono::{DateTime, Utc};
use serde::Serialize;
use shine_service::{
    axum::{ApiEndpoint, ApiMethod, Problem},
    service::CheckedCurrentUser,
};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActiveToken {
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
#[schema(as=ActiveTokens)]
pub struct Response {
    tokens: Vec<ActiveToken>,
}

async fn get_active_tokens(
    State(state): State<AuthServiceState>,
    user: CheckedCurrentUser,
) -> Result<Json<Response>, Problem> {
    let tokens = state
        .identity_manager()
        .list_all_tokens(&user.user_id)
        .await
        .map_err(Problem::internal_error_from)?
        .into_iter()
        .map(|s| ActiveToken {
            user_id: s.user_id,
            kind: s.kind,
            created_at: s.created_at,
            expire_at: s.expire_at,
            is_expired: s.is_expired,
            agent: s.agent,
            country: s.country,
            region: s.region,
            city: s.city,
        })
        .collect();
    Ok(Json(Response { tokens }))
}

pub fn ep_get_active_tokens<B>() -> ApiEndpoint<AuthServiceState, B>
where
    B: HttpBody + Send + 'static,
    B::Data: Send,
    B::Error: Into<BoxError>,
{
    ApiEndpoint::new(ApiMethod::Get, ApiKind::Api("/auth/user/tokens"), get_active_tokens)
        .with_operation_id("get_active_tokens")
        .with_tag("auth")
        .with_schema::<ActiveToken>()
        .with_json_response::<Response>(StatusCode::OK)
}
