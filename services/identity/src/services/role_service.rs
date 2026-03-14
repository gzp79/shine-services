use crate::{
    models::IdentityError,
    repositories::identity::{IdentityDb, Roles},
    services::{IdentityTopic, UserEvent},
};
use shine_infra::sync::TopicBus;
use std::sync::Arc;
use uuid::Uuid;

pub struct RoleService<DB: IdentityDb> {
    db: DB,
    events: Arc<TopicBus<IdentityTopic>>,
}

impl<DB: IdentityDb> RoleService<DB> {
    pub fn new(db: DB, events: Arc<TopicBus<IdentityTopic>>) -> Self {
        Self { db, events }
    }

    pub async fn add_role(&self, user_id: Uuid, role: &str) -> Result<Option<Vec<String>>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        if let Some(roles) = ctx.add_role(user_id, role).await? {
            self.events.publish(&UserEvent::RoleChange(user_id)).await;
            Ok(Some(roles))
        } else {
            Ok(None)
        }
    }

    pub async fn get_roles(&self, user_id: Uuid) -> Result<Option<Vec<String>>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.get_roles(user_id).await
    }

    pub async fn delete_role(&self, user_id: Uuid, role: &str) -> Result<Option<Vec<String>>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        if let Some(roles) = ctx.delete_role(user_id, role).await? {
            self.events.publish(&UserEvent::RoleChange(user_id)).await;
            Ok(Some(roles))
        } else {
            Ok(None)
        }
    }
}
