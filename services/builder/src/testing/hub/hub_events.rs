use crate::models::messages::{DisconnectReason, HubEvent};
use std::{
    collections::{BTreeMap, HashMap},
    ops::Deref,
};
use uuid::Uuid;

/// A connection's lifecycle event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifecycle {
    Connected(Uuid),
    Disconnected(Uuid),
}

/// The events the hub published, as returned by `TestHub::wait_all_commands` and `TestHub::wait_event`.
#[derive(Debug, Default)]
pub struct HubEvents(Vec<HubEvent>);

impl HubEvents {
    /// The lifecycle events of all users, grouped by user ID. Panics on a shutdown event.
    pub fn lifecycles(&self) -> HashMap<Uuid, Vec<Lifecycle>> {
        let mut by_user = HashMap::<Uuid, Vec<Lifecycle>>::new();
        for event in &self.0 {
            let (user_id, lifecycle) = match event {
                HubEvent::UserConnected { user_id, connection_id, .. } => {
                    (*user_id, Lifecycle::Connected(*connection_id))
                }
                HubEvent::UserDisconnected { user_id, connection_id, .. } => {
                    (*user_id, Lifecycle::Disconnected(*connection_id))
                }
                HubEvent::Shutdown => panic!("unexpected Shutdown event"),
            };
            by_user.entry(user_id).or_default().push(lifecycle);
        }
        by_user
    }

    /// The lifecycle events of a specific user.
    pub fn lifecycle_of(&self, user_id: Uuid) -> Vec<Lifecycle> {
        self.lifecycles().remove(&user_id).unwrap_or_default()
    }

    /// The active connection of each user after the events, as the hub's registry should hold them.
    pub fn active(&self) -> HashMap<Uuid, Uuid> {
        let mut active = HashMap::new();
        for event in &self.0 {
            match event {
                HubEvent::UserConnected { user_id, connection_id, .. } => {
                    active.insert(*user_id, *connection_id);
                }
                HubEvent::UserDisconnected { user_id, connection_id, .. }
                    if active.get(user_id) == Some(connection_id) =>
                {
                    active.remove(user_id);
                }
                HubEvent::UserDisconnected { .. } | HubEvent::Shutdown => {}
            }
        }
        active
    }

    /// The number of connections made.
    pub fn connects(&self) -> usize {
        self.0
            .iter()
            .filter(|event| matches!(event, HubEvent::UserConnected { .. }))
            .count()
    }

    /// Connections removed by the hub, by reason.
    pub fn drops(&self) -> BTreeMap<DisconnectReason, u64> {
        let mut drops = BTreeMap::new();
        for event in &self.0 {
            if let HubEvent::UserDisconnected { reason, .. } = event {
                if reason.is_drop() {
                    *drops.entry(*reason).or_default() += 1;
                }
            }
        }
        drops
    }
}

impl From<Vec<HubEvent>> for HubEvents {
    fn from(events: Vec<HubEvent>) -> Self {
        Self(events)
    }
}

impl Deref for HubEvents {
    type Target = [HubEvent];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Extend<HubEvent> for HubEvents {
    fn extend<T: IntoIterator<Item = HubEvent>>(&mut self, events: T) {
        self.0.extend(events);
    }
}

impl IntoIterator for HubEvents {
    type Item = HubEvent;
    type IntoIter = std::vec::IntoIter<HubEvent>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
