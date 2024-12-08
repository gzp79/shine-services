use super::IdentityError;

pub trait IdSequences {
    async fn get_next_id(&mut self) -> Result<u64, IdentityError>;
}
