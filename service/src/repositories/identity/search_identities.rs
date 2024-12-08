use uuid::Uuid;

use super::{Identity, IdentityError};

pub const MAX_SEARCH_RESULT_COUNT: usize = 100;

#[derive(Debug)]
pub enum SearchIdentityOrder {
    UserId(Option<Uuid>),
    Email(Option<(String, Uuid)>),
    Name(Option<(String, Uuid)>),
}

#[derive(Debug)]
pub struct SearchIdentity<'a> {
    pub order: SearchIdentityOrder,
    pub count: Option<usize>,

    pub user_ids: Option<&'a [Uuid]>,
    pub emails: Option<&'a [String]>,
    pub names: Option<&'a [String]>,
}

/// Search for identities.
pub trait IdentitySearch {
    async fn search_identity(&mut self, search: SearchIdentity<'_>) -> Result<Vec<Identity>, IdentityError>;
}
