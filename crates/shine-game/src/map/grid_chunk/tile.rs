pub trait Tile: Default + Send + Sync + 'static {}

impl Tile for u8 {}
impl Tile for u16 {}
impl Tile for u32 {}
impl Tile for u64 {}
impl Tile for i8 {}
impl Tile for i16 {}
impl Tile for i32 {}
impl Tile for i64 {}
