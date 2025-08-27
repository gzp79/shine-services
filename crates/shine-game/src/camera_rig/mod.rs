//! Camera rig module based on https://github.com/h3r2tic/dolly/tree/main

mod camera_pose;
mod debug_camera_plugin;
mod driver;
mod rig;
mod rig_plugin;

pub mod rigs;

pub use self::{
    camera_pose::{CameraPose, CameraPoseDebug},
    debug_camera_plugin::DebugCameraTarget,
    driver::{RigDriver, RigUpdateParams},
    rig::CameraRig,
    rig_plugin::CameraRigPlugin,
};
