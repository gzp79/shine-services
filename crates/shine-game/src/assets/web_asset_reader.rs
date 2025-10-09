use bevy::{
    asset::io::{AssetReader, AssetReaderError, PathStream, Reader},
    log,
    tasks::ConditionalSendFuture,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use ureq::{config::Config as UReqConfig, tls::TlsConfig, Agent};

#[derive(Clone)]
pub struct WebAssetConfig {
    pub base_uri: String,
    pub allow_insecure: bool,
    pub is_versioned: bool,
}

/// Asset reader that treats paths as urls to load assets from.
#[derive(Clone)]
pub struct WebAssetReader {
    agent: Agent,
    base_path: PathBuf,
}

impl WebAssetReader {
    pub fn new(config: WebAssetConfig) -> Result<Self, AssetReaderError> {
        let agent = UReqConfig::builder()
            .tls_config(TlsConfig::builder().disable_verification(config.allow_insecure).build())
            .https_only(true)
            .build()
            .new_agent();

        let mut base_path = PathBuf::from(&config.base_uri);
        if config.is_versioned {
            let version = Self::load_version(&agent, &base_path)?;
            base_path = base_path.join(version);
        }

        log::info!("Using web asset base path: {}", base_path.display());

        Ok(Self { base_path, agent })
    }

    fn load_version(agent: &Agent, base_path: &Path) -> Result<String, AssetReaderError> {
        let path = PathBuf::from("latest.json");
        let uri = Self::make_uri(base_path, &path)?;

        match agent.get(uri).call() {
            Ok(mut response) => {
                #[derive(Deserialize)]
                struct VersionManifest {
                    version: String,
                }

                let reader = response.body_mut().with_config().reader();
                let manifest: VersionManifest = serde_json::from_reader(reader).map_err(|err| {
                    AssetReaderError::Io(
                        std::io::Error::other(std::format!("failed to parse version manifest: {err}")).into(),
                    )
                })?;
                Ok(manifest.version)
            }
            Err(ureq::Error::StatusCode(code)) => {
                if code == 404 {
                    Err(AssetReaderError::NotFound(path))
                } else {
                    Err(AssetReaderError::HttpError(code))
                }
            }
            Err(err) => Err(AssetReaderError::Io(
                std::io::Error::other(std::format!("unexpected error while loading version manifest: {err}")).into(),
            )),
        }
    }

    fn make_uri(base: &Path, resolved_path: &Path) -> Result<String, AssetReaderError> {
        let path = base.join(resolved_path);
        let str_path = path.to_str().ok_or_else(|| {
            AssetReaderError::Io(std::io::Error::other(std::format!("non-utf8 path: {}", path.display())).into())
        })?;
        let uri = str_path.replace("\\", "/").to_owned();
        Ok(uri)
    }
}

#[cfg(target_arch = "wasm32")]
async fn get<'a>(base_path: &Path, path: &Path) -> Result<Box<dyn Reader>, AssetReaderError> {
    use crate::io::wasm::HttpWasmAssetReader;

    let uri = WebAssetReader::make_uri(&base_path, &path)?;
    log::debug!("Loading web asset: {}", uri);

    HttpWasmAssetReader::new("")
        .fetch_bytes(uri)
        .await
        .map(|r| Box::new(r) as Box<dyn Reader>)
}

#[cfg(not(target_arch = "wasm32"))]
async fn get(agent: Agent, base_path: &Path, path: &Path) -> Result<Box<dyn Reader>, AssetReaderError> {
    use bevy::asset::io::VecReader;
    use blocking::unblock;
    use std::io::{BufReader, Read};

    let uri = WebAssetReader::make_uri(base_path, path)?;
    log::debug!("Loading web asset: {}", uri);

    // Use [`unblock`] to run the http request on a separately spawned thread as to not block bevy's
    // async executor.
    let response = unblock(move || agent.get(uri).call()).await;

    match response {
        Ok(mut response) => {
            let mut reader = BufReader::new(response.body_mut().with_config().reader());

            let mut buffer = Vec::new();
            reader.read_to_end(&mut buffer)?;

            Ok(Box::new(VecReader::new(buffer)))
        }
        // ureq considers all >=400 status codes as errors
        Err(ureq::Error::StatusCode(code)) => {
            if code == 404 {
                Err(AssetReaderError::NotFound(path.into()))
            } else {
                Err(AssetReaderError::HttpError(code))
            }
        }
        Err(err) => Err(AssetReaderError::Io(
            std::io::Error::other(std::format!(
                "unexpected error while loading asset {}: {}",
                path.display(),
                err
            ))
            .into(),
        )),
    }
}

impl AssetReader for WebAssetReader {
    fn read<'a>(
        &'a self,
        path: &'a Path,
    ) -> impl ConditionalSendFuture<Output = Result<Box<dyn Reader>, AssetReaderError>> {
        get(self.agent.clone(), &self.base_path, path)
    }

    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<Box<dyn Reader>, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn is_directory<'a>(&'a self, _path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(false)
    }

    async fn read_directory<'a>(&'a self, path: &'a Path) -> Result<Box<PathStream>, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }
}
