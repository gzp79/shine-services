use serde::Deserialize;
use shine_infra::web::{Environment, FeatureConfig, WebAppConfig};
use shine_test::test;
use std::env;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    file_value: String,
    env_value: Option<String>,
    override_value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Feature {
    some_data: Data,
}

impl FeatureConfig for Feature {
    const NAME: &'static str = "feature";
}

const CONFIG_ROOT: &str = "./tests/web_configs";

#[test]
async fn test_config_with_env() {
    env::set_var("SHINE--OVERRIDE_VALUE", "from env");
    env::set_var("SHINE--ENV_VALUE", "from env");

    let cfg = config::Config::builder()
        .add_source(config::File::from_str(
            r#"{"fileValue": "from file", "overrideValue": "from file"}"#,
            config::FileFormat::Json,
        ))
        .add_source(Environment::new())
        .build()
        .unwrap();
    log::info!("{cfg:#?}");
    let cfg = cfg.try_deserialize::<Data>().unwrap();
    log::debug!("{cfg:#?}");

    assert_eq!(cfg.file_value, "from file");
    assert_eq!(cfg.env_value.as_deref(), Some("from env"));
    assert_eq!(cfg.override_value, "from env");
}

#[test]
async fn test_web_config_with_env() {
    env::set_var("SHINE--SERVICE--CAPTCHA_SECRET", "from env");
    env::set_var("SHINE--FEATURE--SOME_DATA--OVERRIDE_VALUE", "from env");
    env::set_var("SHINE--FEATURE--SOME_DATA--ENV_VALUE", "from env");

    let config = WebAppConfig::<Feature>::load("dev", Some(format!("{CONFIG_ROOT}/env.json").into()))
        .await
        .unwrap();
    assert_eq!(config.service.captcha_secret, "from env");
    assert_eq!(config.feature.some_data.file_value, "from file");
    assert_eq!(config.feature.some_data.override_value, "from env");
    assert_eq!(config.feature.some_data.env_value.as_deref(), Some("from env"));
}
