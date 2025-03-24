use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use url::Url;
use utoipa::ToSchema;

/// Url type used in the API
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[schema(value_type = String )]
#[schema(as = Url)]
pub struct ApiUrl(Url);

impl ApiUrl {
    pub fn new(url: Url) -> Self {
        Self(url)
    }

    pub fn into_url(self) -> Url {
        self.0
    }
}

impl Deref for ApiUrl {
    type Target = Url;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ApiUrl {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
