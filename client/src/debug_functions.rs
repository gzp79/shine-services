use crate::avatar::AvatarAction;
use bevy::ecs::{event::EventWriter, system::Query};
use shine_forge::map::{MapChunkId, MapEvent};
use shine_game::input_manager::ActionState;

pub fn debug_load_chunk(mut avatar_q: Query<&ActionState<AvatarAction>>, mut map_events: EventWriter<MapEvent>) {
    let actions = avatar_q.single_mut().unwrap();

    if actions.just_pressed(&AvatarAction::Debug1) {
        log::info!("Debug1 action triggered");
        map_events.write(MapEvent::Load(MapChunkId(12, 13)));
    }

    if actions.just_pressed(&AvatarAction::Debug2) {
        log::info!("Debug2 action triggered");
        map_events.write(MapEvent::Unload(MapChunkId(12, 13)));
    }
}
