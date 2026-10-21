use crate::{
    app_state::AppState, models::CommandStreamError, routes::ws::stream_frame, services::CommandStreamService,
};
use axum::{
    extract::{Path, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    Extension, Json,
};
use shine_game::replay::protocol::ServerFrame;
use shine_infra::{
    db::command_log::StreamId,
    session::CheckedCurrentUser,
    web::responses::{problems, IntoProblemResponse, Problem, ProblemConfig, ProblemResponse},
};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

pub fn stream_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(snapshot))
}

fn problem_of(err: &CommandStreamError) -> Problem {
    match err {
        CommandStreamError::StreamNotFound => Problem::not_found(),
        CommandStreamError::Forbidden => Problem::forbidden(),
        CommandStreamError::NotReady | CommandStreamError::Internal { .. } => Problem::service_unavailable(),
    }
}

/// The state of a stream as a `streamSync` frame.
async fn snapshot_frame(
    streams: Option<&CommandStreamService>,
    user_id: Uuid,
    stream_id: &str,
) -> Result<ServerFrame, Problem> {
    let stream_id = StreamId::parse(stream_id)
        .map_err(|_| Problem::bad_request(problems::INPUT_PATH).with_detail("invalid stream id"))?;
    let streams = streams.ok_or_else(|| Problem::service_unavailable().with_detail("command streams are disabled"))?;
    match streams.read_snapshot(user_id, &stream_id).await {
        Ok(message) => Ok(stream_frame(message)),
        Err(err) => {
            if matches!(err, CommandStreamError::Internal { .. }) {
                tracing::warn!("[{user_id}] Failed to read the snapshot of {stream_id}: {err:#?}");
            }
            Err(problem_of(&err))
        }
    }
}

/// The state of a stream, built from storage: the same payload as `streamSync`, to start from before (or
/// without) a socket and then subscribe with `resume`.
#[utoipa::path(
    get,
    path = "/api/streams/{stream_id}/snapshot",
    tag = "builder",
    params(("stream_id" = String, Path, description = "The stream id")),
    responses(
        (status = OK),
        (status = NOT_FOUND),
        (status = SERVICE_UNAVAILABLE)
    )
)]
pub async fn snapshot(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    user: CheckedCurrentUser,
    Path(stream_id): Path<String>,
) -> Result<Response, ProblemResponse> {
    let user = user.into_user();
    let frame = snapshot_frame(state.command_stream_service(), user.user_id, &stream_id)
        .await
        .map_err(|problem| problem.into_response(&problem_config))?;
    let mut response = Json(frame).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[cfg(test)]
#[path = "command_stream_test.rs"]
mod command_stream_test;
