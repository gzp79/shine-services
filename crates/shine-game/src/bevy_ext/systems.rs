use bevy::ecs::{
    component::Component,
    entity::Entity,
    query::With,
    system::{Commands, Query},
};

pub fn despawn_tagged<T: Component>(entity_q: Query<Entity, With<T>>, mut commands: Commands) {
    for entity in entity_q.iter() {
        commands.entity(entity).despawn();
    }
}
