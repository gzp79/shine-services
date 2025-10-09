use crate::assets::{
    game_manifest::{collect_game_manifests, load_game_manifests},
    AssetSourcePlugin, GameManifestRequests, GameManifests, Manifest, ManifestLoader,
};
use bevy::{
    app::{App, Plugin, PreUpdate},
    asset::AssetApp,
    ecs::schedule::IntoScheduleConfigs,
    log,
};

pub struct AssetPlugin {}

impl Default for AssetPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetPlugin {
    pub fn new() -> Self {
        Self {}
    }
}

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<AssetSourcePlugin>() {
            log::error!("AssetPlugin requires AssetSourcePlugin to be added before calling the platform_init method.");
        }

        app.init_asset::<Manifest>();
        app.init_asset_loader::<ManifestLoader>();
        app.insert_resource(GameManifestRequests::new());
        app.insert_resource(GameManifests::new());

        app.add_systems(PreUpdate, (load_game_manifests, collect_game_manifests).chain());
    }
}
