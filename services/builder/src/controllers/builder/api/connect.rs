use axum::{
    body::Bytes,
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
};
use serde::Serialize;
use shine_core::web::{CheckedCurrentUser, CurrentUser};
use utoipa::ToSchema;

use crate::app_state::AppState;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServiceHealth {}

#[utoipa::path(
    get,
    path = "/api/builder/connect",
    tag = "builder",
    responses(
        (status = OK)
    )
)]
pub async fn connect(
    State(state): State<AppState>,
    user: CheckedCurrentUser,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, user.into_user()))
}

async fn handle_socket(mut socket: WebSocket, user: CurrentUser) {
    // send a ping (unsupported by some browsers) just to kick things off and get a response
    if socket.send(Message::Ping(vec![1, 2, 3])).await.is_ok() {
        println!("Pinged {}...", user.user_id);
    } else {
        println!("Could not send ping {}!", user.user_id);
        return;
    }

    // By splitting socket we can send and receive at the same time. In this example we will send
    // unsolicited messages to client based on some sort of server's internal event (i.e .timer).
    //let (mut sender, mut receiver) = socket.split();
}
