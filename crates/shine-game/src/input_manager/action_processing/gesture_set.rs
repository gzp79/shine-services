use crate::math::{GestureId, JackknifeTemplateSet};
use bevy::{
    ecs::{component::Component, entity::Entity},
    math::Vec2,
};
use std::ops;

/// A set of gesture templates to recognized.
#[derive(Component)]
#[require(AttachedRecognizers)]
pub struct GestureSet(pub JackknifeTemplateSet<Vec2>);

impl ops::Deref for GestureSet {
    type Target = JackknifeTemplateSet<Vec2>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ops::DerefMut for GestureSet {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Store the detected gesture ID.
#[derive(Component, Default)]
pub struct DetectedGesture(pub Option<GestureId>);

#[derive(Component, Default)]
#[relationship_target(relationship = AttachedToGestureSet, linked_spawn)]
pub struct AttachedRecognizers(Vec<Entity>);

/// Attach this component to an input handler entity to associate multiple gesture recognizers as its children.
/// This enables simultaneous detection of multiple gesture types, such as recognizing gestures from different input devices.
/// Useful for scenarios where a single input handler needs to manage several gesture recognizers independently.
#[derive(Component)]
#[relationship(relationship_target = AttachedRecognizers)]
pub struct AttachedToGestureSet(pub Entity);
