use crate::{app_state::AppState, models::Identity, repositories::identity::IdentityDb, services::UserService};
use uuid::Uuid;

pub const MAX_SEARCH_RESULT_COUNT: usize = 100;

#[derive(Debug)]
pub struct IdentitySearchQuery<'a> {
    pub user_ids: Option<&'a [Uuid]>,
    pub emails: Option<&'a [String]>,
    pub names: Option<&'a [String]>,
    pub count: Option<usize>,
}

pub struct IdentitySearchResult {
    pub identities: Vec<Identity>,
    pub is_partial: bool,
}

pub struct IdentitySearchHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    user_service: &'a UserService<IDB>,
}

impl<'a, IDB> IdentitySearchHandler<'a, IDB>
where
    IDB: IdentityDb,
{
    pub fn new(user_service: &'a UserService<IDB>) -> Self {
        Self { user_service }
    }

    pub async fn search_identities(
        &self,
        query: IdentitySearchQuery<'_>,
    ) -> Result<IdentitySearchResult, crate::models::IdentityError> {
        let count = query
            .count
            .unwrap_or(MAX_SEARCH_RESULT_COUNT)
            .min(MAX_SEARCH_RESULT_COUNT);

        let mut identities = self
            .user_service
            .search(query.user_ids, query.emails, query.names, Some(count + 1))
            .await?;

        let is_partial = identities.len() > count;
        identities.truncate(count);

        Ok(IdentitySearchResult { identities, is_partial })
    }
}

impl AppState {
    pub fn identity_search_handler(&self) -> IdentitySearchHandler<'_, impl IdentityDb> {
        IdentitySearchHandler::new(self.user_service())
    }
}
