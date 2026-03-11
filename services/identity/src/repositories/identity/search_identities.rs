use crate::models::{Identity, IdentityError, SearchIdentity};
use std::future::Future;

/// Search for identities.
pub trait IdentitySearch {
    fn search_identity(
        &mut self,
        search: SearchIdentity<'_>,
    ) -> impl Future<Output = Result<Vec<Identity>, IdentityError>> + Send;
}
