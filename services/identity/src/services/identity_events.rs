use shine_core::sync::{Event, TopicEvent};
use uuid::Uuid;

pub struct IdentityTopic;

#[derive(Debug)]
pub enum UserEvent {
    Created(Uuid),
    Updated(Uuid),
    Deleted(Uuid),
    RoleChange(Uuid),
}

impl Event for UserEvent {}
impl TopicEvent for UserEvent {
    type Topic = IdentityTopic;
}

#[derive(Debug)]
pub enum UserLinkEvent {
    Linked(Uuid),
    Unlinked(Uuid),
}

impl Event for UserLinkEvent {}
impl TopicEvent for UserLinkEvent {
    type Topic = IdentityTopic;
}
