use crate::services::{ep_search_identity};
use axum::{routing::get, Router};

pub struct IdentityServiceBuilder;

impl IdentityServiceBuilder {
    pub fn into_router<S>(self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        Router::new()            
            .route("/", get(ep_search_identity::search_identity))
    }
}
