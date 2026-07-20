use crate::repositories::hub_connections::{HubConnectionError, HubConnections};
use std::future::Future;

pub trait HubConnectionDbContext<'c>: HubConnections + Send {}

pub trait HubConnectionDb: Send + Sync {
    fn create_context(
        &self,
    ) -> impl Future<Output = Result<impl HubConnectionDbContext<'_>, HubConnectionError>> + Send;
}
