#![allow(clippy::module_inception)]

mod animation;
pub use self::animation::*;

mod identity_animation;
pub use self::identity_animation::*;
mod time_animation;
pub use self::time_animation::*;
mod smooth_animation;
pub use self::smooth_animation::*;
mod predict_animation;
pub use self::predict_animation::*;
mod map_animation;
pub use self::map_animation::*;
mod curve_animation;
pub use self::curve_animation::*;
