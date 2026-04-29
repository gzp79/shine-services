use crate::indexed::TypedIndex;

/// Integer type with modulo N arithmetic
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RotNIdx<const N: usize>(u8);

impl<const N: usize> Default for RotNIdx<N> {
    fn default() -> Self {
        Self::NONE
    }
}

impl<const N: usize> RotNIdx<N> {
    pub fn add(self, value: usize) -> Self {
        assert!(!self.is_none());
        Self::new((self.0 as usize + value) % N)
    }

    pub fn increment(self) -> Self {
        assert!(!self.is_none());
        Self::new((self.0 as usize + 1) % N)
    }

    pub fn sub(self, value: usize) -> Self {
        assert!(!self.is_none());
        Self::new((self.0 as usize + value + N - 1) % N)
    }

    pub fn decrement(self) -> Self {
        assert!(!self.is_none());
        Self::new((self.0 as usize + N - 1) % N)
    }
}

impl<const N: usize> From<RotNIdx<N>> for usize {
    fn from(i: RotNIdx<N>) -> usize {
        i.0 as usize
    }
}

impl<const N: usize> From<RotNIdx<N>> for u8 {
    fn from(i: RotNIdx<N>) -> u8 {
        i.0
    }
}

impl<const N: usize> From<u8> for RotNIdx<N> {
    fn from(i: u8) -> RotNIdx<N> {
        RotNIdx::new(i as usize)
    }
}

impl<const N: usize> TypedIndex for RotNIdx<N> {
    const NONE: Self = Self(u8::MAX);

    #[inline]
    fn new(index: usize) -> Self {
        debug_assert!(index < N, "RotN index overflow");
        Self(index as u8)
    }

    #[inline]
    fn into_index(self) -> usize {
        debug_assert!(self.0 < N as u8, "RotN index overflow");
        self.0 as usize
    }
}
