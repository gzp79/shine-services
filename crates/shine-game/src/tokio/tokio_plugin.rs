use crate::tokio::TokioRuntime;
use bevy::app::{App, Plugin};
use tokio::runtime::Builder;

pub struct TokioPlugin;

impl Plugin for TokioPlugin {
    fn build(&self, app: &mut App) {
        let runtime = {
            #[cfg(any(
                target_os = "windows",
                target_os = "linux",
                target_os = "macos",
                target_os = "android"
            ))]
            let mut runtime = Builder::new_multi_thread();
            #[cfg(target_family = "wasm")]
            let mut runtime = Builder::new_current_thread();

            runtime.enable_all();
            runtime
                .build()
                .expect("Failed to create Tokio runtime for background tasks")
        };

        app.insert_resource(TokioRuntime { runtime });
    }
}
