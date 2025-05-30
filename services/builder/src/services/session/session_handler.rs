use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::{Session, SessionError};

pub struct SessionHandler {
    sessions: RwLock<HashMap<Uuid, Arc<Session>>>,
}

impl SessionHandler {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    pub async fn acquire_session(
        &self,
        session_id: &Uuid,
        user_id: &Uuid,
    ) -> Result<Arc<Session>, SessionError> {
        let mut sessions = self.sessions.write().await;
        let session = sessions
            .entry(*session_id)
            .or_insert_with(|| Arc::new(Session::new(*session_id)));

        session.connect_user(*user_id).await?;

        Ok(session.clone())
    }
}
