use crate::models::messages::{DisconnectReason, HubMessage};
use std::sync::{Arc, Weak};
use tokio::sync::{
    mpsc::{self, error::TrySendError},
    watch,
};

/// What the send task gets from its endpoint.
#[derive(Debug)]
pub enum SocketMessage {
    /// Data for the client.
    Data(HubMessage),
    /// The connection ended with this reason; queued data is not sent.
    Close(DisconnectReason),
    /// The hub dropped the endpoint without giving a reason. Marks a bug, not a case.
    Gone,
}

/// The reason the connection ends with; set once.
type CloseSlot = watch::Sender<Option<DisconnectReason>>;

/// Creates the two halves of a connection's endpoint. `capacity` is the size of the data queue.
pub fn socket_channel(capacity: usize) -> (SocketSender, SocketReceiver) {
    let (tx, rx) = mpsc::channel(capacity);
    let (close_tx, close_rx) = watch::channel(None);
    (
        SocketSender { tx, close: Arc::new(close_tx) },
        SocketReceiver { rx, close: close_rx },
    )
}

/// The hub side of a connection's endpoint.
#[derive(Clone, Debug)]
pub struct SocketSender {
    tx: mpsc::Sender<HubMessage>,
    close: Arc<CloseSlot>,
}

impl SocketSender {
    /// Queues a message without waiting.
    pub fn try_send(&self, message: HubMessage) -> Result<(), TrySendError<HubMessage>> {
        self.tx.try_send(message)
    }

    /// Ends the connection with the reason.
    pub fn close(&self, reason: DisconnectReason) -> bool {
        self.close.send_if_modified(|slot| {
            if slot.is_some() {
                return false;
            }
            *slot = Some(reason);
            true
        })
    }

    /// The reason the connection was ended with, if it was.
    pub fn close_reason(&self) -> Option<DisconnectReason> {
        *self.close.borrow()
    }

    /// A handle that does not keep the endpoint alive.
    pub fn downgrade(&self) -> WeakSocketSender {
        WeakSocketSender {
            tx: self.tx.downgrade(),
            close: Arc::downgrade(&self.close),
        }
    }
}

/// A [`SocketSender`] that does not keep the endpoint alive: the hub ends a connection by dropping its sender.
#[derive(Clone, Debug)]
pub struct WeakSocketSender {
    tx: mpsc::WeakSender<HubMessage>,
    close: Weak<CloseSlot>,
}

impl WeakSocketSender {
    pub fn upgrade(&self) -> Option<SocketSender> {
        Some(SocketSender {
            tx: self.tx.upgrade()?,
            close: self.close.upgrade()?,
        })
    }
}

/// The socket side of a connection's endpoint, read by the send task.
pub struct SocketReceiver {
    rx: mpsc::Receiver<HubMessage>,
    close: watch::Receiver<Option<DisconnectReason>>,
}

impl SocketReceiver {
    /// The next event, by priority: the close reason beats queued data.
    ///
    /// Cancel safe: both the queue and the slot are awaited in cancel safe ways, so it can be a branch of a
    /// `select!`.
    pub async fn next(&mut self) -> SocketMessage {
        loop {
            if let Some(reason) = *self.close.borrow_and_update() {
                return SocketMessage::Close(reason);
            }
            tokio::select! {
                biased;
                changed = self.close.changed() => {
                    if changed.is_err() {
                        // Every sender is gone, and the queue with them: what is left is still data.
                        return self.after_senders_gone().await;
                    }
                }
                message = self.rx.recv() => {
                    return match message {
                        Some(message) => SocketMessage::Data(message),
                        None => self.after_senders_gone().await,
                    };
                }
            }
        }
    }

    async fn after_senders_gone(&mut self) -> SocketMessage {
        if let Some(reason) = *self.close.borrow() {
            return SocketMessage::Close(reason);
        }
        match self.rx.recv().await {
            Some(message) => SocketMessage::Data(message),
            None => SocketMessage::Gone,
        }
    }

    /// The next queued message, whatever the reason; `None` when the queue is empty or closed.
    #[cfg(test)]
    pub fn try_recv(&mut self) -> Result<HubMessage, mpsc::error::TryRecvError> {
        self.rx.try_recv()
    }

    /// The next queued message; `None` when the queue is closed.
    #[cfg(test)]
    pub async fn recv(&mut self) -> Option<HubMessage> {
        self.rx.recv().await
    }

    /// The reason the connection ended with, if it did.
    #[cfg(test)]
    pub fn close_reason(&self) -> Option<DisconnectReason> {
        *self.close.borrow()
    }
}

#[cfg(test)]
#[path = "socket_channel_test.rs"]
mod socket_channel_test;
