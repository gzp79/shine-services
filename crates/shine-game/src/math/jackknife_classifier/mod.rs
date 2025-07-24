#![allow(clippy::module_inception)]

//! Jackknife Classifier Module based on Jackknife: A Reliable Recognizer with Few Samples and Many Modalities
//! see: <https://github.com/ISUE/Jackknife>

mod jackknife_math;
pub use self::jackknife_math::*;
mod jackknife_dtw;
mod jackknife_math_array_n;
mod jackknife_math_vec2;
mod jackknife_math_vec3;
pub use self::jackknife_dtw::*;

mod jackknife_error;
pub use self::jackknife_error::*;
mod jackknife_config;
pub use self::jackknife_config::*;
mod jackknife_features;
pub use self::jackknife_features::*;
mod jackknife_template;
pub use self::jackknife_template::*;
mod jackknife_classifier;
pub use self::jackknife_classifier::*;
