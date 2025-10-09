use crate::assets::{WebAssetConfig, WebAssetReader};
use bevy::{
    app::{App, Plugin},
    asset::{io::AssetSource, AssetApp},
};

pub const MANIFEST_SOURCE_ID: &str = "manifests";
pub const ASSET_SOURCE_ID: &str = "assets";

pub struct AssetSourcePlugin {
    pub base_uri: String,
    pub allow_insecure: bool,
}

impl AssetSourcePlugin {
    pub fn new(base_uri: impl ToString, allow_insecure: bool) -> Self {
        Self {
            base_uri: base_uri.to_string(),
            allow_insecure,
        }
    }
}

impl Plugin for AssetSourcePlugin {
    fn build(&self, app: &mut App) {
        let reader = WebAssetReader::new(WebAssetConfig {
            base_uri: self.base_uri.clone(),
            allow_insecure: self.allow_insecure,
            is_versioned: true,
        })
        .unwrap();
        app.register_asset_source(
            MANIFEST_SOURCE_ID,
            AssetSource::build().with_reader(move || Box::new(reader.clone())),
        );

        let reader = WebAssetReader::new(WebAssetConfig {
            base_uri: self.base_uri.clone(),
            allow_insecure: self.allow_insecure,
            is_versioned: false,
        })
        .unwrap();
        app.register_asset_source(
            ASSET_SOURCE_ID,
            AssetSource::build().with_reader(move || Box::new(reader.clone())),
        );
    }
}
