use crate::{
    db::{IdentityError, SearchIdentity, SearchIdentityOrder},
    services::IdentityServiceState,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
pub(in crate::services) enum Error {
    #[error(transparent)]
    IdentityError(#[from] IdentityError),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status_code = match &self {
            Error::IdentityError(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (status_code, format!("{self:?}")).into_response()
    }
}

#[derive(Deserialize)]
pub(in crate::services) struct RequestQuery {
    count: Option<usize>,
}

pub(in crate::services) async fn ep_search_identity(
    State(state): State<IdentityServiceState>,
    Query(query): Query<RequestQuery>,
    //session: AppSession,
) -> Result<Response, Error> {
    //let session_data = session.g();
    let identities = state
        .identity_manager()
        .search(SearchIdentity {
            order: SearchIdentityOrder::UserId(None),
            count: query.count,
            user_ids: None,
            emails: None,
            names: None,
        })
        .await?;
    log::info!("identities: {:?}", identities);

    Ok(().into_response())
}
