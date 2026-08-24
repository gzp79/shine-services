use crate::{
    app_state::AppState,
    models::messages::{HubEvent, HubMessage, TopicKey},
    routes::ws::message::{ChatWireComment, WSMessageRequest, WSMessageResponse},
    services::{ChatService, HubService},
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
use shine_infra::{
    session::{CheckedCurrentUser, CurrentUser},
    web::{
        extracts::{Origin, TargetHost},
        responses::{IntoProblemResponse, Problem, ProblemConfig, ProblemResponse},
    },
};
use tokio::sync::mpsc;

/// Per-WS-client egress buffer capacity. A client that lets this many broadcast messages queue
/// without draining them is considered too slow, and the hub drops its subscription — closing the
/// receiver and thus the socket. Internal hub consumers are unbounded and unaffected.
const WS_CLIENT_CHANNEL_CAPACITY: usize = 256;

#[utoipa::path(
    get,
    path = "/api/connect",
    tag = "builder",
    params (),
    responses(
        (status = OK)
    )
)]
pub async fn connect(
    State(state): State<AppState>,
    Extension(problem_config): Extension<ProblemConfig>,
    TargetHost(target_host): TargetHost,
    Origin(origin): Origin,
    user: CheckedCurrentUser,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, ProblemResponse> {
    if !state.settings().ws.is_allowed_host(target_host.as_str()) {
        return Err(Problem::forbidden()
            .with_detail("host is not allowed")
            .with_sensitive_str(target_host)
            .into_response(&problem_config));
    }

    if !state.settings().ws.is_allowed_origin(origin.as_str()) {
        return Err(Problem::forbidden()
            .with_detail("origin is not allowed")
            .with_sensitive_str(origin)
            .into_response(&problem_config));
    }

    let user = user.into_user();
    log::info!("User {} requesting a connection...", user.user_id);

    let hub_service = state.hub_service().clone();
    let chat_service = state.chat_service().clone();
    Ok(ws.on_upgrade(move |socket| handle_socket(socket, user, hub_service, chat_service)))
}

fn event_to_wire_message(message: HubMessage) -> Option<WSMessageResponse> {
    match message {
        HubMessage::Chat(batch) => {
            let messages = batch
                .comments
                .into_iter()
                .map(|comment| ChatWireComment {
                    id: comment.id,
                    from: comment.user_id,
                    text: comment.text,
                })
                .collect();
            Some(WSMessageResponse::Chat { messages })
        }
        _ => None,
    }
}

async fn handle_socket(socket: WebSocket, user: CurrentUser, hub_service: HubService, chat_service: ChatService) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let current_user_id = user.user_id;

    let (tx, mut rx) = mpsc::channel::<HubMessage>(WS_CLIENT_CHANNEL_CAPACITY);

    // Register the connection now that the upgrade has completed. If registration fails there is no
    // live connection to keep, so close the socket without emitting a disconnect for it.
    let connection_id = match hub_service.request_connection(current_user_id, user.key, tx, vec![TopicKey::Hub]) {
        Ok(connection_id) => connection_id,
        Err(err) => {
            log::error!("[{current_user_id}] Failed to register connection: {err:#?}");
            if let Err(err) = ws_sender.close().await {
                log::error!("[{current_user_id}] Failed to close websocket after failed registration: {err:#?}");
            }
            return;
        }
    };

    log::info!("[{current_user_id}] Connected to the hub");

    let mut recv_task = {
        tokio::spawn(async move {
            while let Some(Ok(message)) = ws_receiver.next().await {
                log::info!("[{current_user_id}] WsMessage received");
                if let WsMessage::Text(text) = message {
                    let msg = match serde_json::from_str::<WSMessageRequest>(&text) {
                        Ok(WSMessageRequest::Chat { text }) => Some(text),
                        Err(_) => {
                            log::error!("[{current_user_id}] Received invalid message: {text}");
                            None
                        }
                    };

                    if let Some(text) = msg {
                        if let Err(err) = chat_service.append_comment(current_user_id, &text).await {
                            log::error!("[{current_user_id}] Failed to append chat message: {err:#?}");
                        }
                    }
                }
            }
        })
    };

    let mut send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            log::info!("[{current_user_id}] Bus message received");
            // Close the socket on a disconnect targeting this connection, or on hub shutdown.
            // Without the Shutdown arm the task would block on rx.recv() forever during a graceful
            // drain, since the hub keeps this connection's tx alive.
            let close_reason = match &message {
                HubMessage::Hub(HubEvent::UserDisconnected {
                    connection_id: disconnected, ..
                }) if *disconnected == connection_id => Some("UserDisconnected"),
                HubMessage::Hub(HubEvent::Shutdown) => Some("Shutdown"),
                _ => None,
            };
            if let Some(reason) = close_reason {
                log::info!("[{current_user_id}] ws-close-triggered-by-hub-event: {reason}");
                if let Err(err) = ws_sender.close().await {
                    log::error!("[{current_user_id}] Failed to close websocket on hub event: {err:#?}");
                }
                break;
            }

            if let Some(msg) = event_to_wire_message(message) {
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

    log::info!("{current_user_id}] Disconnecting from hub");
    if let Err(err) = hub_service.request_disconnection(current_user_id, connection_id) {
        log::error!("[{current_user_id}] Failed to send disconnect command: {err:#?}");
    }
}
