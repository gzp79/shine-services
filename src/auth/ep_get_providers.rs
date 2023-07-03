use crate::db::SettingsManager;
use axum::{Extension, Json};

pub(in crate::auth) async fn get_providers(Extension(settings): Extension<SettingsManager>) -> Json<Vec<String>> {
    let providers = settings.auth_providers().iter().cloned().collect();
    Json(providers)
}
