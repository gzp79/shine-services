use bevy::ecs::component::Component;
use smallbox::{smallbox, SmallBox};
use std::{
    any::Any,
    collections::{hash_map::Entry, HashMap},
    hash::Hash,
};

pub trait ActionLike: Clone + Eq + Hash + Send + Sync + 'static {}

impl<A> ActionLike for A where A: Clone + Eq + Hash + Send + Sync + 'static {}

pub trait ActionState: Sync + Send + Any {
    /// Returns `self` as `&dyn Any`
    fn as_any(&self) -> &dyn std::any::Any;

    /// Returns `self` as `&mut dyn Any`
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T> ActionState for T
where
    T: Sync + Send + Any,
{
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

type BoxedState = SmallBox<dyn ActionState, smallbox::space::S64>;

#[derive(Component)]
pub struct ActionStates<A>
where
    A: ActionLike,
{
    version: usize,
    data: HashMap<A, (usize, BoxedState)>,
}

impl<A> Default for ActionStates<A>
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

impl<A> ActionStates<A>
where
    A: ActionLike,
{
    /// Clear all action data
    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn start_update(&mut self) {
        self.version += 1;
    }

    pub fn finish_update(&mut self) {
        self.data.retain(|_, data| data.0 == self.version);
    }

    /// Return the button state bound to the action. If data is not available or not a button, None is returned.
    pub fn get_as<T>(&self, action: &A) -> Option<&T>
    where
        T: ActionState,
    {
        self.data
            .get(action)
            .and_then(|data| data.1.as_any().downcast_ref::<T>())
    }

    pub fn set_as<T>(&mut self, action: A) -> &mut T
    where
        T: ActionState + Default,
    {
        let entry = self.data.entry(action);

        match entry {
            Entry::Vacant(entry) => {
                let pipeline: BoxedState = smallbox!(T::default());
                let entry = entry.insert((self.version, pipeline));
                entry.1.as_any_mut().downcast_mut::<T>().unwrap()
            }
            Entry::Occupied(entry) => {
                let entry = entry.into_mut();
                entry.0 = self.version;
                if entry.1.as_any_mut().downcast_mut::<T>().is_none() {
                    let pipeline: BoxedState = smallbox!(T::default());
                    entry.1 = pipeline;
                }
                entry.1.as_any_mut().downcast_mut::<T>().unwrap()
            }
        }
    }

    pub fn remove(&mut self, action: &A) {
        self.data.remove(action);
    }
}
