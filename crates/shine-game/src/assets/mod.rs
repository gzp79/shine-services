mod asset_plugin;
mod asset_source_plugin;
mod game_manifest;
mod manifest;
mod web_asset_reader;

pub use self::{
    asset_plugin::AssetPlugin,
    asset_source_plugin::{AssetSourcePlugin, ASSET_SOURCE_ID, MANIFEST_SOURCE_ID},
    game_manifest::{GameManifestError, GameManifestRequests, GameManifests},
    manifest::{Manifest, ManifestLoader, ManifestLoaderError},
    web_asset_reader::{WebAssetConfig, WebAssetReader},
};
