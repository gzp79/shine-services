use crate::{
    repositories::identity::{
        ExternalLinks, ExternalUserInfo, IdSequences, Identities, Identity, IdentityDb, IdentityError, IdentitySearch,
        SearchIdentity,
    },
    services::{IdentityTopic, UserEvent, UserLinkEvent},
};
use shine_infra::{crypto::IdEncoder, sync::TopicBus};
use std::sync::Arc;
use thiserror::Error as ThisError;
use uuid::Uuid;
use validator::ValidateEmail;

#[derive(Debug, ThisError)]
pub enum CreateUserError {
    #[error("Retry limit reached for user creation")]
    RetryLimitReached,
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

pub struct UserService<DB: IdentityDb> {
    db: DB,
    name_generator: Box<dyn IdEncoder>,
    events: Arc<TopicBus<IdentityTopic>>,
}

impl<DB: IdentityDb> UserService<DB> {
    pub fn new<UE: IdEncoder>(db: DB, name_generator: UE, events: Arc<TopicBus<IdentityTopic>>) -> Self {
        Self {
            db,
            name_generator: Box::new(name_generator),
            events,
        }
    }

    pub async fn create(&self, id: Uuid, name: &str, email: Option<(&str, bool)>) -> Result<Identity, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        let identity = ctx.create_user(id, name, email).await?;
        drop(ctx);

        self.events.publish(&UserEvent::Created(identity.id)).await;
        Ok(identity)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Identity>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.find_by_id(id).await
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<Identity>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.find_by_email(email).await
    }

    pub async fn update(
        &self,
        id: Uuid,
        name: Option<&str>,
        email: Option<(&str, bool)>,
    ) -> Result<Option<Identity>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        match ctx.update(id, name, email).await? {
            Some(identity) => {
                self.events.publish(&UserEvent::Updated(id)).await;
                Ok(Some(identity))
            }
            None => Ok(None),
        }
    }

    pub async fn delete(&self, id: Uuid) -> Result<(), IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.cascaded_delete(id).await?;
        self.events.publish(&UserEvent::Deleted(id)).await;
        Ok(())
    }

    pub async fn search(&self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, IdentityError> {
        let mut ctx = self.db.create_context().await?;
        ctx.search_identity(search).await
    }

    pub async fn generate_name(&self) -> Result<String, IdentityError> {
        let id = {
            let mut ctx = self.db.create_context().await?;
            ctx.get_next_id().await?
        };

        let id = self.name_generator.obfuscate(id)?;
        Ok(id)
    }

    pub async fn create_with_retry(
        &self,
        name: Option<&str>,
        email: Option<&str>,
    ) -> Result<Identity, CreateUserError> {
        const MAX_RETRY_COUNT: usize = 10;

        let mut name = name.map(|e| e.to_owned());
        let email = email.filter(|email| email.validate_email()).map(|email| (email, false));

        let mut retry_count = 0;
        loop {
            log::debug!("Creating new user; retry: {retry_count:#?}");
            if retry_count > MAX_RETRY_COUNT {
                return Err(CreateUserError::RetryLimitReached);
            }
            retry_count += 1;

            let user_id = Uuid::new_v4();
            let user_name = match name.take() {
                Some(name) => name,
                None => self.generate_name().await?,
            };

            match self.create(user_id, &user_name, email).await {
                Ok(identity) => return Ok(identity),
                Err(IdentityError::NameConflict) => continue,
                Err(IdentityError::UserIdConflict) => continue,
                Err(err) => return Err(CreateUserError::IdentityError(err)),
            }
        }
    }

    pub async fn create_linked_user(
        &self,
        user_id: Uuid,
        name: &str,
        external_user: &ExternalUserInfo,
    ) -> Result<Identity, IdentityError> {
        let mut ctx = self.db.create_context().await?;

        let identity = ctx.create_user(user_id, name, None).await?;

        // Rollback on link failure
        if let Err(err) = ctx.link_user(user_id, external_user).await {
            if let Err(del_err) = ctx.cascaded_delete(user_id).await {
                log::error!("Failed to delete user ({user_id}) after failed link: {del_err}");
            }
            return Err(err);
        }

        drop(ctx);

        self.events.publish(&UserEvent::Created(user_id)).await;
        self.events.publish(&UserLinkEvent::Linked(user_id)).await;

        Ok(identity)
    }
}
