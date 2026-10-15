/// Logs the pool-internal connection errors (background connects, failed validation) that bb8
/// otherwise drops; a checkout would only report a timeout.
#[derive(Debug, Clone, Copy)]
pub struct PoolErrorLogger(pub &'static str);

impl<E: std::fmt::Display> bb8::ErrorSink<E> for PoolErrorLogger {
    fn sink(&self, error: E) {
        tracing::warn!("{} pool connection error: {error}", self.0);
    }

    fn boxed_clone(&self) -> Box<dyn bb8::ErrorSink<E>> {
        Box::new(*self)
    }
}
