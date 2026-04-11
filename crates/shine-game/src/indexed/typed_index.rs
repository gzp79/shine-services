/// Trait for typed indices backed by `u32`.
///
/// - Real indices: `0..u32::MAX - 1` (positive values)
/// - `NONE`: `u32::MAX` sentinel
///
/// `into_index()` panics on NONE or ghost. Use `into_ghost_index()` for ghosts.
pub trait TypedIndex: Copy + Eq + std::fmt::Debug {
    const NONE: Self;

    fn new(index: usize) -> Self;
    fn into_index(self) -> usize;

    fn is_none(self) -> bool;

    #[inline]
    fn is_valid(self) -> bool {
        !self.is_none()
    }

    /// Convert to `usize` if real, `None` otherwise.
    fn try_into_index(self) -> Option<usize> {
        if self.is_none() {
            None
        } else {
            Some(self.into_index())
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
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        #[allow(dead_code)]
        impl $name {
            pub fn range(start: Self, end: Self) -> impl Iterator<Item = Self> {
                (start.0..end.0).map(Self)
            }
        }

        impl $crate::indexed::TypedIndex for $name {
            const NONE: Self = Self(u32::MAX);

            #[inline]
            fn new(index: usize) -> Self {
                debug_assert!(
                    index < u32::MAX as usize,
                    concat!(stringify!($name), " index overflow")
                );
                Self(index as u32)
            }

            #[inline]
            fn into_index(self) -> usize {
                debug_assert!(
                    self.0 != u32::MAX,
                    concat!("called into_index() on non-real ", stringify!($name))
                );
                self.0 as usize
            }

            #[inline]
            fn is_none(self) -> bool {
                self.0 == u32::MAX
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                if $crate::indexed::TypedIndex::is_none(*self) {
                    write!(f, "{}(NONE)", stringify!($name))
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
                let v = u32::deserialize(deserializer)?;
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
            assert!(!idx.is_none());
        }
    }

    #[test]
    fn none() {
        assert!(TestIdx::NONE.is_none());
    }

    #[test]
    fn debug_format() {
        assert_eq!(format!("{:?}", TestIdx::new(42)), "TestIdx(42)");
        assert_eq!(format!("{:?}", TestIdx::NONE), "TestIdx(NONE)");
    }

    #[test]
    fn range() {
        let range = TestIdx::range(TestIdx::new(5), TestIdx::new(10));
        let collected: Vec<_> = range.collect();
        let expected: Vec<_> = (5..10).map(|i| TestIdx::new(i)).collect();
        assert_eq!(collected, expected);
    }
}
