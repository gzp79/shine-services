use crate::{
    repositories::{
        identity::{Identity, IdentityDb, IdentityError, TokenInfo, TokenKind},
        session::SessionDb,
    },
    services::{LinkService, RoleService, SessionService, TokenService, UserService},
};
use shine_infra::web::{
    extracts::{ClientFingerprint, SiteInfo},
    session::SessionKey,
};
use uuid::Uuid;

pub struct AuthResult {
    pub identity: Identity,
    pub token_info: TokenInfo,
    pub create_access_token: bool,
    pub rotated_token: Option<String>,
}

pub struct AuthHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    user_service: &'a UserService<IDB>,
    token_service: &'a TokenService<IDB>,
    role_service: &'a RoleService<IDB>,
    link_service: &'a LinkService<IDB>,
    session_service: &'a SessionService<SDB>,
}

impl<'a, IDB, SDB> AuthHandler<'a, IDB, SDB>
where
    IDB: IdentityDb,
    SDB: SessionDb,
{
    pub fn new(
        user_service: &'a UserService<IDB>,
        token_service: &'a TokenService<IDB>,
        role_service: &'a RoleService<IDB>,
        link_service: &'a LinkService<IDB>,
        session_service: &'a SessionService<SDB>,
    ) -> Self {
        Self {
            user_service,
            token_service,
            role_service,
            link_service,
            session_service,
        }
    }
}
