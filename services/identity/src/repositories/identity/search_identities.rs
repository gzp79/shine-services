use crate::models::{Identity, IdentityError};
use std::future::Future;
use uuid::Uuid;

pub const MAX_SEARCH_RESULT_COUNT: usize = 100;

#[derive(Debug)]
pub struct SearchIdentityQuery<'a> {
    pub user_ids: Option<&'a [Uuid]>,
    pub emails: Option<&'a [String]>,
    pub names: Option<&'a [String]>,
    pub count: Option<usize>,
}

/// Search for identities.
pub trait IdentitySearch {
    fn search_identity(
        &mut self,
        search: SearchIdentityQuery<'_>,
    ) -> impl Future<Output = Result<Vec<Identity>, IdentityError>> + Send;
}
