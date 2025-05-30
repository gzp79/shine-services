use crate::telemetry::TelemetryConfig;
use anyhow::{anyhow, Error as AnyError};
use serde::{
    de::{DeserializeOwned, Error as SerdeError, IgnoredAny, MapAccess, Visitor},
    Deserialize, Deserializer,
};
use std::{
    fmt::{self, Debug},
    marker::PhantomData,
    path::PathBuf,
};

use super::{CoreConfig, ServiceConfig};

pub trait FeatureConfig: Debug {
    const NAME: &'static str;
}

#[derive(Debug, Clone)]
pub struct WebAppConfig<F>
where
    F: FeatureConfig,
{
    pub core: CoreConfig,
    pub service: ServiceConfig,
    pub telemetry: TelemetryConfig,

    //#[serde(flatten)]
    pub feature: F,
}

impl<'de, F> Deserialize<'de> for WebAppConfig<F>
where
    F: FeatureConfig + Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct WebAppConfigVisitor<F>
        where
            F: FeatureConfig,
        {
            marker: PhantomData<F>,
        }

        impl<'de, F> Visitor<'de> for WebAppConfigVisitor<F>
        where
            F: FeatureConfig + Deserialize<'de>,
        {
            type Value = WebAppConfig<F>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct WebAppConfig")
            }

            fn visit_map<V>(self, mut map: V) -> Result<WebAppConfig<F>, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut core_stage = None;
                let mut core_version = None;
                let mut core_root_file = None;
                let mut core_before_layers = None;
                let mut core_after_layers = None;

                let mut service = None;
                let mut telemetry = None;
                let mut feature = None;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "stage" => {
                            if core_stage.is_some() {
                                return Err(SerdeError::duplicate_field("stage"));
                            }
                            core_stage = Some(map.next_value()?);
                        }
                        "version" => {
                            if core_version.is_some() {
                                return Err(SerdeError::duplicate_field("version"));
                            }
                            core_version = Some(map.next_value()?);
                        }
                        "rootFile" => {
                            if core_root_file.is_some() {
                                return Err(SerdeError::duplicate_field("rootFile"));
                            }
                            core_root_file = Some(map.next_value()?);
                        }
                        "beforeLayers" => {
                            if core_before_layers.is_some() {
                                return Err(SerdeError::duplicate_field("beforeLayers"));
                            }
                            core_before_layers = Some(map.next_value()?);
                        }
                        "afterLayers" => {
                            if core_after_layers.is_some() {
                                return Err(SerdeError::duplicate_field("afterLayers"));
                            }
                            core_after_layers = Some(map.next_value()?);
                        }
                        "service" => {
                            if service.is_some() {
                                return Err(SerdeError::duplicate_field("service"));
                            }
                            service = Some(map.next_value()?);
                        }
                        "telemetry" => {
                            if telemetry.is_some() {
                                return Err(SerdeError::duplicate_field("telemetry"));
                            }
                            telemetry = Some(map.next_value()?);
                        }
                        _ if key == F::NAME => {
                            if feature.is_some() {
                                return Err(SerdeError::duplicate_field(F::NAME));
                            }
                            feature = Some(map.next_value()?);
                        }
                        _ => {
                            let _: IgnoredAny = map.next_value()?;
                        }
                    }
                }

                let core_stage = core_stage.ok_or_else(|| SerdeError::missing_field("stage"))?;
                let core_version =
                    core_version.ok_or_else(|| SerdeError::missing_field("version"))?;
                let core_root_file =
                    core_root_file.ok_or_else(|| SerdeError::missing_field("rootFile"))?;
                let core_before_layers =
                    core_before_layers.ok_or_else(|| SerdeError::missing_field("beforeLayers"))?;
                let core_after_layers =
                    core_after_layers.ok_or_else(|| SerdeError::missing_field("afterLayers"))?;
                let service = service.ok_or_else(|| SerdeError::missing_field("service"))?;
                let telemetry = telemetry.ok_or_else(|| SerdeError::missing_field("telemetry"))?;
                let feature = feature.ok_or_else(|| SerdeError::missing_field(F::NAME))?;

                Ok(WebAppConfig {
                    core: CoreConfig {
                        stage: core_stage,
                        version: core_version,
                        root_file: core_root_file,
                        before_layers: core_before_layers,
                        after_layers: core_after_layers,
                    },
                    service,
                    telemetry,
                    feature,
                })
            }
        }

        deserializer.deserialize_map(WebAppConfigVisitor {
            marker: PhantomData,
        })
    }
}

impl<F> WebAppConfig<F>
where
    F: FeatureConfig + DeserializeOwned + Debug,
{
    pub async fn load(stage: &str, config_file: Option<PathBuf>) -> Result<Self, AnyError> {
        let pre_init = CoreConfig::new(stage, config_file)?;
        let builder = pre_init.create_config_builder()?;
        let config = builder.build().await?;

        let cfg: Self = config.try_deserialize()?;
        log::info!("Config loaded [{}]: {:#?}", cfg.core.root_file, cfg);

        if pre_init != cfg.core {
            Err(anyhow!("Core config mismatch"))
        } else {
            Ok(cfg)
        }
    }
}
