//! Camera rig module based on https://github.com/h3r2tic/dolly/tree/main

mod driver;
pub use self::driver::*;
mod rig;
pub use self::rig::*;

pub mod rigs;
