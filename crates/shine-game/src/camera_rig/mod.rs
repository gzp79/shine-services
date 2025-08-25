//! Camera rig module based on https://github.com/h3r2tic/dolly/tree/main

mod driver;
pub use self::driver::*;
mod camera_pose;
pub use self::camera_pose::*;
mod rig;
pub use self::rig::*;
mod rig_plugin;
pub use self::rig_plugin::*;
mod debug_camera_plugin;
pub use self::debug_camera_plugin::*;

pub mod rigs;
