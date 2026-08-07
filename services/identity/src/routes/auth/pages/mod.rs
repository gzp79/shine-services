mod delete_user;
mod email_login;
mod guest_login;
mod logout;
mod oauth2_auth;
mod oauth2_link;
mod oauth2_login;
mod oidc_auth;
mod oidc_link;
mod oidc_login;
mod router;
mod token_login;
mod validate;

pub use self::router::{oauth2_provider_routes, oidc_provider_routes, page_routes};
