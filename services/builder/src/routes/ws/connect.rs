use crate::{
    app_state::AppState,
    routes::ws::message::{RequestMessage, ResponseMessage},
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
use shine_infra::{
    session::{CheckedCurrentUser, CurrentUser, CurrentUserService},
    web::{
        extracts::{Origin, TargetHost, ValidatedPath},
        responses::{IntoProblemResponse, Problem, ProblemConfig, ProblemResponse},
    },
};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use utoipa::IntoParams;
use uuid::Uuid;
use validator::Validate;

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
    Extension(session_service): Extension<Arc<CurrentUserService>>,
    ValidatedPath(path): ValidatedPath<PathParams>,
    TargetHost(target_host): TargetHost,
    Origin(origin): Origin,
    user: CheckedCurrentUser,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, ProblemResponse> {
    if !state.settings().ws.is_allowed_host(target_host.as_str()) {
        return Err(Problem::forbidden()
            .with_detail("host is not allowed")
            .into_response(&problem_config));
    }

    if !state.settings().ws.is_allowed_origin(origin.as_str()) {
        return Err(Problem::forbidden()
            .with_detail("origin is not allowed")
            .into_response(&problem_config));
    }

    let auth_check_interval = state.settings().ws.auth_check_interval;

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
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, user, session, session_service, auth_check_interval)))
}

async fn handle_socket(
    socket: WebSocket,
    user: CurrentUser,
    session: Arc<Session>,
    session_service: Arc<CurrentUserService>,
    auth_check_interval: Duration,
) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let current_user_id = user.user_id;
    let session_key = user.key;
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
                        log::error!("[{current_user_id}] Failed to serialize message {msg:#?} with error {err:#?}");
                        continue;
                    }
                };
                if let Err(err) = ws_sender.send(WsMessage::Text(data.into())).await {
                    log::error!("[{current_user_id}] Failed to send message to the user: {err:#?}");
                }
            }
        }
    });

    let mut auth_task = tokio::spawn(async move {
        let mut interval = time::interval(auth_check_interval);
        interval.tick().await; // skip the first immediate tick
        loop {
            interval.tick().await;
            if session_service
                .get_current_user(current_user_id, session_key)
                .await
                .is_err()
            {
                log::info!("[{current_user_id}] Session expired, closing WebSocket connection");
                break;
            }
        }
    });

    // If any one of the tasks exits, abort the others.
    tokio::select! {
        rv_a = (&mut send_task) => {
            log::info!("Send task exited: {rv_a:?}");
            recv_task.abort();
            auth_task.abort();
        },
        rv_b = (&mut recv_task) => {
            log::info!("Receive task exited: {rv_b:?}");
            send_task.abort();
            auth_task.abort();
        },
        _ = (&mut auth_task) => {
            log::info!("[{current_user_id}] Auth task exited, dropping connection");
            send_task.abort();
            recv_task.abort();
        }
    }

    session.disconnect_user(current_user_id).await;
    message_sender.send(Message::Chat(current_user_id, "${tr: Disconnected}".to_string()));
}
