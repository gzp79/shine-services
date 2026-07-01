#![allow(clippy::module_inception)]

mod email;
mod normalizer;

pub use crate::email::{email::Email, normalizer::NORM_EMAIL_VERSION};
