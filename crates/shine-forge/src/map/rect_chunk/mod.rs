#![allow(clippy::module_inception)]

mod rect_coord;
pub use self::rect_coord::*;

mod rect_config;
pub use self::rect_config::*;
mod rect_chunk;
pub use self::rect_chunk::*;
mod rect_dense;
pub use self::rect_dense::*;
mod rect_sparse;
pub use self::rect_sparse::*;

pub mod rect_plugin;
pub use self::rect_plugin::*;
