//! The chat kind end to end: who may open the global chat.

use super::{chat_access, CHAT_STREAM_ID};
use crate::services::command_stream::StreamAccess;
use shine_infra::db::command_log::StreamId;
use shine_test::test;
use uuid::Uuid;

#[test]
fn only_the_global_chat_is_open_to_users() {
    let access = chat_access();
    let user = Uuid::new_v4();
    let chat = StreamId::parse(CHAT_STREAM_ID).unwrap();
    assert!(access.can_read(user, &chat) && access.can_write(user, &chat));
    for other in ["chat-other", "chat-global2", "x"] {
        let other = StreamId::parse(other).unwrap();
        assert!(
            !access.can_read(user, &other) && !access.can_write(user, &other),
            "{other}"
        );
    }
}
