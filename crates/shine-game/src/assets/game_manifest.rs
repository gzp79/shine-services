use crate::assets::{Manifest, ASSET_SOURCE_ID, MANIFEST_SOURCE_ID};
use bevy::{
    asset::{AssetEvent, AssetId, AssetServer, Assets, Handle},
    ecs::{
        change_detection::DetectChanges,
        message::MessageReader,
        resource::Resource,
        system::{Res, ResMut},
    },
    log,
};
use std::collections::HashMap;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub enum GameManifestError {
    #[error("Invalid asset path: {0}")]
    InvalidPath(String),
    #[error("Invalid asset type: {asset}")]
    InvalidAsset { asset: String },
    #[error("Manifest not available for: {asset}")]
    ManifestNotAvailable { asset: String },
    #[error("Asset key not found in manifest {asset}: {key}")]
    KeyNotFound { asset: String, key: String },
}

#[derive(Resource)]
pub struct GameManifestRequests {
    /// the requested manifests
    manifests: HashMap<String, String>,
    /// the list of processed manifests
    processed_manifests: HashMap<String, Handle<Manifest>>,
    id_to_manifests: HashMap<AssetId<Manifest>, String>,
}

impl Default for GameManifestRequests {
    fn default() -> Self {
        Self::new()
    }
}

impl GameManifestRequests {
    pub fn new() -> Self {
        Self {
            manifests: HashMap::new(),
            processed_manifests: HashMap::new(),
            id_to_manifests: HashMap::new(),
        }
    }

    pub fn manifests(&self) -> impl Iterator<Item = &String> {
        self.manifests.keys()
    }

    pub fn add_manifest(&mut self, name: &str, path: &str) {
        self.manifests.insert(name.to_string(), path.to_string());
    }

    pub fn add_manifests(&mut self, iter: impl IntoIterator<Item = (String, String)>) {
        self.manifests.extend(iter);
    }

    pub fn remove_manifest(&mut self, name: &str) {
        self.manifests.remove(name);
        if let Some(handle) = self.processed_manifests.remove(name) {
            self.id_to_manifests.remove(&handle.id());
        }
    }

    fn get_unprocessed(&self) -> Vec<(String, String)> {
        self.manifests
            .iter()
            .filter(|(name, _)| !self.processed_manifests.contains_key(*name))
            .map(|(name, path)| (name.clone(), path.clone()))
            .collect()
    }

    fn set_processed(&mut self, asset: String, handle: Handle<Manifest>) {
        self.processed_manifests.insert(asset.clone(), handle.clone());
        self.id_to_manifests.insert(handle.id(), asset);
    }
}

#[derive(Resource, Clone)]
pub struct GameManifests {
    pub manifests: HashMap<String, Manifest>,
    pub id_to_manifests: HashMap<AssetId<Manifest>, String>,
}

impl Default for GameManifests {
    fn default() -> Self {
        Self::new()
    }
}

impl GameManifests {
    pub fn new() -> Self {
        Self {
            manifests: HashMap::new(),
            id_to_manifests: HashMap::new(),
        }
    }

    /// Return true if all assets are loaded
    pub fn is_loaded<'a>(&'a self, assets: impl IntoIterator<Item = &'a str>) -> bool {
        for asset in assets {
            if !self.manifests.contains_key(asset) {
                return false;
            }
        }
        true
    }

    fn resolve_asset_key(&self, asset: &str, key: &str) -> Result<String, GameManifestError> {
        let manifest = self
            .manifests
            .get(asset)
            .ok_or_else(|| GameManifestError::ManifestNotAvailable { asset: asset.to_string() })?;
        let uri = manifest.0.get(key).ok_or_else(|| GameManifestError::KeyNotFound {
            asset: asset.to_string(),
            key: key.to_string(),
        })?;
        Ok(format!("{ASSET_SOURCE_ID}://{uri}"))
    }

    pub fn resolve_path(&self, path: &str) -> Result<String, GameManifestError> {
        let mut parts = path.splitn(2, '/');

        let manifest_type = parts
            .next()
            .ok_or_else(|| GameManifestError::InvalidPath("missing asset type".to_string()))?;
        let key = parts
            .next()
            .ok_or_else(|| GameManifestError::InvalidPath("missing asset key".to_string()))?;

        self.resolve_asset_key(manifest_type, key)
            .inspect(|p| log::debug!("Resolved game asset path: {path} -> {p}"))
    }

    fn store(&mut self, asset: &str, id: AssetId<Manifest>, manifest: Manifest) {
        self.manifests.insert(asset.to_owned(), manifest);
        self.id_to_manifests.insert(id, asset.to_owned());
    }

    fn remove_by_id(&mut self, id: &AssetId<Manifest>) -> Option<String> {
        if let Some(asset) = self.id_to_manifests.remove(id) {
            self.manifests.remove(&asset);
            Some(asset)
        } else {
            None
        }
    }
}

pub fn load_game_manifests(mut requests: ResMut<GameManifestRequests>, asset_server: Res<AssetServer>) {
    // todo: Resources has no Observer based change detection, so we manually check if the requests changed
    if !requests.is_changed() {
        return;
    }

    for (asset, path) in requests.get_unprocessed() {
        log::info!("Loading game manifests {asset}...");
        let handle = asset_server.load::<Manifest>(format!("{MANIFEST_SOURCE_ID}://{path}",));
        requests.set_processed(asset, handle);
    }
}

pub fn collect_game_manifests(
    mut manifest_messages: MessageReader<AssetEvent<Manifest>>,
    game_manifest_requests: Res<GameManifestRequests>,
    mut game_manifests: ResMut<GameManifests>,
    manifest_assets: Res<Assets<Manifest>>,
) {
    for msg in manifest_messages.read() {
        log::info!("Game manifest asset event: {:?}", msg);
        match msg {
            AssetEvent::Added { id } | AssetEvent::Modified { id } => {
                if let Some(asset) = game_manifest_requests.id_to_manifests.get(id) {
                    let manifest = manifest_assets.get(*id).unwrap();
                    log::info!("Game manifest [{}] changed: {:?}", asset, manifest);
                    game_manifests.store(asset, *id, manifest.clone());
                }
            }
            AssetEvent::Removed { id } => {
                if let Some(asset) = game_manifests.remove_by_id(id) {
                    log::info!("Game manifest [{}] removed", asset);
                }
            }
            AssetEvent::LoadedWithDependencies { .. } => {}
            AssetEvent::Unused { .. } => {}
        }
    }
}
