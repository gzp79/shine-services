use crate::models::IdentityError;
use std::future::Future;

pub trait IdSequences {
    fn get_next_id(&mut self) -> impl Future<Output = Result<u64, IdentityError>> + Send;
}
