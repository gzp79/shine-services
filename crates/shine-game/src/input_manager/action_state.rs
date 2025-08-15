use crate::input_manager::{AnyActionValue, AxisValue, ButtonStatus, ButtonValue, DualAxisValue, InputValueFold};
use bevy::{ecs::component::Component, math::Vec2};
use smallbox::{smallbox, SmallBox};
use std::{
    collections::{hash_map::Entry, HashMap},
    hash::Hash,
    ops::{Deref, DerefMut},
};

pub trait ActionLike: Clone + Eq + Hash + Send + Sync + 'static {}
impl<A> ActionLike for A where A: Clone + Eq + Hash + Send + Sync + 'static {}

/// A trait to convert InputValue into action value.
pub trait IntoActionValue: Clone + Send + Sync + 'static {
    type ActionValue: Sync + Send + Default + 'static;

    fn default_fold() -> Box<dyn InputValueFold<Self>>
    where
        Self: Sized;

    fn update_state(state: &mut Self::ActionValue, value: Option<Self>, time_s: f32);
}

type BoxedState = SmallBox<dyn AnyActionValue, smallbox::space::S64>;

#[derive(Component)]
pub struct ActionState<A>
where
    A: ActionLike,
{
    version: usize,
    data: HashMap<A, (usize, BoxedState)>,
}

impl<A> Default for ActionState<A>
where
    A: ActionLike,
{
    fn default() -> Self {
        Self {
            version: 0,
            data: HashMap::new(),
        }
    }
}

impl<A> ActionState<A>
where
    A: ActionLike,
{
    /// Clear all action data
    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn start_update(&mut self) -> usize {
        self.version += 1;
        self.version
    }

    pub fn finish_update(&mut self) {
        self.data.retain(|_, data| data.0 == self.version);
    }

    /// Return the button state bound to the action. If data is not available or not a button, None is returned.
    pub fn get_as<T>(&self, action: &A) -> Option<&T>
    where
        T: Sync + Send + 'static,
    {
        self.data
            .get(action)
            .and_then(|data| data.1.deref().as_any().downcast_ref::<T>())
    }

    pub fn set_as<T>(&mut self, action: A) -> &mut T
    where
        T: Sync + Send + Default + 'static,
    {
        let entry = self.data.entry(action);

        match entry {
            Entry::Vacant(entry) => {
                let pipeline: BoxedState = smallbox!(T::default());
                let entry = entry.insert((self.version, pipeline));
                entry.1.deref_mut().as_any_mut().downcast_mut::<T>().unwrap()
            }
            Entry::Occupied(entry) => {
                let entry = entry.into_mut();
                entry.0 = self.version;
                if entry.1.deref_mut().as_any_mut().downcast_mut::<T>().is_none() {
                    let pipeline: BoxedState = smallbox!(T::default());
                    entry.1 = pipeline;
                }
                entry.1.deref_mut().as_any_mut().downcast_mut::<T>().unwrap()
            }
        }
    }

    pub fn remove(&mut self, action: &A) {
        self.data.remove(action);
    }

    /// A convenience method to get the button state. If action is not available or not a button, None is returned.
    #[inline]
    pub fn button_value(&self, action: &A) -> ButtonStatus {
        self.get_as::<ButtonValue>(action)
            .map_or(ButtonStatus::None, |state| state.status)
    }

    /// A convenience method to get if button was just pressed. If action is not available or not a button, None is returned.
    #[inline]
    pub fn just_pressed(&self, action: &A) -> bool {
        self.get_as::<ButtonValue>(action)
            .is_some_and(|state| state.just_pressed())
    }

    /// A convenience method to get if button was just released. If action is not available or not a button, None is returned.
    #[inline]
    pub fn just_released(&self, action: &A) -> bool {
        self.get_as::<ButtonValue>(action)
            .is_some_and(|state| state.just_released())
    }

    /// A convenience method to get if button is currently pressed. If action is not available or not a button, None is returned.
    #[inline]
    pub fn is_pressed(&self, action: &A) -> bool {
        self.get_as::<ButtonValue>(action).is_some_and(|state| state.is_down())
    }

    /// A convenience method to get the axis value, returning None if data is not available.
    #[inline]
    pub fn try_axis_value(&self, action: &A) -> Option<f32> {
        self.get_as::<AxisValue>(action).and_then(|state| state.value)
    }

    /// A convenience method to get the axis value, returning 0.0 if data is not available.
    #[inline]
    pub fn axis_value(&self, action: &A) -> f32 {
        self.try_axis_value(action).unwrap_or(0.0)
    }

    /// A convenience method to get the dual-axis value, returning None if data is not available.
    #[inline]
    pub fn try_dual_axis_value(&self, action: &A) -> Option<Vec2> {
        self.get_as::<DualAxisValue>(action).and_then(|state| state.value)
    }

    /// A convenience method to get the dual-axis value, returning Vec2::ZERO if data is not available.
    #[inline]
    pub fn dual_axis_value(&self, action: &A) -> Vec2 {
        self.try_dual_axis_value(action).unwrap_or(Vec2::ZERO)
    }
}
