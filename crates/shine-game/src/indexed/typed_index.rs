/// Trait for typed indices backed by `i32`.
///
/// - Real indices: `0..i32::MAX - 1` (positive values)
/// - Ghost indices: negative values (`-1` = ghost 0, `-2` = ghost 1, ...)
/// - `NONE`: `i32::MAX` sentinel
///
/// `into_index()` panics on NONE or ghost. Use `into_ghost_index()` for ghosts.
pub trait TypedIndex: Copy + Eq + std::fmt::Debug {
    const NONE: Self;

    fn new(index: usize) -> Self;
    fn new_ghost(index: usize) -> Self;

    /// Convert to `usize`. Panics in debug builds if this is `NONE` or ghost.
    fn into_index(self) -> usize;

    /// Convert ghost to `usize`. Panics in debug builds if this is not ghost.
    fn into_ghost_index(self) -> usize;

    fn is_none(self) -> bool;
    fn is_ghost(self) -> bool;

    fn is_real(self) -> bool {
        !self.is_none() && !self.is_ghost()
    }

    /// Convert to `usize` if real, `None` otherwise.
    fn try_into_index(self) -> Option<usize> {
        if self.is_real() {
            Some(self.into_index())
        } else {
            None
        }
    }
}

/// Define a newtype index struct implementing `TypedIndex` with Serialize/Deserialize.
///
/// ```ignore
/// define_typed_index!(VertIdx, "Typed index into a vertex array.");
/// ```
#[macro_export]
macro_rules! define_typed_index {
    ($name:ident, $doc:expr) => {
        #[doc = $doc]
        #[derive(Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(i32);

        impl $crate::indexed::TypedIndex for $name {
            const NONE: Self = Self(i32::MAX);

            #[inline]
            fn new(index: usize) -> Self {
                debug_assert!(
                    index < i32::MAX as usize,
                    concat!(stringify!($name), " index overflow")
                );
                Self(index as i32)
            }

            #[inline]
            fn new_ghost(index: usize) -> Self {
                debug_assert!(
                    index < i32::MAX as usize,
                    concat!(stringify!($name), " ghost index overflow")
                );
                Self(-(index as i32) - 1)
            }

            #[inline]
            fn into_index(self) -> usize {
                debug_assert!(
                    self.0 >= 0 && self.0 != i32::MAX,
                    concat!("called into_index() on non-real ", stringify!($name))
                );
                self.0 as usize
            }

            #[inline]
            fn into_ghost_index(self) -> usize {
                debug_assert!(
                    self.0 < 0,
                    concat!("called into_ghost_index() on non-ghost ", stringify!($name))
                );
                (-self.0 - 1) as usize
            }

            #[inline]
            fn is_none(self) -> bool {
                self.0 == i32::MAX
            }

            #[inline]
            fn is_ghost(self) -> bool {
                self.0 < 0
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                if $crate::indexed::TypedIndex::is_none(*self) {
                    write!(f, "{}(NONE)", stringify!($name))
                } else if $crate::indexed::TypedIndex::is_ghost(*self) {
                    write!(
                        f,
                        "{}(ghost:{})",
                        stringify!($name),
                        $crate::indexed::TypedIndex::into_ghost_index(*self)
                    )
                } else {
                    write!(f, "{}({})", stringify!($name), self.0)
                }
            }
        }

        impl serde::Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                self.0.serialize(serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let v = i32::deserialize(deserializer)?;
                Ok(Self(v))
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;

    define_typed_index!(TestIdx, "Test index.");

    #[test]
    fn round_trip() {
        for i in [0, 1, 42, 1000, i32::MAX as usize - 1] {
            let idx = TestIdx::new(i);
            assert_eq!(idx.into_index(), i);
            assert!(idx.is_real());
            assert!(!idx.is_none());
            assert!(!idx.is_ghost());
        }
    }

    #[test]
    fn none() {
        assert!(TestIdx::NONE.is_none());
        assert!(!TestIdx::NONE.is_real());
        assert!(!TestIdx::NONE.is_ghost());
    }

    #[test]
    fn ghost_round_trip() {
        for i in [0, 1, 42, 1000] {
            let idx = TestIdx::new_ghost(i);
            assert!(idx.is_ghost());
            assert!(!idx.is_real());
            assert!(!idx.is_none());
            assert_eq!(idx.into_ghost_index(), i);
        }
    }

    #[test]
    fn debug_format() {
        assert_eq!(format!("{:?}", TestIdx::new(42)), "TestIdx(42)");
        assert_eq!(format!("{:?}", TestIdx::NONE), "TestIdx(NONE)");
        assert_eq!(format!("{:?}", TestIdx::new_ghost(3)), "TestIdx(ghost:3)");
    }
}
