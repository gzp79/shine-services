use crate::tokio::TokioRuntime;
use bevy::app::App;
use std::future::Future;
use tokio::runtime::Handle;

/// Extension trait for Bevy App to access the Tokio runtime.
pub trait TokeAppExt {
    fn get_tokio_runtime(&self) -> Handle;

    fn spawn_tokio_task<Fut, S>(&self, future_fn: S)
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
        S: FnOnce() -> Fut + Send + 'static;
}

impl TokeAppExt for App {
    fn get_tokio_runtime(&self) -> Handle {
        self.world()
            .get_resource::<TokioRuntime>()
            .expect("Missing TokioPlugin")
            .runtime
            .handle()
            .clone()
    }

    fn spawn_tokio_task<Fut, S>(&self, future_fn: S)
    where
        Fut: Future + Send + 'static,
        Fut::Output: Send + 'static,
        S: FnOnce() -> Fut + Send + 'static,
    {
        let handle = self.get_tokio_runtime();
        handle.spawn(future_fn());
    }
}
