use std::fmt;

pub trait EnumIndex: Into<usize> + TryFrom<usize> + fmt::Debug {}
impl<T: Into<usize> + TryFrom<usize> + fmt::Debug> EnumIndex for T where <T as TryFrom<usize>>::Error: fmt::Debug {}

/// Defines an enum with explicit `usize` index mappings, automatically generating
/// `TryFrom<usize>` and `From<EnumType> for usize` implementations so the enum
/// satisfies the [`EnumIndex`] trait.
///
/// An associated `COUNT` constant is also generated, equal to the number of variants.
///
/// # Syntax
///
/// ```text
/// define_enum_index! {
///     $(#[$attr:meta])*
///     $vis $Name {
///         $($(#[$variant_attr:meta])* $value:literal => $Variant),* $(,)?
///     }
/// }
/// ```
///
/// # Example
///
/// ```rust
/// use shine_game::define_enum_index;
///
/// define_enum_index! {
///     /// Cardinal directions, starting from North going clockwise.
///     #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
///     pub Direction {
///         /// Facing north
///         0 => North,
///         /// Facing east
///         1 => East,
///         /// Facing south
///         2 => South,
///         /// Facing west
///         3 => West,
///     }
/// }
///
/// assert_eq!(Direction::COUNT, 4);
/// assert_eq!(usize::from(Direction::East), 1);
/// assert_eq!(Direction::try_from(2), Ok(Direction::South));
/// assert!(Direction::try_from(99).is_err());
/// ```
#[macro_export]
macro_rules! define_enum_index {
    (
        $(#[$meta:meta])*
        $vis:vis $name:ident {
            $( $(#[$variant_meta:meta])* $value:literal => $variant:ident ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $(
                $(#[$variant_meta])*
                $variant,
            )*
        }

        #[allow(dead_code)]
        impl $name {
            /// The total number of variants in this enum.
            pub const COUNT: usize = [$($value as usize),*].len();

            /// Returns an iterator over all variants in declaration order.
            pub fn all() -> impl Iterator<Item = Self>
            {
                [$($name::$variant),*].into_iter()
            }

            /// Converts this enum variant into its corresponding index.
            pub fn into_index(self) -> usize {
                usize::from(self)
            }
        }

        impl TryFrom<usize> for $name {
            type Error = &'static str;

            #[inline]
            fn try_from(value: usize) -> Result<Self, Self::Error> {
                match value {
                    $($value => Ok($name::$variant),)*
                    _ => Err(concat!("Invalid ", stringify!($name), " value")),
                }
            }
        }

        impl From<$name> for usize {
            #[inline]
            fn from(value: $name) -> Self {
                match value {
                    $($name::$variant => $value,)*
                }
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use shine_test::test;

    crate::define_enum_index! {
        /// A simple three-variant enum used for macro tests.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub Tri {
            0 => A,
            1 => B,
            2 => C,
        }
    }

    #[test]
    fn count() {
        assert_eq!(Tri::COUNT, 3);
    }

    #[test]
    fn from_enum_to_usize() {
        assert_eq!(usize::from(Tri::A), 0);
        assert_eq!(usize::from(Tri::B), 1);
        assert_eq!(usize::from(Tri::C), 2);
    }

    #[test]
    fn try_from_usize_ok() {
        assert_eq!(Tri::try_from(0), Ok(Tri::A));
        assert_eq!(Tri::try_from(1), Ok(Tri::B));
        assert_eq!(Tri::try_from(2), Ok(Tri::C));
    }

    #[test]
    fn try_from_usize_err() {
        assert!(Tri::try_from(3).is_err());
        assert!(Tri::try_from(usize::MAX).is_err());
    }

    #[test]
    fn round_trip() {
        for variant in [Tri::A, Tri::B, Tri::C] {
            let idx = usize::from(variant);
            assert_eq!(Tri::try_from(idx), Ok(variant));
        }
    }

    #[test]
    fn all_iterator() {
        let all: Vec<_> = Tri::all().collect();
        assert_eq!(all, vec![Tri::A, Tri::B, Tri::C]);
    }

    crate::define_enum_index! {
        /// Enum with non-contiguous explicit values to verify the macro is value-driven.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub Sparse {
            0 => Zero,
            5 => Five,
            10 => Ten,
        }
    }

    #[test]
    fn sparse_count() {
        assert_eq!(Sparse::COUNT, 3);
    }

    #[test]
    fn sparse_from() {
        assert_eq!(usize::from(Sparse::Zero), 0);
        assert_eq!(usize::from(Sparse::Five), 5);
        assert_eq!(usize::from(Sparse::Ten), 10);
    }

    #[test]
    fn sparse_try_from_valid() {
        assert_eq!(Sparse::try_from(0), Ok(Sparse::Zero));
        assert_eq!(Sparse::try_from(5), Ok(Sparse::Five));
        assert_eq!(Sparse::try_from(10), Ok(Sparse::Ten));
    }

    #[test]
    fn sparse_try_from_invalid() {
        assert!(Sparse::try_from(1).is_err());
        assert!(Sparse::try_from(6).is_err());
        assert!(Sparse::try_from(99).is_err());
    }
}
