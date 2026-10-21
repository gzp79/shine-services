use crate::{
    models::messages::{HubMessage, StreamMessage, StreamResume},
    services::{
        command_stream::{IngestOutcome, StreamKind},
        hub_metrics, CommandStreamService, HubService,
    },
};
use shine_game::replay::protocol::{ClientFrame, WireResume};
use shine_infra::{db::command_log::StreamId, telemetry::label};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::mpsc::WeakSender;
use uuid::Uuid;

/// A stream the connection follows.
struct Followed {
    kind: Arc<dyn StreamKind>,
    can_write: bool,
}

/// The stream side of one socket: it opens streams for the connection, checks the commands it writes, and
/// answers its own failures. The replies go straight into the connection's egress channel through a weak
/// sender, never through the hub's command loop, and do not keep the channel open once the hub drops it.
pub struct StreamSession {
    user_id: Uuid,
    connection_id: Uuid,
    hub: HubService,
    streams: Option<CommandStreamService>,
    replies: WeakSender<HubMessage>,
    followed: HashMap<StreamId, Followed>,
}

impl StreamSession {
    pub fn new(
        user_id: Uuid,
        connection_id: Uuid,
        hub: HubService,
        streams: Option<CommandStreamService>,
        replies: WeakSender<HubMessage>,
    ) -> Self {
        Self {
            user_id,
            connection_id,
            hub,
            streams,
            replies,
            followed: HashMap::new(),
        }
    }

    pub async fn handle(&mut self, frame: ClientFrame) {
        let (kind, outcome) = match frame {
            ClientFrame::StreamSubscribe { stream_id, kind, resume } => {
                ("streamSubscribe", self.subscribe(&stream_id, &kind, resume).await)
            }
            ClientFrame::StreamUnsubscribe { stream_id } => ("streamUnsubscribe", self.unsubscribe(&stream_id)),
            ClientFrame::StreamCommand { stream_id, command_id, command } => {
                ("streamCommand", self.command(&stream_id, command_id, &command).await)
            }
        };
        hub_metrics()
            .ws_messages_received
            .add(1, &[label("kind", kind), label("outcome", outcome)]);
    }

    /// Sends a reply to the client unless its channel is full or gone: the hub drops such a connection itself.
    fn reply(&self, message: StreamMessage) {
        if let Some(replies) = self.replies.upgrade() {
            let _ = replies.try_send(HubMessage::Stream(message));
        }
    }

    fn parse_id(&self, stream_id: &str) -> Option<StreamId> {
        match StreamId::parse(stream_id) {
            Ok(stream_id) => Some(stream_id),
            Err(_) => {
                tracing::debug!("[{}] Invalid stream id in a stream frame", self.user_id);
                None
            }
        }
    }

    fn subscribe_failed(&self, stream_id: StreamId, reason: &'static str) -> &'static str {
        self.reply(StreamMessage::SubscribeFailed { stream_id, reason });
        reason
    }

    async fn subscribe(&mut self, stream_id: &str, kind: &str, resume: Option<WireResume>) -> &'static str {
        let Some(stream_id) = self.parse_id(stream_id) else {
            return "invalid";
        };
        let Some(streams) = self.streams.clone() else {
            return self.subscribe_failed(stream_id, "unavailable");
        };

        let opened = match streams.open(self.user_id, &stream_id, kind).await {
            Ok(opened) => opened,
            Err(err) => {
                if let crate::services::command_stream::OpenError::Failed(source) = &err {
                    tracing::warn!("[{}] Failed to open the stream {stream_id}: {source:#?}", self.user_id);
                }
                return self.subscribe_failed(stream_id, err.reason());
            }
        };

        let kind = opened.kind.kind();
        let resume = resume.map(|resume| StreamResume {
            generation: resume.generation,
            version: resume.version,
        });
        // Followed before the hub is asked, so a command sent right after the subscribe is accepted.
        self.followed.insert(
            stream_id.clone(),
            Followed {
                kind: opened.kind,
                can_write: opened.can_write,
            },
        );
        if let Err(err) =
            self.hub
                .request_subscribe_stream(self.user_id, self.connection_id, stream_id.clone(), kind, resume)
        {
            tracing::warn!("[{}] Failed to request a stream subscription: {err:#?}", self.user_id);
            self.followed.remove(&stream_id);
            return self.subscribe_failed(stream_id, "failed");
        }
        "ok"
    }

    fn unsubscribe(&mut self, stream_id: &str) -> &'static str {
        let Some(stream_id) = self.parse_id(stream_id) else {
            return "invalid";
        };
        if self.followed.remove(&stream_id).is_none() {
            return "not_subscribed";
        }
        if let Err(err) = self
            .hub
            .request_unsubscribe_stream(self.user_id, self.connection_id, stream_id)
        {
            tracing::info!(
                "[{}] Failed to request a stream unsubscription (hub stopped): {err:#?}",
                self.user_id
            );
        }
        "ok"
    }

    async fn command(&mut self, stream_id: &str, command_id: Uuid, command: &str) -> &'static str {
        let Some(stream_id) = self.parse_id(stream_id) else {
            return "invalid";
        };
        let nack = |reason: &'static str| {
            self.reply(StreamMessage::Nack {
                stream_id: stream_id.clone(),
                command_id,
                reason,
            });
            reason
        };

        let (Some(streams), Some(followed)) = (&self.streams, self.followed.get(&stream_id)) else {
            return nack("notSubscribed");
        };
        if !followed.can_write {
            return nack("forbidden");
        }
        match streams
            .ingest(followed.kind.as_ref(), &stream_id, command_id, self.user_id, command)
            .await
        {
            Ok(IngestOutcome::Accepted(_)) => "ok",
            Ok(IngestOutcome::Duplicate(_)) => nack("duplicate"),
            Ok(IngestOutcome::Invalid) => nack("invalid"),
            Ok(IngestOutcome::NotReady) => nack("notReady"),
            Ok(IngestOutcome::Busy) => nack("busy"),
            Err(err) => {
                tracing::warn!("[{}] Failed to ingest a command of {stream_id}: {err:#?}", self.user_id);
                nack("failed")
            }
        }
    }
}
