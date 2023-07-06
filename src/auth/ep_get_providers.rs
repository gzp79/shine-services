use crate::auth::AuthServiceState;
use axum::{extract::State, Json};

pub(in crate::auth) async fn get_providers(State(state): State<AuthServiceState>) -> Json<Vec<String>> {
    let providers = state.settings_manager.auth_providers().to_vec();
    Json(providers)
}
