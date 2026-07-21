use crate::repositories::hub_registry::{HubConnectionError, HubRegistry};
use std::future::Future;

pub trait HubConnectionDbContext<'c>: HubRegistry + Send {}

pub trait HubConnectionDb: Send + Sync {
    fn create_context(
        &self,
    ) -> impl Future<Output = Result<impl HubConnectionDbContext<'_>, HubConnectionError>> + Send;
}
