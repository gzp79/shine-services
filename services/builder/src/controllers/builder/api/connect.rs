use axum::{
    extract::{
        ws::{Message as WsMessage, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    Extension,
};
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use shine_core::web::{CheckedCurrentUser, CurrentUser, IntoProblem, Problem, ProblemConfig, ValidatedPath};
use std::sync::Arc;
use utoipa::IntoParams;
use uuid::Uuid;
use validator::Validate;

use crate::{
    app_state::AppState,
    repositories::{Message, Session},
};

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PathParams {
    #[serde(rename = "id")]
    session_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/api/connect/:id",
    tag = "builder",
    params (
        PathParams
    ),
    responses(
        (status = OK)
    )
)]
pub async fn connect(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    ValidatedPath(path): ValidatedPath<PathParams>,
    user: CheckedCurrentUser,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, Problem> {
    let user = user.into_user();
    log::info!(
        "User {} requesting a connection to the session {}...",
        path.session_id,
        user.user_id
    );

    let session = state
        .sessions()
        .acquire_session(&path.session_id, &user.user_id)
        .await
        .map_err(|err| err.into_problem(&problem_config))?;
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, user, session)))
}

async fn handle_socket(socket: WebSocket, user: CurrentUser, session: Arc<Session>) {
    let (message_sender, mut message_receiver) = session.message_channel();
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let current_user_id = user.user_id;
    let session_id = session.id();

    log::info!("[{current_user_id}] Connected to the session {session_id}");

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(message)) = ws_receiver.next().await {
            log::info!("[{current_user_id}] WsMessage received");
            match message {
                WsMessage::Text(text) => {
                    if let Err(err) = message_sender.send(Message::Chat(current_user_id, text)) {
                        log::error!(
                            "[{current_user_id}] Failed to enqueue message to the session: {:#?}",
                            err
                        );
                    }
                }
                _ => {}
            }
        }
    });

    let mut send_task = tokio::spawn(async move {
        while let Ok(message) = message_receiver.recv().await {
            log::info!("[{current_user_id}] Message received");
            match message {
                Message::Chat(user_id, text) => {
                    if user_id == current_user_id {
                        continue;
                    }
                    if let Err(err) = ws_sender.send(WsMessage::Text(text)).await {
                        log::error!("[{current_user_id}] Failed to send message to the user: {:#?}", err);
                    }
                }
                msg => {
                    log::error!("[{current_user_id}] Unknown message received: {:#?}", msg);
                }
            }
        }
    });

    // If any one of the tasks exit, abort the other.
    tokio::select! {
        rv_a = (&mut send_task) => {
            log::info!("Send task exited: {rv_a:?}");
            recv_task.abort();
        },
        rv_b = (&mut recv_task) => {
            log::info!("Receive task exited: {rv_b:?}");
            send_task.abort();
        }
    }

    session.disconnect_user(current_user_id).await;
}
