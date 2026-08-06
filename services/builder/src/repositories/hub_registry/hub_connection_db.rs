use crate::{models::HubError, repositories::hub_registry::HubRegistry};
use std::future::Future;

pub trait HubConnectionDbContext<'c>: HubRegistry + Send {}

pub trait HubConnectionDb: Send + Sync {
    fn create_context(&self) -> impl Future<Output = Result<impl HubConnectionDbContext<'_>, HubError>> + Send;
}
