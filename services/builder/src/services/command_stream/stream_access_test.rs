use crate::services::command_stream::{AllowListStreamAccess, StreamAccess};
use shine_game::replica::{StreamId, StreamKind};
use shine_test::test;
use std::collections::HashSet;
use uuid::Uuid;

fn stream(kind: StreamKind, id: &str) -> StreamId {
    StreamId::new(kind, id).unwrap()
}

#[test]
async fn an_allow_list_opens_only_its_streams_of_its_kind() {
    let access = AllowListStreamAccess::new(HashSet::from([stream(StreamKind::Chat, "global")]));
    let user = Uuid::new_v4();

    assert!(access.allows(user, &stream(StreamKind::Chat, "global")).await);
    assert!(!access.allows(user, &stream(StreamKind::Chat, "other")).await);
    assert!(!access.allows(user, &stream(StreamKind::PixelMap, "global")).await);
}
