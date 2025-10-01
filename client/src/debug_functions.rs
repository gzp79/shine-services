use crate::avatar::AvatarAction;
use bevy::ecs::{message::MessageWriter, system::Query};
use shine_forge::map::{MapChunkId, MapMessage};
use shine_game::input_manager::ActionState;

pub fn debug_load_chunk(mut avatar_q: Query<&ActionState<AvatarAction>>, mut map_messages: MessageWriter<MapMessage>) {
    let actions = avatar_q.single_mut().unwrap();

    if actions.just_pressed(&AvatarAction::Debug1) {
        log::info!("Debug1 action triggered");
        map_messages.write(MapMessage::Load(MapChunkId(12, 13)));
    }

    if actions.just_pressed(&AvatarAction::Debug2) {
        log::info!("Debug2 action triggered");
        map_messages.write(MapMessage::Unload(MapChunkId(12, 13)));
    }
}
