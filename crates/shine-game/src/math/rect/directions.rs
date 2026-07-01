crate::define_enum_index! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    /// 4 neighbor direction for a flat-topped quad grid in CCW order
    pub QuadFlatDir {
        0 => E,
        1 => N,
        2 => W,
        3 => S,
    }
}

crate::define_enum_index! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    /// 4 neighbor direction for a pointy-topped quad grid in CCW order
    pub QuadPointyDir {
        0 => NE,
        1 => NW,
        2 => SW,
        3 => SE,
    }
}
