use crate::{
    app_state::AppState,
    services::{Message, MessageSource, Session},
};
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
use shine_infra::web::{
    extracts::ValidatedPath,
    responses::{IntoProblemResponse, ProblemConfig, ProblemResponse},
    session::{CheckedCurrentUser, CurrentUser},
};
use std::sync::Arc;
use utoipa::IntoParams;
use uuid::Uuid;
use validator::Validate;

use super::{RequestMessage, ResponseMessage};

#[derive(Deserialize, Validate, IntoParams)]
#[serde(rename_all = "camelCase")]
pub struct PathParams {
    #[serde(rename = "id")]
    session_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/api/connect/{id}",
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
) -> Result<impl IntoResponse, ProblemResponse> {
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
        .map_err(|err| err.into_response(&problem_config))?;
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, user, session)))
}

async fn handle_socket(socket: WebSocket, user: CurrentUser, session: Arc<Session>) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let current_user_id = user.user_id;
    let session_id = session.id();

    log::info!("[{current_user_id}] Connected to the session {session_id}");
    let message_sender = session.message_sender(MessageSource::User(current_user_id));
    let mut message_receiver = session.subscribe_messages();

    let mut recv_task = {
        let message_sender = message_sender.clone();
        tokio::spawn(async move {
            message_sender.send(Message::Chat(current_user_id, "${tr: Connected}".to_string()));

            while let Some(Ok(message)) = ws_receiver.next().await {
                log::info!("[{current_user_id}] WsMessage received");
                match message {
                    WsMessage::Text(text) => {
                        let msg = match serde_json::from_str::<RequestMessage>(&text) {
                            Ok(msg) => match msg {
                                RequestMessage::Chat { text } => Some(Message::Chat(current_user_id, text)),
                            },
                            Err(_) => {
                                log::error!("[{current_user_id}] Received invalid message: {text}");
                                None
                            }
                        };

                        if let Some(msg) = msg {
                            message_sender.send(msg);
                        }
                    }
                    _ => {}
                }
            }
        })
    };

    let mut send_task = tokio::spawn(async move {
        while let Ok(message) = message_receiver.recv().await {
            log::info!("[{current_user_id}] Message received");
            let msg = match message {
                Message::Chat(user_id, text) => Some(ResponseMessage::Chat { from: user_id, text }),
            };

            if let Some(msg) = msg {
                let data = match serde_json::to_string(&msg) {
                    Ok(data) => data,
                    Err(err) => {
                        log::error!(
                            "[{current_user_id}] Failed to serialize message {:#?} with error {:#?}",
                            msg,
                            err
                        );
                        continue;
                    }
                };
                if let Err(err) = ws_sender.send(WsMessage::Text(data.into())).await {
                    log::error!("[{current_user_id}] Failed to send message to the user: {:#?}", err);
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
    message_sender.send(Message::Chat(current_user_id, "${tr: Disconnected}".to_string()));
}
