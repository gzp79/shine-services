use crate::models::Identity;

pub struct UserInfo {
    pub identity: Identity,
    pub roles: Vec<String>,
    pub is_linked: bool,
}
