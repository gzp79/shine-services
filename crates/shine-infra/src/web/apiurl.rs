use serde::{Deserialize, Serialize};
use std::ops::{Deref, DerefMut};
use url::Url;
use utoipa::{
    openapi::{schema::Schema, KnownFormat, ObjectBuilder, SchemaFormat, Type},
    PartialSchema, ToSchema,
};

/// Url type used in the API
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiUrl(Url);

impl PartialSchema for ApiUrl {
    fn schema() -> utoipa::openapi::RefOr<Schema> {
        ObjectBuilder::new()
            .schema_type(Type::String)
            .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uri)))
            .build()
            .into()
    }
}

impl ToSchema for ApiUrl {}

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
