use crate::{
    models::messages::{DisconnectReason, HubEvent, HubMessage},
    services::SocketReceiver,
    testing::hub::test_hub::BARRIER_TIMEOUT,
};
use std::ops::{Deref, DerefMut};
use tokio::{sync::mpsc::error::TryRecvError, task::JoinHandle};
use uuid::Uuid;

/// The receiving end of a connection, as the socket of the connection would read it.
pub struct TestEgress(SocketReceiver);

impl TestEgress {
    pub fn new(egress: SocketReceiver) -> Self {
        Self(egress)
    }

    /// The next message; `None` when the queue is closed. Panics if nothing arrives in time.
    pub async fn next(&mut self) -> Option<HubMessage> {
        tokio::time::timeout(BARRIER_TIMEOUT, self.recv()).await.unwrap()
    }

    /// Takes everything queued so far, without waiting. The connection is closed once it ended with a reason or
    /// its queue is closed.
    pub fn drain(&mut self) -> EgressState {
        let mut state = EgressState::default();
        loop {
            match self.try_recv() {
                Ok(message) => state.record(message, None),
                Err(TryRecvError::Empty) => {
                    state.record_close(self.close_reason());
                    return state;
                }
                Err(TryRecvError::Disconnected) => {
                    state.closed = true;
                    state.record_close(self.close_reason());
                    return state;
                }
            }
        }
    }

    /// Reads the egress until its queue closes; only the drops of the given connection are recorded.
    pub fn spawn_reader(mut self, connection_id: Uuid) -> JoinHandle<EgressState> {
        tokio::spawn(async move {
            let mut state = EgressState::default();
            while let Some(message) = self.recv().await {
                state.record(message, Some(connection_id));
            }
            state.closed = true;
            state.record_close(self.close_reason());
            state
        })
    }
}

impl Deref for TestEgress {
    type Target = SocketReceiver;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TestEgress {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// What was read from an [`TestEgress`].
#[derive(Debug, Default)]
pub struct EgressState {
    pub pings: Vec<String>,
    pub disconnects: Vec<DisconnectReason>,
    pub closed: bool,
}

impl EgressState {
    /// Records the reason the connection ended with: a drop is listed, and the connection is closed.
    fn record_close(&mut self, reason: Option<DisconnectReason>) {
        if let Some(reason) = reason {
            self.closed = true;
            if reason.is_drop() {
                self.disconnects.push(reason);
            }
        }
    }

    /// Records a message; the drops of other connections than `connection_id` (if given) are ignored.
    fn record(&mut self, message: HubMessage, connection_id: Option<Uuid>) {
        match message {
            HubMessage::Ping(event) => self.pings.push(event.id),
            HubMessage::Hub(HubEvent::UserDisconnected {
                connection_id: dropped, reason, ..
            }) if reason.is_drop() && connection_id.is_none_or(|own| own == dropped) => self.disconnects.push(reason),
            HubMessage::Hub(_) | HubMessage::StreamMessage(_) | HubMessage::StreamEvent(_) => {}
        }
    }

    pub fn assert_open(&self) -> &Self {
        assert!(!self.closed, "expected egress to be open");
        self
    }

    pub fn assert_closed(&self) -> &Self {
        assert!(self.closed, "expected egress to be closed");
        self
    }

    pub fn assert_pings(&self, expected: &[&str]) -> &Self {
        assert_eq!(self.pings, expected);
        self
    }

    pub fn assert_disconnects(&self, expected: &[DisconnectReason]) -> &Self {
        assert_eq!(self.disconnects, expected);
        self
    }

    pub fn assert_last_disconnect(&self, reason: DisconnectReason) -> &Self {
        assert!(
            self.disconnects.ends_with(&[reason]),
            "expected egress to be closed with reason: {:?}",
            reason
        );
        self
    }
}
