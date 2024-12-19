use std::future::Future;

use super::IdentityError;

pub trait IdSequences {
    fn get_next_id(&mut self) -> impl Future<Output = Result<u64, IdentityError>> + Send;
}
