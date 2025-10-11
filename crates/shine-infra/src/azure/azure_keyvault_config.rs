use async_trait::async_trait;
use azure_core::credentials::TokenCredential;
use azure_security_keyvault_secrets::SecretClient;
use config::{
    AsyncSource as ConfigAsyncSource, ConfigError, Map as ConfigMap, Value as ConfigValue, ValueKind as ConfigValueKind,
};
use core::fmt;
use futures::TryStreamExt;
use std::sync::Arc;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
#[error("Azure core error: {0}")]
pub struct AzureKeyvaultConfigError(#[source] azure_core::Error);

impl From<AzureKeyvaultConfigError> for ConfigError {
    fn from(err: AzureKeyvaultConfigError) -> Self {
        log::error!("{err:?}");
        ConfigError::Foreign(Box::new(err))
    }
}

#[derive(Clone)]
pub struct AzureKeyvaultConfigSource {
    keyvault_url: String,
    client: Arc<SecretClient>,
}

impl fmt::Debug for AzureKeyvaultConfigSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AzureKeyvaultConfigSource").finish()
    }
}

impl AzureKeyvaultConfigSource {
    pub fn new(
        azure_credentials: Arc<dyn TokenCredential>,
        keyvault_url: &str,
    ) -> Result<AzureKeyvaultConfigSource, ConfigError> {
        let client = SecretClient::new(keyvault_url, azure_credentials, None).map_err(AzureKeyvaultConfigError)?;
        Ok(Self {
            keyvault_url: keyvault_url.to_owned(),
            client: Arc::new(client),
        })
    }
}

#[async_trait]
impl ConfigAsyncSource for AzureKeyvaultConfigSource {
    async fn collect(&self) -> Result<ConfigMap<String, ConfigValue>, ConfigError> {
        let mut config = ConfigMap::new();

        log::info!("Loading secrets from {} ...", self.keyvault_url);
        let origin = self.keyvault_url.to_string();
        let mut stream = self
            .client
            .list_secret_properties(None)
            .map_err(AzureKeyvaultConfigError)?
            .into_stream();
        while let Some(secret) = stream.try_next().await.map_err(AzureKeyvaultConfigError)? {
            if let Some(id) = secret.id {
                let key = id.split('/').next_back();
                if let Some(key) = key {
                    let path = key.replace('-', ".");
                    log::info!("Reading secret {key:?}");
                    let secret = self
                        .client
                        .get_secret(key, None)
                        .await
                        .map_err(AzureKeyvaultConfigError)?
                        .into_body()
                        .await
                        .map_err(AzureKeyvaultConfigError)?;
                    if let (Some(attributes), Some(value)) = (secret.attributes, secret.value) {
                        if attributes.enabled.unwrap_or(false) {
                            // try to parse value, as conversion from string to a concrete type is not automatic.
                            let value = if let Ok(parsed) = value.parse::<i64>() {
                                ConfigValueKind::I64(parsed)
                            } else {
                                ConfigValueKind::String(value)
                            };

                            config.insert(path, ConfigValue::new(Some(&origin), value));
                        }
                    }
                }
            }
        }

        //log::info!("keyvault config: {:#?}", config);
        Ok(config)
    }
}
