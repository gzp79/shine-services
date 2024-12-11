use std::future::Future;

//todo: workaround as of  https://github.com/rust-lang/rust/issues/100013#issuecomment-1941232513
pub fn gat_fix<R>(f: impl Future<Output = R> + Send) -> impl Future<Output = R> + Send {
    f
}
