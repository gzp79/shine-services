use crate::{
    camera_rig::{RigDriver, RigUpdateParams},
    math::value::{AnimatedVariable, Variable},
};
use bevy::{math::Vec3, transform::components::Transform};

/// Offsets the camera along a vector in the coordinate space of the parent.
pub struct Arm<A>
where
    A: AnimatedVariable<Value = Vec3>,
{
    pub offset: A,
}

impl<A> Arm<A>
where
    A: AnimatedVariable<Value = Vec3>,
{
    pub fn new(offset: A) -> Self {
        Self { offset }
    }
}

impl<A> RigDriver for Arm<A>
where
    A: AnimatedVariable<Value = Vec3>,
{
    fn visit_variables(&self, visitor: &mut dyn FnMut(&dyn Variable) -> bool) {
        visitor(&self.offset);
    }

    fn variable_mut(&mut self, name: &str) -> Option<&mut dyn Variable> {
        if self.offset.name() == Some(name) {
            Some(&mut self.offset)
        } else {
            None
        }
    }

    fn update(&mut self, params: RigUpdateParams) -> Transform {
        let parent_position = params.parent.translation;
        let parent_rotation = params.parent.rotation;
        let offset: Vec3 = self.offset.animate(params.delta_time_s);

        let position = parent_position + parent_rotation * offset;

        Transform::from_translation(position).with_rotation(parent_rotation)
    }
}
