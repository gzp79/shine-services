use crate::indexed::TypedIndex;

/// Integer type with module 3 arithmetic
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rot3Idx(u8);

impl Default for Rot3Idx {
    fn default() -> Self {
        Self::NONE
    }
}

impl Rot3Idx {
    pub fn third(a: Self, b: Self) -> Self {
        assert!(!a.is_none() && !b.is_none() && a != b);
        Self::new((3 - a.0 - b.0) as usize)
    }

    pub fn increment(self) -> Self {
        assert!(!self.is_none());
        Self::new(((self.0 + 1) % 3) as usize)
    }

    pub fn decrement(self) -> Self {
        assert!(!self.is_none());
        Self::new(((self.0 + 2) % 3) as usize)
    }

    pub fn mirror(self, over: u8) -> Self {
        assert!(self.0 != over);
        match over {
            0 => Self::new(3 - self.0 as usize),
            1 => Self::new(2 - self.0 as usize),
            2 => Self::new(1 - self.0 as usize),
            _ => unreachable!(""),
        }
    }
}

impl From<Rot3Idx> for usize {
    fn from(i: Rot3Idx) -> usize {
        i.0 as usize
    }
}

impl From<Rot3Idx> for u8 {
    fn from(i: Rot3Idx) -> u8 {
        i.0
    }
}

impl From<u8> for Rot3Idx {
    fn from(i: u8) -> Rot3Idx {
        Rot3Idx::new(i as usize)
    }
}

impl TypedIndex for Rot3Idx {
    const NONE: Self = Self(u8::MAX);

    #[inline]
    fn new(index: usize) -> Self {
        debug_assert!(index < 3, "Rot3 index overflow");
        Self(index as u8)
    }

    #[inline]
    fn into_index(self) -> usize {
        debug_assert!(self.0 != u8::MAX, "called into_index() on non-real Rot3");
        self.0 as usize
    }

    #[inline]
    fn is_none(self) -> bool {
        self.0 == u8::MAX
    }
}
