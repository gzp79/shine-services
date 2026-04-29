crate::define_enum_index! {
    /// 6 neighbor direction for a pointy-topped hex grid in CCW order
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub HexPointyDir {
        0 => E,
        1 => NE,
        2 => NW,
        3 => W,
        4 => SW,
        5 => SE,
    }
}

crate::define_enum_index! {
    /// 6 neighbor direction for a flat-topped hex grid in CCW order
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub HexFlatDir {
        0 => NE,
        1 => N,
        2 => NW,
        3 => SW,
        4 => S,
        5 => SE,
    }
}
