use crate::{
    app_state::AppState,
    models::messages::{ChatMessage, HubCommand, HubMessage, TopicKey},
    routes::ws::message::{WSMessageRequest, WSMessageResponse},
    services::{HubReceiver, HubSender},
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
            .into_response(&problem_config));
    }

    if !state.settings().ws.is_allowed_origin(origin.as_str()) {
        return Err(Problem::forbidden()
            .with_detail("origin is not allowed")
            .into_response(&problem_config));
    }

    let user = user.into_user();
    log::info!("User {} requesting a connection...", user.user_id);

    let sender = state.hub_service().sender();
    let subscription = state.hub_service().subscribe(vec![TopicKey::Chat]).await;

    sender
        .send_command(HubCommand::ConnectUser {
            user_id: user.user_id,
            session_key: user.key,
        })
        .map_err(|err| err.into_response(&problem_config))?;

    Ok(ws.on_upgrade(move |socket| handle_socket(socket, user, sender, subscription)))
}

fn event_to_wire_message(message: HubMessage) -> Option<WSMessageResponse> {
    match message {
        HubMessage::Chat(ChatMessage { user_id, text }) => Some(WSMessageResponse::Chat { from: user_id, text }),
        _ => None,
    }
}

async fn handle_socket(socket: WebSocket, user: CurrentUser, sender: HubSender, mut subscription: HubReceiver) {
    let (mut ws_sender, mut ws_receiver) = socket.split();
    let current_user_id = user.user_id;

    log::info!("[{current_user_id}] Connected to the hub");

    let mut recv_task = {
        let sender = sender.clone();
        tokio::spawn(async move {
            if let Err(err) = sender.send_command(HubCommand::Chat(ChatMessage {
                user_id: current_user_id,
                text: "${tr: Connected}".to_string(),
            })) {
                log::error!("[{current_user_id}] Failed to send initial message: {err:#?}");
            }

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
                        if let Err(err) =
                            sender.send_command(HubCommand::Chat(ChatMessage { user_id: current_user_id, text }))
                        {
                            log::error!("[{current_user_id}] Failed to send message: {err:#?}");
                        }
                    }
                }
            }
        })
    };

    let mut send_task = tokio::spawn(async move {
        while let Some(message) = subscription.recv().await {
            log::info!("[{current_user_id}] Bus message received");
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
    if let Err(err) = sender.send_command(HubCommand::DisconnectUser {
        user_id: current_user_id,
        session_key: user.key,
    }) {
        log::error!("[{current_user_id}] Failed to send disconnect command: {err:#?}");
    }
}
