use crate::map::{MapError, MapLayer, RectCoord, RectLayer, RectLayerConfig, Tile};
use bevy::ecs::component::Component;
use std::collections::HashMap;

/// A 2d rectangular grid of tiles with a default value and a sparse memory layout for the non-default tiles.
#[derive(Component)]
pub struct RectSparseLayer<T>
where
    T: Tile,
{
    width: u32,
    height: u32,
    default: T,
    data: HashMap<RectCoord, T>,
}

impl<T> RectSparseLayer<T>
where
    T: Tile,
{
    pub fn new(config: &RectLayerConfig<T>) -> Self {
        Self {
            width: config.width,
            height: config.height,
            default: <T as Default>::default(),
            data: HashMap::new(),
        }
    }

    pub fn default(&self) -> &T {
        &self.default
    }

    pub fn get_mut(&mut self, coord: RectCoord) -> &mut T {
        //todo: return some Entry like api to avoid creation of default tile if not needed
        if self.is_in_bounds(coord) {
            self.data.entry(coord).or_insert_with(|| self.default.clone())
        } else {
            panic!("Out of bounds access");
        }
    }

    pub fn occupied(&self) -> impl Iterator<Item = (RectCoord, &T)> {
        self.data.iter().map(|(coord, tile)| (*coord, tile))
    }
}

impl<T> MapLayer for RectSparseLayer<T>
where
    T: Tile,
{
    type Tile = T;

    fn new_empty() -> Self
    where
        Self: Sized,
    {
        Self {
            width: 0,
            height: 0,
            default: <T as Default>::default(),
            data: HashMap::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.width == 0 && self.height == 0
    }

    fn clear(&mut self) {
        self.width = 0;
        self.height = 0;
        self.data.clear();
    }

    fn load(&mut self, data: &[u8]) -> Result<(), MapError> {
        todo!()
    }

    fn save(&self) -> Vec<u8> {
        todo!()
    }
}

impl<T> From<RectLayerConfig<T>> for RectSparseLayer<T>
where
    T: Tile,
{
    fn from(config: RectLayerConfig<T>) -> Self {
        Self::new(&config)
    }
}

impl<T> RectLayer for RectSparseLayer<T>
where
    T: Tile,
{
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    fn try_get(&self, coord: RectCoord) -> Option<&Self::Tile> {
        if self.is_in_bounds(coord) {
            self.data.get(&coord).or(Some(&self.default))
        } else {
            None
        }
    }

    fn get(&self, coord: RectCoord) -> &Self::Tile {
        if self.is_in_bounds(coord) {
            self.data.get(&coord).unwrap_or(&self.default)
        } else {
            panic!("Out of bounds access");
        }
    }
}
