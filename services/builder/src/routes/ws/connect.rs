use crate::{
    app_state::AppState,
    models::messages::{DisconnectReason, HubEvent, HubMessage, TopicKey},
    routes::ws::{
        message::{ChatWireComment, WSMessageRequest, WSMessageResponse, WsErrorCode, PROTOCOL_VERSION},
        rate_limiter::RateLimiter,
    },
    services::{hub_metrics, ChatService, HubService},
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
use shine_infra::{
    session::{CheckedCurrentUser, CurrentUser, SessionKey},
    telemetry::{current_span_context, label, link_span},
    web::{
        extracts::{Origin, TargetHost},
        responses::{IntoProblemResponse, Problem, ProblemConfig, ProblemResponse},
    },
};
use std::{
    fmt::Debug,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tracing::{
    field::{display, Empty},
    info_span, instrument, Instrument, Span,
};
use uuid::Uuid;

/// Per-WS-client egress buffer capacity. A client that lets this many broadcast messages queue
/// without draining them is considered too slow, and the hub drops its subscription — closing the
/// receiver and thus the socket. Internal hub consumers are unbounded and unaffected.
const WS_CLIENT_CHANNEL_CAPACITY: usize = 1000;

/// Close codes of a connection the hub ended, telling the client what to do next; the frame's
/// reason carries the drop reason.
/// - 4001: another connection of the user replaced this one, do not reconnect on your own.
/// - 4002: the session is gone, sign in again before reconnecting.
/// - 4003: transient, reconnect with backoff and jitter.
/// - 1013 (try again later): the client was too slow or the hub could not serve it, reconnect with backoff.
/// - 1001 (going away): the server is shutting down, reconnect with backoff and jitter.
/// - 1008 (policy): the client kept sending refused frames.
const CLOSE_CODE_REPLACED: u16 = 4001;
const CLOSE_CODE_SESSION_INVALID: u16 = 4002;
const CLOSE_CODE_TRANSIENT: u16 = 4003;

/// Close code of a hub drop reason.
fn close_code(reason: DisconnectReason) -> u16 {
    match reason {
        DisconnectReason::Shutdown => close_code::AWAY,
        DisconnectReason::Replaced => CLOSE_CODE_REPLACED,
        DisconnectReason::SessionInvalid => CLOSE_CODE_SESSION_INVALID,
        DisconnectReason::SlowConsumer => close_code::AGAIN,
        DisconnectReason::Client
        | DisconnectReason::RegistryInactive
        | DisconnectReason::HeartbeatInactive
        | DisconnectReason::DeadChannel => CLOSE_CODE_TRANSIENT,
    }
}

/// Largest chat text accepted, in bytes.
const MAX_CHAT_TEXT_BYTES: usize = 2000;

/// Largest frame accepted from a client; leaves room for the JSON around a chat text.
const MAX_FRAME_BYTES: usize = 8 * 1024;

/// A client may send this many chat messages at once, then `CHAT_REFILL_PER_SEC` per second.
const CHAT_BURST: u32 = 5;
const CHAT_REFILL_PER_SEC: f64 = 1.0;

/// Consecutive refused frames after which the client is closed.
const MAX_STRIKES: u32 = 10;

/// Notices of the socket itself waiting for the sender task.
const NOTICE_CHANNEL_CAPACITY: usize = 32;

/// Max wait after a client close for the hub to drop the connection, before the send task is aborted.
const SEND_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

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
    tracing::debug!("[{}] Requesting a connection", user.user_id);

    // Refused before the upgrade, so a reconnect loop or an overloaded hub costs one cheap response.
    if let Err(err) = state.hub_service().check_connect_allowed(user.user_id) {
        tracing::debug!("[{}] Connection refused: {err}", user.user_id);
        return Err(Problem::from(err).into_response(&problem_config));
    }

    let hub_service = state.hub_service().clone();
    let chat_service = state.chat_service().clone();
    let span_context = current_span_context();
    Ok(ws
        .max_message_size(MAX_FRAME_BYTES)
        .max_frame_size(MAX_FRAME_BYTES)
        .on_upgrade(move |socket| handle_socket(socket, user, hub_service, chat_service, span_context)))
}

fn event_to_wire_message(message: HubMessage) -> Option<WSMessageResponse> {
    match message {
        HubMessage::Hub(_) => None,
        HubMessage::Chat(batch) => {
            let messages = batch
                .comments
                .iter()
                .map(|comment| ChatWireComment {
                    id: comment.id.clone(),
                    from: comment.user_id,
                    text: comment.text.clone(),
                })
                .collect();
            Some(WSMessageResponse::Chat { messages })
        }
    }
}

/// Something the socket itself tells the client, besides the hub's messages.
enum Notice {
    Frame(WSMessageResponse),
    /// End the socket with this close frame.
    Close(CloseFrame),
}

/// Timeouts of a socket.
#[derive(Clone, Copy, Debug)]
pub struct SocketTimings {
    /// Idle time after which the server pings the client.
    pub ping_interval: Duration,
    /// Time without any frame from the client (pongs included) after which the socket is closed.
    pub idle_timeout: Duration,
    /// Time a single frame may take to be written; a client that does not read is cut off.
    pub send_timeout: Duration,
}

impl Default for SocketTimings {
    fn default() -> Self {
        Self {
            ping_interval: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(90),
            send_timeout: Duration::from_secs(10),
        }
    }
}

async fn handle_socket(
    socket: WebSocket,
    user: CurrentUser,
    hub_service: HubService,
    chat_service: ChatService,
    span_context: SpanContext,
) {
    let (ws_sender, ws_receiver) = socket.split();
    serve_socket(
        ws_sender,
        ws_receiver,
        user.user_id,
        user.key,
        hub_service,
        chat_service,
        span_context,
        SocketTimings::default(),
    )
    .await
}

/// Runs a client socket as a hub connection until either side ends it.
#[allow(clippy::too_many_arguments)]
async fn serve_socket<S, R, E>(
    mut ws_sender: S,
    ws_receiver: R,
    current_user_id: Uuid,
    session_key: SessionKey,
    hub_service: HubService,
    chat_service: ChatService,
    span_context: SpanContext,
    timings: SocketTimings,
) where
    S: Sink<WsMessage> + Unpin + Send + 'static,
    S::Error: Debug,
    R: Stream<Item = Result<WsMessage, E>> + Unpin + Send + 'static,
    E: Send + 'static,
{
    let (tx, rx) = mpsc::channel::<HubMessage>(WS_CLIENT_CHANNEL_CAPACITY);

    let Some(connection_id) = open_connection(span_context, &hub_service, current_user_id, session_key, tx) else {
        // There is no live connection to keep, so close the socket without emitting a disconnect for it.
        if let Err(err) = ws_sender.close().await {
            tracing::debug!("[{current_user_id}] Failed to close websocket after failed registration: {err:#?}");
        }
        return;
    };
    tracing::info!("[{current_user_id}] [{connection_id}] Connected to the hub");
    let mut connection = ConnectionGuard::new(&hub_service, current_user_id, connection_id);

    let (notice_tx, notice_rx) = mpsc::channel::<Notice>(NOTICE_CHANNEL_CAPACITY);
    let _ = notice_tx.try_send(Notice::Frame(WSMessageResponse::Hello {
        version: PROTOCOL_VERSION,
        connection_id,
    }));
    let client_left = Arc::new(AtomicBool::new(false));

    let mut recv_task = tokio::spawn(receive_from_client(
        ws_receiver,
        chat_service,
        current_user_id,
        connection_id,
        notice_tx,
        timings.idle_timeout,
    ));

    let mut send_task = tokio::spawn(forward_egress(
        rx,
        notice_rx,
        ws_sender,
        current_user_id,
        connection_id,
        client_left.clone(),
        timings,
    ));

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

    let reason = if send_exited {
        "send task exited"
    } else {
        "receive task exited"
    };
    tracing::info!("[{current_user_id}] [{connection_id}] Disconnecting from hub: {reason}");
    connection.close();
    if send_exited {
        recv_task.abort();
    } else {
        client_left.store(true, Ordering::Relaxed);
        // Keep `rx` open until the hub drops the connection (the send task ends when `rx` closes), so
        // sends in the meantime do not find a closed channel and count a client close as a dead channel.
        if tokio::time::timeout(SEND_DRAIN_TIMEOUT, &mut send_task).await.is_err() {
            send_task.abort();
        }
    }
}

/// Reads the client's frames: chat goes to the room, anything else is refused. A client that keeps
/// sending refused frames is closed, and one that goes silent past the idle timeout is too.
async fn receive_from_client<R, E>(
    mut ws_receiver: R,
    chat_service: ChatService,
    user_id: Uuid,
    connection_id: Uuid,
    notices: mpsc::Sender<Notice>,
    idle_timeout: Duration,
) where
    R: Stream<Item = Result<WsMessage, E>> + Unpin,
{
    let mut limiter = RateLimiter::new(CHAT_BURST, CHAT_REFILL_PER_SEC, Instant::now());
    let mut strikes = 0u32;
    let mut append_failures = 0u64;

    loop {
        let frame = match tokio::time::timeout(idle_timeout, ws_receiver.next()).await {
            Ok(Some(Ok(frame))) => frame,
            Ok(_) => break,
            Err(_) => {
                tracing::info!("[{user_id}] [{connection_id}] Closing an idle websocket");
                break;
            }
        };
        tracing::debug!("[{user_id}] WsMessage received");

        let rejection = match frame {
            WsMessage::Text(text) => match serde_json::from_str::<WSMessageRequest>(&text) {
                Ok(WSMessageRequest::Chat { text }) => {
                    if text.trim().is_empty() {
                        Some(WsErrorCode::Invalid)
                    } else if text.len() > MAX_CHAT_TEXT_BYTES {
                        Some(WsErrorCode::TooLong)
                    } else if !limiter.allow(Instant::now()) {
                        Some(WsErrorCode::RateLimited)
                    } else {
                        let span = info_span!(
                            parent: None,
                            "ws message",
                            enduser.id = %user_id,
                            ws.connection_id = %connection_id,
                            ws.message.kind = "chat"
                        );
                        match chat_service.append_comment(user_id, &text).instrument(span).await {
                            Ok(id) => {
                                strikes = 0;
                                hub_metrics()
                                    .ws_messages_received
                                    .add(1, &[label("kind", "chat"), label("outcome", "ok")]);
                                let _ = notices.try_send(Notice::Frame(WSMessageResponse::Ack { id }));
                                None
                            }
                            Err(err) => {
                                hub_metrics()
                                    .ws_messages_received
                                    .add(1, &[label("kind", "chat"), label("outcome", "append_failed")]);
                                // During an outage every message fails: log the first and then ever fewer.
                                append_failures += 1;
                                if append_failures.is_power_of_two() {
                                    tracing::error!(
                                        "[{user_id}] Failed to append chat message ({append_failures} failures): {err:#?}"
                                    );
                                }
                                let _ = notices.try_send(Notice::Frame(WSMessageResponse::Error {
                                    code: WsErrorCode::AppendFailed,
                                }));
                                continue;
                            }
                        }
                    }
                }
                Err(_) => {
                    tracing::trace!("[{user_id}] Received invalid message: {text}");
                    Some(WsErrorCode::Invalid)
                }
            },
            WsMessage::Binary(_) => Some(WsErrorCode::Invalid),
            WsMessage::Close(_) => break,
            WsMessage::Ping(_) | WsMessage::Pong(_) => continue,
        };

        let Some(code) = rejection else {
            continue;
        };
        hub_metrics()
            .ws_messages_received
            .add(1, &[label("kind", "chat"), label("outcome", rejected_label(code))]);
        let _ = notices.try_send(Notice::Frame(WSMessageResponse::Error { code }));
        strikes += 1;
        if strikes >= MAX_STRIKES {
            tracing::info!("[{user_id}] [{connection_id}] Closing a websocket that keeps sending refused frames");
            let close = Notice::Close(CloseFrame {
                code: close_code::POLICY,
                reason: "abusive_client".into(),
            });
            let _ = tokio::time::timeout(Duration::from_secs(1), notices.send(close)).await;
            break;
        }
    }
}

fn rejected_label(code: WsErrorCode) -> &'static str {
    match code {
        WsErrorCode::Invalid => "invalid",
        WsErrorCode::TooLong => "too_long",
        WsErrorCode::RateLimited => "rate_limited",
        WsErrorCode::AppendFailed => "append_failed",
    }
}

