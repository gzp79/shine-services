use crate::input_manager::{MapInput, MappedInput, TypedUserInput, UserInput};

/// Helper to make the common input processing accessible as a method on `UserInput`.
pub trait InputProcess: UserInput {
    fn map<T, U, M>(self, map: M) -> MappedInput<T, U, Self, M>
    where
        T: Send + Sync + 'static,
        U: Send + Sync + 'static,
        Self: TypedUserInput<T> + Sized,
        M: MapInput<T, U>,
    {
        MappedInput::new(self, map)
    }
}

impl<T> InputProcess for T where T: UserInput + Sized {}
