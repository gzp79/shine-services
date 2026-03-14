use std::sync::Arc;

use shine_infra::sync::TopicBus;
use uuid::Uuid;

use crate::{
    models::{ExternalLink, ExternalUserInfo, Identity, IdentityError},
    repositories::identity::{ExternalLinks, IdentityDb},
    services::{IdentityTopic, UserLinkEvent},
};

pub struct LinkService<DB: IdentityDb> {
    db: DB,
    events: Arc<TopicBus<IdentityTopic>>,
}

impl<DB: IdentityDb> LinkService<DB> {
    pub fn new(db: DB, events: Arc<TopicBus<IdentityTopic>>) -> Self {
        Self { db, events }
    }

    pub async fn find_by_external_link(
        &self,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<Identity>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.find_by_external_link(provider, provider_id).await
    }

    pub async fn add_external_link(
        &self,
        user_id: Uuid,
        external_user: &ExternalUserInfo,
    ) -> Result<(), IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.link_user(user_id, external_user).await?;
        self.events.publish(&UserLinkEvent::Linked(user_id)).await;
        Ok(())
    }

    pub async fn delete_extern_link(
        &self,
        user_id: Uuid,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<()>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        match ctx.delete_link(user_id, provider, provider_id).await? {
            Some(_) => {
                self.events.publish(&UserLinkEvent::Unlinked(user_id)).await;
                Ok(Some(()))
            }
            None => Ok(None),
        }
    }

    pub async fn is_linked(&self, user_id: Uuid) -> Result<bool, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.is_linked(user_id).await
    }

    pub async fn list_external_links_by_user(&self, user_id: Uuid) -> Result<Vec<ExternalLink>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.find_all_links(user_id).await
    }
}
