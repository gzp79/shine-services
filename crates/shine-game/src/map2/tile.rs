use std::fmt::Debug;

pub trait Tile: 'static + Default + Debug + Send + Sync {}

impl Tile for u8 {}
impl Tile for u16 {}
impl Tile for u32 {}
impl Tile for u64 {}
impl Tile for i8 {}
impl Tile for i16 {}
impl Tile for i32 {}
impl Tile for i64 {}

#[derive(Debug)]
pub struct VersionedTile<T>
where
    T: Tile,
{
    pub version: u32,
    pub tile: T,
}

impl<T> Default for VersionedTile<T>
where
    T: Tile,
{
    fn default() -> Self {
        Self {
            version: 0,
            tile: T::default(),
        }
    }
}

impl<T> Tile for VersionedTile<T> where T: Tile {}
