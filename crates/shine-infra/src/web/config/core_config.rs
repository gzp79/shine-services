use crate::{azure::azure_keyvault_config::AzureKeyvaultConfigSource, web::Environment};
use azure_core::credentials::TokenCredential;
use azure_identity::{AzureCliCredential, ClientSecretCredential};
use config::{builder::AsyncState, Config, ConfigBuilder, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};

pub const DEFAULT_VERSION_CONFIG_FILE: &str = "server_version.json";

/// Partial configuration required for early setup.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CoreConfig {
    pub stage: String,
    pub version: String,
    pub before_layers: Vec<String>,
    pub after_layers: Vec<String>,
    pub root_file: String,
}

impl CoreConfig {
    pub fn new(stage: &str, config_file: Option<PathBuf>) -> Result<Self, ConfigError> {
        log::info!("Loading configuration for {}", stage);

        let root_file = config_file.unwrap_or_else(|| Path::new(&format!("server_config.{}.json", stage)).to_owned());
        let mut builder = Config::builder().add_source(File::from(root_file.as_path()));

        let version_path = Path::new(DEFAULT_VERSION_CONFIG_FILE);
        if version_path.exists() {
            builder = builder.add_source(File::from(version_path));
        } else {
            log::warn!("No version file found at {}", version_path.display());
            builder = builder.set_override("version", "custom")?;
        }

        builder = builder
            .set_override("stage", stage)?
            .set_override("rootFile", root_file.to_str().unwrap().to_string())?;

        let s = builder.build()?;
        let cfg: CoreConfig = s.try_deserialize()/*.inspect(|a| log::error!("{a:#?}"))*/?;

        log::debug!("pre-init configuration: {:#?}", cfg);
        Ok(cfg)
    }

    pub fn create_config_builder(&self) -> Result<ConfigBuilder<AsyncState>, ConfigError> {
        log::debug!("Setting up configuration builder...");
        let mut builder = ConfigBuilder::<AsyncState>::default();

        #[derive(Debug)]
        enum Layer<'a> {
            Base,
            Environment,
            Config(&'a str, &'a str, Option<&'a str>),
        }
        impl<'a> Layer<'a> {
            fn from_layer(layer: &'a str) -> Result<Self, ConfigError> {
                if layer == "environment" {
                    Ok(Layer::Environment)
                } else {
                    let mut tokens = layer.splitn(2, "://");
                    let schema = tokens.next().ok_or(ConfigError::FileParse {
                        uri: Some(layer.to_owned()),
                        cause: "Invalid config layer".into(),
                    })?;
                    Ok(Self::Config(schema, layer, tokens.next()))
                }
            }
        }

        let mut layers = Vec::with_capacity(self.before_layers.len() + self.after_layers.len() + 1);
        for l in self.before_layers.iter().map(|x| Layer::from_layer(x.as_str())) {
            layers.push(l?);
        }
        layers.push(Layer::Base);
        for l in self.after_layers.iter().map(|x| Layer::from_layer(x.as_str())) {
            layers.push(l?);
        }

        let mut azure_credentials: Option<Arc<dyn TokenCredential>> = None;
        for layer in layers {
            log::debug!("Adding layer: {:?}", layer);
            match layer {
                Layer::Base => {
                    builder = builder.add_source(File::from(Path::new(&self.root_file)));
                }
                Layer::Environment => {
                    builder = builder.add_source(Environment::new());
                }
                Layer::Config("file", url, path) => {
                    let path = path.ok_or(ConfigError::FileParse {
                        uri: Some(url.to_owned()),
                        cause: "Missing file path".into(),
                    })?;
                    builder = builder.add_source(File::from(Path::new(path)));
                }
                Layer::Config("file?", url, path) => {
                    let path = path.ok_or(ConfigError::FileParse {
                        uri: Some(url.to_owned()),
                        cause: "Missing file path".into(),
                    })?;

                    if Path::new(path).exists() {
                        log::info!("Adding optional config file {}...", path);
                        builder = builder.add_source(File::from(Path::new(path)));
                    }
                }
                Layer::Config("azk", url, path) => {
                    let path = path.ok_or(ConfigError::FileParse {
                        uri: Some(url.to_owned()),
                        cause: "Missing azure keyvault location".into(),
                    })?;
                    if azure_credentials.is_none() {
                        azure_credentials = if let (Some(tenant_id), Some(client_id), Some(client_secret)) = (
                            env::var("AZURE_TENANT_ID").ok(),
                            env::var("AZURE_CLIENT_ID").ok(),
                            env::var("AZURE_CLIENT_SECRET").ok(),
                        ) {
                            let credentials: Arc<dyn TokenCredential> =
                                ClientSecretCredential::new(&tenant_id, client_id, client_secret.into(), None)
                                    .map_err(|err| ConfigError::FileParse {
                                        uri: Some(url.to_owned()),
                                        cause: err.into(),
                                    })?;
                            log::info!("Getting azure credentials through environment...");
                            Some(credentials)
                        } else {
                            log::info!("Getting azure credentials through azure cli...");
                            let credentials: Arc<dyn TokenCredential> =
                                AzureCliCredential::new(None).map_err(|err| ConfigError::FileParse {
                                    uri: Some(url.to_owned()),
                                    cause: err.into(),
                                })?;
                            Some(credentials)
                        };
                    }
                    let azure_credentials = azure_credentials.clone().unwrap();
                    let keyvault_url = format!("https://{}", path);
                    let keyvault = AzureKeyvaultConfigSource::new(azure_credentials.clone(), &keyvault_url)?;
                    builder = builder.add_async_source(keyvault);
                }
                Layer::Config(schema, url, _) => {
                    return Err(ConfigError::FileParse {
                        uri: Some(url.to_owned()),
                        cause: format!("Unsupported schema, {schema}").into(),
                    })
                }
            }
        }

        // these properties cannot be altered wrt the core config
        builder = builder
            .set_override("stage", self.stage.clone())?
            .set_override("version", self.version.clone())?
            .set_override("rootFile", self.root_file.clone())?;

        Ok(builder)
    }
}
