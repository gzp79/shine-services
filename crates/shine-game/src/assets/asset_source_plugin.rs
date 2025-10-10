use crate::assets::{WebAssetConfig, WebAssetReader};
use bevy::{
    app::{App, Plugin},
    asset::{io::AssetSource, AssetApp},
};

pub const MANIFEST_SOURCE_ID: &str = "manifests";
pub const ASSET_SOURCE_ID: &str = "assets";

pub struct AssetSourcePlugin {
    pub web_asset_config: WebAssetConfig,
}

impl AssetSourcePlugin {
    pub fn new(web_asset_config: WebAssetConfig) -> Self {
        Self { web_asset_config }
    }
}

impl Plugin for AssetSourcePlugin {
    fn build(&self, app: &mut App) {
        let reader = WebAssetReader::new(self.web_asset_config.clone()).unwrap();
        app.register_asset_source(
            MANIFEST_SOURCE_ID,
            AssetSource::build().with_reader(move || Box::new(reader.clone())),
        );

        let reader = WebAssetReader::new(self.web_asset_config.clone().with_no_version()).unwrap();
        app.register_asset_source(
            ASSET_SOURCE_ID,
            AssetSource::build().with_reader(move || Box::new(reader.clone())),
        );
    }
}
