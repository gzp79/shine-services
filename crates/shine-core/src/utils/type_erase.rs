use std::any::Any;

/// Helper to erase type information
pub trait TypeErase: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn type_name(&self) -> &'static str;
}

impl<T> TypeErase for T
where
    T: Any + Send + Sync,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn type_name(&self) -> &'static str {
        std::any::type_name::<T>()
    }
}

pub trait TypeEraseExt: TypeErase {
    /// Returns the simplified, human readable type name. It is usually the last segment of the full type
    /// name without the generic type parameters.
    fn simple_type_name(&self) -> &'static str {
        let raw = self.type_name();
        let strip_generic = raw.split_once("<").map(|(first, _)| first).unwrap_or(raw);
        let base = strip_generic.split("::").last().unwrap_or(strip_generic);
        base
    }
}

impl<T> TypeEraseExt for T where T: ?Sized + TypeErase {}
