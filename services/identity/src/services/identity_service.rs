use crate::repositories::identity::{
    ExternalLink, ExternalLinks, ExternalUserInfo, IdSequences, Identities, Identity, IdentityDb, IdentityError,
    IdentitySearch, Roles, SearchIdentity, TokenInfo, TokenKind, Tokens,
};
use chrono::Duration;
use ring::digest;
use shine_infra::{
    crypto::IdEncoder,
    sync::{EventHandler, EventHandlerId, TopicBus, TopicEvent},
    web::extracts::{ClientFingerprint, SiteInfo},
};
use uuid::Uuid;

use super::{IdentityTopic, UserEvent, UserLinkEvent};

pub struct IdentityService<DB>
where
    DB: IdentityDb,
{
    pub db: DB,
    user_name_generator: Box<dyn IdEncoder>,
    event_bus: TopicBus<IdentityTopic>,
}

impl<DB> IdentityService<DB>
where
    DB: IdentityDb,
{
    pub fn new<UE: IdEncoder>(db: DB, user_name_generator: UE) -> Self {
        Self {
            db,
            user_name_generator: Box::new(user_name_generator),
            event_bus: TopicBus::new(),
        }
    }

    pub async fn subscribe<E, H>(&self, handler: H) -> EventHandlerId
    where
        E: TopicEvent<Topic = IdentityTopic>,
        H: EventHandler<E>,
    {
        self.event_bus.subscribe::<E, H>(handler).await
    }

    pub async fn create_user(
        &self,
        user_id: Uuid,
        user_name: &str,
        email: Option<(&str, bool)>,
        external_user_info: Option<&ExternalUserInfo>,
    ) -> Result<Identity, IdentityError> {
        //let email = email.map(|e| e.normalize_email());
        let mut db = self.db.create_context().await?;
        let identity = db.create_user(user_id, user_name, email).await?;
        if let Some(external_user_info) = external_user_info {
            if let Err(err) = db.link_user(user_id, external_user_info).await {
                if let Err(err) = db.cascaded_delete(user_id).await {
                    log::error!("Failed to delete user ({user_id}) after failed link: {err}");
                }
                return Err(err);
            }
        }

        self.event_bus.publish(&UserEvent::Created(user_id)).await;
        if external_user_info.is_some() {
            self.event_bus.publish(&UserLinkEvent::Linked(user_id)).await;
        }

        Ok(identity)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<Identity>, IdentityError> {
        let mut db = self.db.create_context().await?;
        db.find_by_id(id).await
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<Identity>, IdentityError> {
        let mut db = self.db.create_context().await?;
        db.find_by_email(email).await
    }

    pub async fn update(
        &self,
        id: Uuid,
        name: Option<&str>,
        email: Option<(&str, bool)>,
    ) -> Result<Option<Identity>, IdentityError> {
        let mut db = self.db.create_context().await?;
        match db.update(id, name, email).await? {
            Some(identity) => {
                self.event_bus.publish(&UserEvent::Updated(id)).await;
                Ok(Some(identity))
            }
            None => Ok(None),
        }
    }

    pub async fn cascaded_delete(&self, id: Uuid) -> Result<(), IdentityError> {
        let mut db = self.db.create_context().await?;
        db.cascaded_delete(id).await?;
        self.event_bus.publish(&UserEvent::Deleted(id)).await;
        Ok(())
    }

    pub async fn find_by_external_link(
        &self,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<Identity>, IdentityError> {
        let mut db = self.db.create_context().await?;
        db.find_by_external_link(provider, provider_id).await
    }

    pub async fn add_external_link(
        &self,
        user_id: Uuid,
        external_user: &ExternalUserInfo,
    ) -> Result<(), IdentityError> {
        let mut db = self.db.create_context().await?;
        db.link_user(user_id, external_user).await?;
        self.event_bus.publish(&UserLinkEvent::Linked(user_id)).await;
        Ok(())
    }

    pub async fn delete_extern_link(
        &self,
        user_id: Uuid,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<()>, IdentityError> {
        let mut db = self.db.create_context().await?;
        match db.delete_link(user_id, provider, provider_id).await? {
            Some(_) => {
                self.event_bus.publish(&UserLinkEvent::Unlinked(user_id)).await;
                Ok(Some(()))
            }
            None => Ok(None),
        }
    }

    pub async fn is_linked(&self, user_id: Uuid) -> Result<bool, IdentityError> {
        let mut db = self.db.create_context().await?;
        db.is_linked(user_id).await
    }

    pub async fn list_external_links_by_user(&self, user_id: Uuid) -> Result<Vec<ExternalLink>, IdentityError> {
        let mut db = self.db.create_context().await?;
        db.find_all_links(user_id).await
    }

    pub async fn search(&self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, IdentityError> {
        let mut db = self.db.create_context().await?;
        db.search_identity(search).await
    }

    pub async fn add_token(
        &self,
        user_id: Uuid,
        kind: TokenKind,
        token: &str,
        time_to_live: &Duration,
        fingerprint_to_bind_to: Option<&ClientFingerprint>,
        email_to_bind_to: Option<&str>,
        site_info: &SiteInfo,
    ) -> Result<TokenInfo, IdentityError> {
        let mut db = self.db.create_context().await?;
        let token_hash = hash_token(token);
        db.store_token(
            user_id,
            kind,
            &token_hash,
            time_to_live,
            fingerprint_to_bind_to,
            email_to_bind_to,
            site_info,
        )
        .await
    }

    pub async fn find_token_by_hash(&self, token_hash: &str) -> Result<Option<TokenInfo>, IdentityError> {
        let mut db = self.db.create_context().await?;
        db.find_by_hash(token_hash).await
    }

    pub async fn list_all_tokens_by_user(&self, user_id: &Uuid) -> Result<Vec<TokenInfo>, IdentityError> {
        let mut db = self.db.create_context().await?;
        db.find_by_user(user_id).await
    }

    /// Get the identity associated to an access token, but keep the token in the DB.
    pub async fn test_token(
        &self,
        allowed_kind: &[TokenKind],
        token: &str,
    ) -> Result<Option<(Identity, TokenInfo)>, IdentityError> {
        let mut db = self.db.create_context().await?;
        let token_hash = hash_token(token);
        db.test_token(allowed_kind, &token_hash).await
    }

    /// Get the identity associated to an access token and remove the token from the DB.
    pub async fn take_token(
        &self,
        allowed_kind: &[TokenKind],
        token: &str,
    ) -> Result<Option<(Identity, TokenInfo)>, IdentityError> {
        let mut db = self.db.create_context().await?;
        let token_hash = hash_token(token);
        db.take_token(allowed_kind, &token_hash).await
    }

    pub async fn delete_token(&self, kind: TokenKind, token: &str) -> Result<Option<()>, IdentityError> {
        let mut db = self.db.create_context().await?;
        let token_hash = hash_token(token);
        db.delete_token_by_hash(kind, &token_hash).await
    }

    pub async fn delete_hashed_token_by_user(
        &self,
        user_id: Uuid,
        token_hash: &str,
    ) -> Result<Option<()>, IdentityError> {
        let mut db = self.db.create_context().await?;
        db.delete_token_by_user(user_id, token_hash).await
    }

    pub async fn delete_all_tokens_by_user(&self, user_id: Uuid, kinds: &[TokenKind]) -> Result<(), IdentityError> {
        let mut db = self.db.create_context().await?;
        db.delete_all_token_by_user(user_id, kinds).await
    }

    pub async fn delete_terminable_tokens_by_user(&self, user_id: Uuid) -> Result<(), IdentityError> {
        let mut db = self.db.create_context().await?;
        db.delete_terminable_tokens_by_user(user_id).await
    }

    pub async fn add_role(&self, user_id: Uuid, role: &str) -> Result<Option<Vec<String>>, IdentityError> {
        let mut db = self.db.create_context().await?;
        if let Some(roles) = db.add_role(user_id, role).await? {
            self.event_bus.publish(&UserEvent::RoleChange(user_id)).await;
            Ok(Some(roles))
        } else {
            Ok(None)
        }
    }

    pub async fn get_roles(&self, user_id: Uuid) -> Result<Option<Vec<String>>, IdentityError> {
        let mut db = self.db.create_context().await?;
        db.get_roles(user_id).await
    }

    pub async fn delete_role(&self, user_id: Uuid, role: &str) -> Result<Option<Vec<String>>, IdentityError> {
        let mut db = self.db.create_context().await?;
        if let Some(roles) = db.delete_role(user_id, role).await? {
            self.event_bus.publish(&UserEvent::RoleChange(user_id)).await;
            Ok(Some(roles))
        } else {
            Ok(None)
        }
    }

    pub async fn generate_user_name(&self) -> Result<String, IdentityError> {
        // some alternatives and sources:
        // - <https://datatracker.ietf.org/doc/html/rfc1751>
        // - <https://github.com/archer884/harsh>
        // - <https://github.com/pjebs/optimus-go>

        let id = {
            let mut db = self.db.create_context().await?;
            db.get_next_id().await?
        };

        let id = self.user_name_generator.obfuscate(id)?;
        Ok(id)
    }
}

/// Generate a (crypto) hashed version of a token to protect data in rest.
fn hash_token(token: &str) -> String {
    // there is no need for a complex hash as key has a big entropy already
    // and it'd be too expensive to invert the hashing.
    let hash = digest::digest(&digest::SHA256, token.as_bytes());
    let hash = hex::encode(hash);
    log::debug!("Hashing token: {token:?} -> [{hash}]");
    hash
}
