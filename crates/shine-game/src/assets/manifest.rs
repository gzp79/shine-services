use bevy::{
    asset::{io::Reader, Asset, AssetLoader, LoadContext, LoadDirectError},
    log,
    reflect::TypePath,
};
use serde::Deserialize;
use std::collections::HashMap;
use thiserror::Error as ThisError;

#[derive(Clone, Asset, TypePath, Deserialize, Debug)]
pub struct Manifest(pub HashMap<String, String>);

#[derive(Debug, ThisError)]
pub enum ManifestLoaderError {
    #[error("Could not load asset: {0}")]
    Io(#[from] std::io::Error),
    #[error("Could not parse Json: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error(transparent)]
    LoadDirectError(#[from] LoadDirectError),
}

pub struct ManifestLoader {}

impl Default for ManifestLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ManifestLoader {
    pub fn new() -> Self {
        Self {}
    }
}

impl AssetLoader for ManifestLoader {
    type Asset = Manifest;
    type Settings = ();
    type Error = ManifestLoaderError;

    fn extensions(&self) -> &[&str] {
        &["json"]
    }

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let manifest = serde_json::de::from_slice::<Manifest>(&bytes)?;
        log::debug!("Loaded manifest: {manifest:?}");
        Ok(manifest)
    }
}
