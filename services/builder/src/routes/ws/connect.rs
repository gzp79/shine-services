use crate::{
    app_config::{AppConfig, AppConfigEx},
    app_state::AppState,
    models::messages::{
        CommandFailure, DisconnectReason, HubEvent, HubMessage, PingEvent, StreamMessage, SubscribeFailure, TopicKey,
    },
    routes::ws::{
        message::{decode_wire_command, stream_message_to_wire, PingRequest, WSMessageRequest, WSMessageResponse},
        socket_budget::{BudgetKind, SocketBudget, Verdict},
        ws_config::{SocketLimits, SocketTimeouts},
        ws_metrics::ws_metrics,
    },
    services::{hub_metrics, stream_metrics, HubService, MAX_COMMAND_BYTES},
};
use axum::{
    extract::{
        ws::{close_code, CloseFrame, Message as WsMessage, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    Extension,
};
use futures::{Sink, SinkExt, Stream, StreamExt};
use opentelemetry::trace::SpanContext;
use shine_game::replica::StreamId;
use shine_infra::{
    session::{CheckedCurrentUser, CurrentUser, SessionKey},
    sync::AdmissionPermit,
    telemetry::{current_span_context, link_span},
    web::{
        extracts::{Origin, TargetHost},
        responses::{IntoProblemResponse, Problem, ProblemConfig, ProblemResponse},
    },
};
use std::{fmt::Debug, sync::Arc, time::Duration};
use tokio::{sync::mpsc, time::MissedTickBehavior};
use tracing::{
    field::{display, Empty},
    info_span, instrument, Instrument, Span,
};
use uuid::Uuid;
use validator::Validate;

/// Per-WS-client egress buffer capacity. A client that lets this many broadcast messages queue
/// without draining them is considered too slow, and the hub drops its subscription — closing the
/// receiver and thus the socket. Internal hub consumers are unbounded and unaffected.
const WS_CLIENT_CHANNEL_CAPACITY: usize = 1000;

/// Close code sent when the hub drops a connection; the frame's reason carries the drop reason.
const CLOSE_CODE_DROPPED: u16 = 4000;

/// Close code of a hub drop reason.
fn close_code(reason: DisconnectReason) -> u16 {
    match reason {
        DisconnectReason::Shutdown => close_code::AWAY,
        DisconnectReason::RateLimited => close_code::POLICY,
        DisconnectReason::Client
        | DisconnectReason::Replaced
        | DisconnectReason::RegistryInactive
        | DisconnectReason::HeartbeatInactive
        | DisconnectReason::SessionInvalid
        | DisconnectReason::SlowConsumer
        | DisconnectReason::DeadChannel => CLOSE_CODE_DROPPED,
    }
}

/// Max size of a message and of a frame received from a client. The largest request is a stream command,
/// capped at `MAX_COMMAND_BYTES` plus a small envelope. A larger message ends the socket.
const MAX_WS_MESSAGE_BYTES: usize = 2 * MAX_COMMAND_BYTES;

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
    Extension(config): Extension<AppConfig>,
    Extension(problem_config): Extension<ProblemConfig>,
    TargetHost(target_host): TargetHost,
    Origin(origin): Origin,
    user: CheckedCurrentUser,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, ProblemResponse> {
    if !config.is_allowed_host(target_host.as_str()) {
        hub_metrics().connect_rejected("host");
        return Err(Problem::forbidden()
            .with_detail("host is not allowed")
            .with_sensitive_str(target_host)
            .into_response(&problem_config));
    }

    if !config.is_allowed_origin(origin.as_str()) {
        hub_metrics().connect_rejected("origin");
        return Err(Problem::forbidden()
            .with_detail("origin is not allowed")
            .with_sensitive_str(origin)
            .into_response(&problem_config));
    }

    let user = user.into_user();
    tracing::debug!("[{}] Requesting a connection", user.user_id);

    let hub_service = state.hub_service().clone();
    let permit = match hub_service.admit(user.user_id) {
        Ok(permit) => permit,
        Err(reason) => {
            hub_metrics().connect_rejected(reason.as_str());
            return Err(reason.into_response(&problem_config));
        }
    };
    let span_context = current_span_context();
    let ws = ws
        .max_message_size(MAX_WS_MESSAGE_BYTES)
        .max_frame_size(MAX_WS_MESSAGE_BYTES);
    Ok(ws.on_upgrade(move |socket| {
        let ws_config = config.ws_config();
        handle_socket(
            socket,
            user,
            hub_service,
            span_context,
            ws_config.timeouts,
            ws_config.limits,
            permit,
        )
    }))
}

fn event_to_wire_message(message: HubMessage) -> Option<WSMessageResponse> {
    match message {
        HubMessage::StreamMessage(message) => Some(stream_message_to_wire(&message)),
        HubMessage::Ping(event) => Some(WSMessageResponse::Ping { id: event.id }),
        HubMessage::Hub(_) | HubMessage::StreamEvent(_) => None,
    }
}

async fn handle_socket(
    socket: WebSocket,
    user: CurrentUser,
    hub_service: HubService,
    span_context: SpanContext,
    timeouts: SocketTimeouts,
    limits: SocketLimits,
    // Held until the socket ends.
    _permit: AdmissionPermit,
) {
    let (ws_sender, ws_receiver) = socket.split();
    serve_socket(
        ws_sender,
        ws_receiver,
        user.user_id,
        user.key,
        hub_service,
        span_context,
        timeouts,
        limits,
    )
    .await
}

/// Runs a client socket as a hub connection until either side ends it.
async fn serve_socket<S, R, E>(
    mut ws_sender: S,
    mut ws_receiver: R,
    current_user_id: Uuid,
    session_key: SessionKey,
    hub_service: HubService,
    span_context: SpanContext,
    timeouts: SocketTimeouts,
    limits: SocketLimits,
) where
    S: Sink<WsMessage> + Unpin + Send + 'static,
    S::Error: Debug,
    R: Stream<Item = Result<WsMessage, E>> + Unpin + Send + 'static,
    E: Send + 'static,
{
    let (tx, rx) = mpsc::channel::<HubMessage>(WS_CLIENT_CHANNEL_CAPACITY);
    // To answer its own client. Weak, so that it does not keep the channel open: the hub ends the socket by
    // dropping its sender.
    let replies = tx.downgrade();

    let Some(connection_id) = open_connection(span_context, &hub_service, current_user_id, session_key, tx) else {
        // There is no live connection to keep, so close the socket without emitting a disconnect for it.
        if let Err(err) = ws_sender.close().await {
            tracing::debug!("[{current_user_id}] Failed to close websocket after failed registration: {err:#?}");
        }
        return;
    };

    let connected = serde_json::to_string(&WSMessageResponse::Connected { user_id: current_user_id })
        .expect("Connected response is serializable");
    if let Err(err) = ws_sender.send(WsMessage::Text(connected.into())).await {
        tracing::debug!("[{current_user_id}] Failed to send Connected message: {err:#?}");
        close_connection(&hub_service, current_user_id, connection_id);
        return;
    }

    tracing::info!("[{current_user_id}] [{connection_id}] Connected to the hub");

    let mut recv_task = {
        let hub_service = hub_service.clone();
        tokio::spawn(async move {
            let mut budget = SocketBudget::new(&limits);
            loop {
                // Any frame counts as a sign of life, a `Pong` answering the server's `Ping` included.
                let message = match tokio::time::timeout(timeouts.idle, ws_receiver.next()).await {
                    Ok(Some(Ok(message))) => message,
                    Ok(_) => break,
                    Err(_) => {
                        tracing::info!("[{current_user_id}] [{connection_id}] Socket idle, closing");
                        ws_metrics().socket_timed_out("idle");
                        break;
                    }
                };
                tracing::debug!("[{current_user_id}] WsMessage received");
                let text = match message {
                    WsMessage::Text(text) => Some(text),
                    WsMessage::Binary(_) => None,
                    // The ping, pong and close frames are the socket's own business, they cost nothing.
                    _ => continue,
                };
                let request = text
                    .as_deref()
                    .and_then(|text| serde_json::from_str::<WSMessageRequest>(text).ok());

                let kind = request
                    .as_ref()
                    .map_or(BudgetKind::Command, WSMessageRequest::budget_kind);
                match budget.check(kind) {
                    Verdict::Allow => {}
                    Verdict::Deny => {
                        ws_metrics().message_received(
                            request.as_ref().map_or("unknown", WSMessageRequest::metric_kind),
                            "rate_limited",
                        );
                        if let Some(request) = &request {
                            refuse_request(request, &replies, current_user_id);
                        }
                        continue;
                    }
                    Verdict::Close => {
                        tracing::info!("[{current_user_id}] [{connection_id}] Socket over its budget, dropping");
                        ws_metrics().message_received(
                            request.as_ref().map_or("unknown", WSMessageRequest::metric_kind),
                            "rate_limited",
                        );
                        if let Err(err) =
                            hub_service.request_drop(current_user_id, connection_id, DisconnectReason::RateLimited)
                        {
                            tracing::info!("[{current_user_id}] Failed to drop the socket (hub stopped): {err:#?}");
                        }
                        break;
                    }
                }

                match (request, text) {
                    (Some(request), _) => {
                        let span = info_span!(
                            parent: None,
                            "ws message",
                            enduser.id = %current_user_id,
                            ws.connection_id = %connection_id,
                            ws.message.kind = "stream"
                        );
                        handle_stream_request(request, &hub_service, &replies, current_user_id, connection_id)
                            .instrument(span)
                            .await;
                    }
                    (None, Some(text)) => {
                        ws_metrics().message_received("unknown", "invalid");
                        tracing::trace!("[{current_user_id}] Received invalid message: {text}");
                    }
                    // Binary frames are ignored.
                    (None, None) => {}
                }
            }
        })
    };

    let mut send_task = tokio::spawn(forward_egress(rx, ws_sender, current_user_id, connection_id, timeouts));

    let send_exited = tokio::select! {
        rv_a = (&mut send_task) => {
            if let Err(err) = rv_a {
                tracing::error!("[{current_user_id}] Send task failed: {err:#?}");
            }
            true
        },
        rv_b = (&mut recv_task) => {
            if let Err(err) = rv_b {
                tracing::error!("[{current_user_id}] Receive task failed: {err:#?}");
            }
            false
        }
    };

    if send_exited {
        tracing::info!("[{current_user_id}] [{connection_id}] Disconnecting from hub: send task exited");
    } else {
        tracing::info!("[{current_user_id}] [{connection_id}] Disconnecting from hub: receive task exited");
    }

    close_connection(&hub_service, current_user_id, connection_id);
    if send_exited {
        recv_task.abort();
    } else {
        // Keep `rx` open until the hub drops the connection (the send task ends when `rx` closes), so
        // sends in the meantime do not find a closed channel and count a client close as a dead channel.
        if tokio::time::timeout(timeouts.send_drain, &mut send_task).await.is_err() {
            send_task.abort();
        }
    }
}

fn send_stream_response(replies: &mpsc::WeakSender<HubMessage>, message: StreamMessage) {
    // A channel that is gone, or full, means the client is going away or is too slow: no answer is needed.
    if let Some(tx) = replies.upgrade() {
        let _ = tx.try_send(HubMessage::StreamMessage(Arc::new(message)));
    }
}

fn parse_stream_id(user_id: Uuid, stream_id: &str) -> Option<StreamId> {
    match StreamId::parse(stream_id) {
        Ok(stream_id) => Some(stream_id),
        Err(_) => {
            tracing::trace!("[{user_id}] Invalid stream id: {stream_id}");
            None
        }
    }
}

/// Answers a request that is over the budget of the socket. Only the ones the client waits for are answered.
fn refuse_request(request: &WSMessageRequest, replies: &mpsc::WeakSender<HubMessage>, user_id: Uuid) {
    match request {
        WSMessageRequest::StreamCommand { stream_id, command_id, .. } => {
            if let Some(stream_id) = parse_stream_id(user_id, stream_id) {
                let reason = CommandFailure::RateLimited;
                stream_metrics().command_refused(stream_id.kind(), reason);
                send_stream_response(
                    replies,
                    StreamMessage::CommandFailed {
                        stream_id,
                        command_id: *command_id,
                        reason,
                    },
                );
            }
        }
        WSMessageRequest::StreamSubscribe { stream_id } => {
            if let Some(stream_id) = parse_stream_id(user_id, stream_id) {
                let reason = SubscribeFailure::RateLimited;
                send_stream_response(replies, StreamMessage::SubscribeFailed { stream_id, reason });
            }
        }
        WSMessageRequest::StreamUnsubscribe { .. } | WSMessageRequest::Ping(_) => {}
    }
}

/// Handles a request about streams, sent by the client.
async fn handle_stream_request(
    request: WSMessageRequest,
    hub_service: &HubService,
    replies: &mpsc::WeakSender<HubMessage>,
    user_id: Uuid,
    connection_id: Uuid,
) {
    let (kind, outcome) = match request {
        WSMessageRequest::StreamSubscribe { stream_id } => {
            let Some(stream_id) = parse_stream_id(user_id, &stream_id) else {
                return;
            };
            match hub_service
                .subscribe_stream(connection_id, user_id, stream_id.clone())
                .await
            {
                Ok(()) => ("stream_subscribe", "ok"),
                Err(reason) => {
                    send_stream_response(replies, StreamMessage::SubscribeFailed { stream_id, reason });
                    ("stream_subscribe", "refused")
                }
            }
        }
        WSMessageRequest::StreamUnsubscribe { stream_id } => {
            let Some(stream_id) = parse_stream_id(user_id, &stream_id) else {
                return;
            };
            let outcome = match hub_service.unsubscribe_stream(connection_id, stream_id) {
                Ok(()) => "ok",
                Err(_) => "failed",
            };
            ("stream_unsubscribe", outcome)
        }
        WSMessageRequest::StreamCommand { stream_id, command_id, command } => {
            let Some(stream_id) = parse_stream_id(user_id, &stream_id) else {
                return;
            };
            let sent = match decode_wire_command(&command) {
                Some(command) => {
                    hub_service
                        .send_stream_command(connection_id, user_id, &stream_id, command_id, &command)
                        .await
                }
                None => {
                    stream_metrics().command_refused(stream_id.kind(), CommandFailure::Invalid);
                    Err(CommandFailure::Invalid)
                }
            };
            match sent {
                Ok(()) => ("stream_command", "ok"),
                // Like any other malformed frame, an invalid command is dropped without an answer.
                Err(CommandFailure::Invalid) => {
                    tracing::trace!("[{user_id}] Invalid command for {stream_id}");
                    ("stream_command", "invalid")
                }
                Err(reason) => {
                    send_stream_response(replies, StreamMessage::CommandFailed { stream_id, command_id, reason });
                    ("stream_command", "refused")
                }
            }
        }
        WSMessageRequest::Ping(ping) => {
            let outcome = if !hub_service.ping_enabled() {
                "disabled"
            } else {
                if let Err(errors) = ping.validate() {
                    tracing::trace!("[{user_id}] Validation errors: {errors:#?}");
                    return;
                }
                let PingRequest { id, user_id } = ping;
                let message = HubMessage::Ping(PingEvent { id });
                match match user_id {
                    Some(user_id) => hub_service.send_to_user(user_id, message).await,
                    None => hub_service.broadcast(message).await,
                } {
                    Ok(()) => "ok",
                    Err(_) => "failed",
                }
            };
            ("ping", outcome)
        }
    };
    ws_metrics().message_received(kind, outcome);
}

/// Writes a frame within the write timeout. `false` when the socket is not writable any more (the write failed
/// or timed out): the peer is gone or does not read, and nothing more can be sent to it.
async fn send_frame<S>(ws_sender: &mut S, frame: WsMessage, user_id: Uuid, write_timeout: Duration) -> bool
where
    S: Sink<WsMessage> + Unpin,
    S::Error: Debug,
{
    match tokio::time::timeout(write_timeout, ws_sender.send(frame)).await {
        Ok(Ok(())) => true,
        Ok(Err(err)) => {
            tracing::debug!("[{user_id}] Failed to send message to the user: {err:#?}");
            false
        }
        Err(_) => {
            tracing::info!("[{user_id}] Write to the socket timed out, closing");
            ws_metrics().socket_timed_out("write");
            false
        }
    }
}

/// Forwards the hub's messages to the socket, and closes it once the hub ends the connection.
/// Also pings the peer periodically, which keeps the receiving side of the socket from going idle while the
/// peer is alive, and ends when a frame cannot be written within the write timeout.
async fn forward_egress<S>(
    mut rx: mpsc::Receiver<HubMessage>,
    mut ws_sender: S,
    user_id: Uuid,
    connection_id: Uuid,
    timeouts: SocketTimeouts,
) where
    S: Sink<WsMessage> + Unpin,
    S::Error: Debug,
{
    let mut ping = tokio::time::interval_at(
        tokio::time::Instant::now() + timeouts.ping_interval,
        timeouts.ping_interval,
    );
    ping.set_missed_tick_behavior(MissedTickBehavior::Delay);

    // The hub ends the socket by dropping its sender, after the socket's own `UserDisconnected` when it
    // removed the connection itself (the event is lost when the channel is full).
    let mut close_frame = None;
    loop {
        let message = tokio::select! {
            message = rx.recv() => match message {
                Some(message) => message,
                None => break,
            },
            _ = ping.tick() => {
                if !send_frame(&mut ws_sender, WsMessage::Ping(Default::default()), user_id, timeouts.write).await {
                    return;
                }
                continue;
            }
        };
        tracing::debug!("[{user_id}] Bus message received");
        if let HubMessage::Hub(HubEvent::UserDisconnected {
            connection_id: closed, reason, ..
        }) = message
        {
            if closed != connection_id || !reason.is_drop() {
                continue;
            }
            tracing::info!("[{user_id}] [{connection_id}] ws-close-triggered-by-hub: {reason}");
            close_frame = Some(CloseFrame {
                code: close_code(reason),
                reason: reason.as_str().into(),
            });
            break;
        }

        if let Some(msg) = event_to_wire_message(message) {
            let data = match serde_json::to_string(&msg) {
                Ok(data) => data,
                Err(err) => {
                    tracing::error!("[{user_id}] Failed to serialize message: {err:#?}");
                    tracing::trace!("[{user_id}] Unserializable message: {msg:#?}");
                    continue;
                }
            };
            if !send_frame(&mut ws_sender, WsMessage::Text(data.into()), user_id, timeouts.write).await {
                return;
            }
        }
    }

    send_frame(&mut ws_sender, WsMessage::Close(close_frame), user_id, timeouts.write).await;
}

/// Registers the connection in the hub, now that the upgrade has completed.
/// Linked to the upgrade request, not parented, so the request span ends with the 101 response.
#[instrument(
    name = "ws open",
    parent = None,
    skip_all,
    fields(enduser.id = %user_id, ws.connection_id = Empty)
)]
fn open_connection(
    span_context: SpanContext,
    hub_service: &HubService,
    user_id: Uuid,
    session_key: SessionKey,
    tx: mpsc::Sender<HubMessage>,
) -> Option<Uuid> {
    link_span(&Span::current(), span_context);
    match hub_service.request_connection(user_id, session_key, tx, vec![TopicKey::Hub, TopicKey::Ping]) {
        Ok(connection_id) => {
            Span::current().record("ws.connection_id", display(connection_id));
            Some(connection_id)
        }
        Err(err) => {
            tracing::warn!("[{user_id}] Failed to register connection: {err:#?}");
            hub_metrics().connect_rejected("hub_stopped");
            None
        }
    }
}

/// Removes the connection from the hub.
#[instrument(
    name = "ws close",
    parent = None,
    skip_all,
    fields(enduser.id = %user_id, ws.connection_id = %connection_id)
)]
fn close_connection(hub_service: &HubService, user_id: Uuid, connection_id: Uuid) {
    if let Err(err) = hub_service.request_disconnection(user_id, connection_id) {
        tracing::info!("[{user_id}] Failed to send disconnect command (hub stopped): {err:#?}");
    }
}

#[cfg(test)]
#[path = "connect_test.rs"]
mod connect_test;
