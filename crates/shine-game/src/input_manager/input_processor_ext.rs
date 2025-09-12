use crate::input_manager::{InputProcessor, MapInput, MappedInput, TypedInputProcessor};
use std::any::Any;

pub trait InputProcessorExt: InputProcessor {
    fn boxed(self) -> Box<dyn InputProcessor>
    where
        Self: Sized,
    {
        Box::new(self)
    }

    fn traverse<'a>(&'a self, visitor: &mut dyn FnMut(usize, &'a dyn InputProcessor) -> bool) {
        self.visit_recursive(0, visitor);
    }

    fn find_by_name<'a>(&'a self, name: &str) -> Option<&'a dyn InputProcessor> {
        let mut result = None;

        self.traverse(&mut |_, node| {
            if result.is_none() && node.name() == name {
                result = Some(node);
            }
            result.is_none()
        });

        result
    }

    fn find_by_name_as<T>(&self, name: &str) -> Option<&T>
    where
        T: InputProcessor,
    {
        self.find_by_name(name).and_then(|input| {
            let s = input as &dyn Any;
            s.downcast_ref::<T>()
        })
    }

    fn map<T, U, M>(self, map: M) -> MappedInput<T, U, Self, M>
    where
        T: Send + Sync + 'static,
        U: Send + Sync + 'static,
        Self: TypedInputProcessor<T> + Sized,
        M: MapInput<T, U>,
    {
        MappedInput::new(self, map)
    }
}

impl<T: InputProcessor + ?Sized> InputProcessorExt for T {}
