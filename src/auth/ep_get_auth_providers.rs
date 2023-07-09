use crate::auth::AuthServiceState;
use axum::{extract::State, Json};

pub(in crate::auth) async fn ep_get_auth_providers(State(state): State<AuthServiceState>) -> Json<Vec<String>> {
    let providers = state.providers().to_vec();
    Json(providers)
}
