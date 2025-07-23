use crate::ai::{JackknifeConfig, JackknifeFeatures, JackknifePointMath};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "V: Serialize + DeserializeOwned")]
pub struct JackknifeTemplate<V>
where
    V: JackknifePointMath<V>,
{
    features: JackknifeFeatures<V>,
}

impl<V> JackknifeTemplate<V>
where
    V: JackknifePointMath<V>,
{
    pub fn new(features: JackknifeFeatures<V>) -> Self {
        Self { features }
    }

    pub fn from_points(config: &JackknifeConfig, points: &[V]) -> Self {
        let features = JackknifeFeatures::from_points(points, config);
        Self::new(features)
    }

    pub fn features(&self) -> &JackknifeFeatures<V> {
        &self.features
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound = "V: Serialize + DeserializeOwned")]
pub struct JackknifeTemplateSet<V>
where
    V: JackknifePointMath<V>,
{
    config: JackknifeConfig,
    templates: Vec<JackknifeTemplate<V>>,
}

impl<V> JackknifeTemplateSet<V>
where
    V: JackknifePointMath<V>,
{
    pub fn new(config: JackknifeConfig) -> Self {
        Self { config, templates: Vec::new() }
    }

    pub fn templates(&self) -> &[JackknifeTemplate<V>] {
        &self.templates
    }

    pub fn add_template(&mut self, template: JackknifeTemplate<V>) -> &mut Self {
        self.templates.push(template);
        self
    }

    pub fn add_template_from_points(&mut self, points: &[V]) -> &mut Self {
        let template = JackknifeTemplate::from_points(&self.config, points);
        self.add_template(template);
        self
    }
}
