use crate::repositories::hub_connections::HubConnectionError;
use serde::{Deserialize, Serialize};
use shine_infra::session::SessionKey;
use std::future::Future;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HubConnection {
    pub user_id: Uuid,
    pub connection_id: Uuid,
    #[serde(with = "shine_infra::session::serde_session_key")]
    pub session_key: SessionKey,
}

impl HubConnection {
    pub fn new(user_id: Uuid, session_key: SessionKey) -> Self {
        Self {
            user_id,
            connection_id: Uuid::new_v4(),
            session_key,
        }
    }
}

pub trait HubConnections {
    fn list_connections(&mut self) -> impl Future<Output = Result<Vec<HubConnection>, HubConnectionError>> + Send;

    fn create_connection(
        &mut self,
        user_id: Uuid,
        session_key: SessionKey,
    ) -> impl Future<Output = Result<HubConnection, HubConnectionError>> + Send;

    fn find_connection_by_user(
        &mut self,
        user_id: Uuid,
    ) -> impl Future<Output = Result<Option<HubConnection>, HubConnectionError>> + Send;
}
