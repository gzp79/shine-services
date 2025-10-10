use bevy::{
    asset::io::{AssetReader, AssetReaderError, PathStream, Reader, VecReader},
    log,
    tasks::ConditionalSendFuture,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[cfg(not(target_arch = "wasm32"))]
use ureq::{config::Config as UReqConfig, tls::TlsConfig, Agent};

// A convenience alias to simplify conditional compilation.
#[cfg(target_arch = "wasm32")]
type Agent = ();

#[derive(Clone)]
pub struct WebAssetConfig {
    pub base_uri: String,
    pub version: Option<String>,
    pub allow_insecure: bool,
}

impl WebAssetConfig {
    pub fn with_version(self, version: String) -> Self {
        Self { version: Some(version), ..self }
    }

    pub fn with_no_version(self) -> Self {
        Self { version: None, ..self }
    }

    pub async fn with_loaded_version(self) -> Result<Self, AssetReaderError> {
        #[derive(Deserialize)]
        struct VersionManifest {
            version: String,
        }

        let agent = self.create_agent();
        let base_path = PathBuf::from(&self.base_uri);
        let path = PathBuf::from("latest.json");

        let buffer = get(agent, &base_path, &path).await?;
        let manifest: VersionManifest = serde_json::from_slice(&buffer).map_err(|err| {
            AssetReaderError::Io(std::io::Error::other(std::format!("failed to parse version manifest: {err}")).into())
        })?;

        Ok(self.with_version(manifest.version))
    }

    fn create_agent(&self) -> Agent {
        #[cfg(not(target_arch = "wasm32"))]
        {
            UReqConfig::builder()
                .tls_config(TlsConfig::builder().disable_verification(self.allow_insecure).build())
                .https_only(true)
                .build()
                .new_agent()
        }

        #[cfg(target_arch = "wasm32")]
        {
            ()
        }
    }
}

/// Asset reader that treats paths as urls to load assets from.
#[derive(Clone)]
pub struct WebAssetReader {
    agent: Agent,
    base_path: PathBuf,
}

impl WebAssetReader {
    pub fn new(config: WebAssetConfig) -> Result<Self, AssetReaderError> {
        let agent = config.create_agent();

        let mut base_path = PathBuf::from(&config.base_uri);
        if let Some(version) = config.version {
            let platform = if cfg!(target_arch = "wasm32") { "web" } else { "pc" };
            base_path = base_path.join(version).join(platform);
        }

        log::info!("Using web asset base path: {}", base_path.display());

        Ok(Self { base_path, agent })
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
async fn get<'a>(_agent: (), base_path: &Path, path: &Path) -> Result<Vec<u8>, AssetReaderError> {
    // based on https://github.com/bevyengine/bevy/blob/main/crates/bevy_asset/src/io/wasm.rs

    use js_sys::{Uint8Array, JSON};
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::Response;

    let uri = WebAssetReader::make_uri(&base_path, &path)?;
    log::debug!("Loading web asset: {}", uri);

    fn js_value_to_err(context: &str) -> impl FnOnce(JsValue) -> std::io::Error + '_ {
        move |value| {
            let message = match JSON::stringify(&value) {
                Ok(js_str) => format!("Failed to {context}: {js_str}"),
                Err(_) => format!("Failed to {context}"),
            };
            std::io::Error::other(message)
        }
    }

    // The JS global scope includes a self-reference via a specializing name, which can be used to determine the type of global context available.
    let window = web_sys::window().ok_or_else(|| std::io::Error::other("failed to get window"))?;
    let resp_value = JsFuture::from(window.fetch_with_str(&uri))
        .await
        .map_err(js_value_to_err("fetch path"))?;
    let resp = resp_value
        .dyn_into::<Response>()
        .map_err(js_value_to_err("convert fetch to Response"))?;
    match resp.status() {
        200 => {
            let data = JsFuture::from(resp.array_buffer().unwrap()).await.unwrap();
            let bytes = Uint8Array::new(&data).to_vec();
            Ok(bytes)
        }
        404 => Err(AssetReaderError::NotFound(path.to_path_buf())),
        status => Err(AssetReaderError::HttpError(status)),
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn get(agent: Agent, base_path: &Path, path: &Path) -> Result<Vec<u8>, AssetReaderError> {
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

            Ok(buffer)
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
        async {
            let buffer = get(self.agent.clone(), &self.base_path, path).await?;
            let reader: Box<dyn Reader> = Box::new(VecReader::new(buffer));
            Ok(reader)
        }
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
