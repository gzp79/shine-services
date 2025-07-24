use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JackknifeMethod {
    /// Use inner product as a measure.
    InnerProduct,

    /// Use euclidean distance as a measure.
    EuclideanDistance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JackknifeConfig {
    pub resample_count: usize,
    pub dtw_radius: usize,
    pub method: JackknifeMethod,

    pub z_normalize: bool,
    pub abs_correction: bool,
    pub extent_correction: bool,
    pub use_lower_bound: bool,
}

impl JackknifeConfig {
    pub fn inner_product() -> Self {
        Self {
            resample_count: 16,
            dtw_radius: 2,
            method: JackknifeMethod::InnerProduct,
            z_normalize: false,
            abs_correction: true,
            extent_correction: true,
            use_lower_bound: true,
        }
    }
}
