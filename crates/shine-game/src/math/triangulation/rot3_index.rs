use crate::indexed::{RotNIdx, TypedIndex};

/// Integer type with modulo 3 arithmetic
pub type Rot3Idx = RotNIdx<3>;

impl Rot3Idx {
    pub fn third(a: Self, b: Self) -> Self {
        assert!(!a.is_none() && !b.is_none() && a != b);
        let a_val: u8 = a.into();
        let b_val: u8 = b.into();
        Self::new((3 - a_val - b_val) as usize)
    }

    pub fn mirror(self, over: u8) -> Self {
        let self_val: u8 = self.into();
        assert!(self_val != over);
        match over {
            0 => Self::new((3 - self_val) as usize),
            1 => Self::new((2 - self_val) as usize),
            2 => Self::new((1 - self_val) as usize),
            _ => unreachable!(""),
        }
    }
}