/// Writes a frame within the send timeout. `false` when the client does not take it, which ends
/// the socket: a client that does not read must not pin its tasks and queued messages.
async fn send_frame<S>(ws_sender: &mut S, frame: WsMessage, timeout: Duration, user_id: Uuid) -> bool
where
    S: Sink<WsMessage> + Unpin,
    S::Error: Debug,
{
    match tokio::time::timeout(timeout, ws_sender.send(frame)).await {
        Ok(Ok(())) => true,
        Ok(Err(err)) => {
            tracing::debug!("[{user_id}] Failed to send message to the user: {err:#?}");
            false
        }
        Err(_) => {
            tracing::info!("[{user_id}] Sending to the user timed out");
            false
        }
    }
}

/// Forwards the hub's messages and the socket's own notices to the client, pings it while it is
/// quiet, and closes the socket once the hub ends the connection.
async fn forward_egress<S>(
    mut rx: mpsc::Receiver<HubMessage>,
    mut notices: mpsc::Receiver<Notice>,
    mut ws_sender: S,
    user_id: Uuid,
    connection_id: Uuid,
    client_left: Arc<AtomicBool>,
    timings: SocketTimings,
) where
    S: Sink<WsMessage> + Unpin,
    S::Error: Debug,
{
    // The hub ends the socket by dropping its sender, after the socket's own `UserDisconnected` when it
    // removed the connection itself (the event is lost when the channel is full).
    let mut close_frame = None;
    let mut healthy = true;
    let mut pinger = tokio::time::interval(timings.ping_interval);
    pinger.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    pinger.tick().await;

    let ended_by_hub = loop {
        let frame = tokio::select! {
            message = rx.recv() => {
                let Some(message) = message else {
                    break true;
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
                    break true;
                }
                match event_to_wire_message(message) {
                    Some(msg) => msg,
                    None => continue,
                }
            }
            Some(notice) = notices.recv() => match notice {
                Notice::Frame(msg) => msg,
                Notice::Close(frame) => {
                    close_frame = Some(frame);
                    break false;
                }
            },
            _ = pinger.tick() => {
                if !send_frame(&mut ws_sender, WsMessage::Ping(Vec::new().into()), timings.send_timeout, user_id).await {
                    healthy = false;
                    break false;
                }
                continue;
            }
        };

        let data = match serde_json::to_string(&frame) {
            Ok(data) => data,
            Err(err) => {
                tracing::error!("[{user_id}] Failed to serialize message: {err:#?}");
                tracing::trace!("[{user_id}] Unserializable message: {frame:#?}");
                continue;
            }
        };
        if !send_frame(&mut ws_sender, WsMessage::Text(data.into()), timings.send_timeout, user_id).await {
            healthy = false;
            break false;
        }
    };

    // A channel the hub ended without a notice was dropped for being too slow (the notice did not
    // fit) or dead; a client that left needs no reason.
    if close_frame.is_none() && ended_by_hub && !client_left.load(Ordering::Relaxed) {
        close_frame = Some(CloseFrame {
            code: close_code::AGAIN,
            reason: "try_again_later".into(),
        });
    }
    if healthy {
        send_frame(&mut ws_sender, WsMessage::Close(close_frame), timings.send_timeout, user_id).await;
    }
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
    // The socket only needs chat: the end of its own connection is sent to it directly, and
    // lifecycle events of other users would cost every socket an event per connect or disconnect.
    match hub_service.request_connection(user_id, session_key, tx, vec![TopicKey::Chat]) {
        Ok(connection_id) => {
            Span::current().record("ws.connection_id", display(connection_id));
            Some(connection_id)
        }
        Err(err) => {
            tracing::warn!("[{user_id}] Failed to register connection: {err:#?}");
            None
        }
    }
}

/// Removes the connection from the hub when the socket's task ends, however it ends: a cancelled or
/// panicking task would otherwise leave the connection registered and its registry entry refreshed.
struct ConnectionGuard<'a> {
    hub_service: &'a HubService,
    user_id: Uuid,
    connection_id: Uuid,
    closed: bool,
}

impl<'a> ConnectionGuard<'a> {
    fn new(hub_service: &'a HubService, user_id: Uuid, connection_id: Uuid) -> Self {
        Self { hub_service, user_id, connection_id, closed: false }
    }

    fn close(&mut self) {
        if !std::mem::replace(&mut self.closed, true) {
            close_connection(self.hub_service, self.user_id, self.connection_id);
        }
    }
}

impl Drop for ConnectionGuard<'_> {
    fn drop(&mut self) {
        self.close();
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
