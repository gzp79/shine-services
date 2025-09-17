use crate::map::{AxialCoord, HexLayer, HexLayerConfig, MapError, MapLayer, Tile};
use bevy::ecs::component::Component;
use std::collections::HashMap;

/// A 2d hexagonal grid of tiles with a default value and a sparse memory layout for the non-default tiles.
#[derive(Component)]
pub struct HexSparseLayer<T>
where
    T: Tile,
{
    radius: u32,
    default: T,
    data: HashMap<AxialCoord, T>,
}

impl<T> HexSparseLayer<T>
where
    T: Tile,
{
    pub fn new(config: &HexLayerConfig<T>) -> Self {
        Self {
            radius: config.radius,
            default: <T as Default>::default(),
            data: HashMap::new(),
        }
    }

    pub fn default(&self) -> &T {
        &self.default
    }

    pub fn get_mut(&mut self, coord: AxialCoord) -> &mut T {
        //todo: return some Entry like api to avoid creation of default tile if not needed
        if self.is_in_bounds(coord) {
            self.data.entry(coord).or_insert_with(|| self.default.clone())
        } else {
            panic!("Out of bounds access");
        }
    }

    pub fn occupied(&self) -> impl Iterator<Item = (AxialCoord, &T)> {
        self.data.iter().map(|(coord, tile)| (*coord, tile))
    }
}

impl<T> MapLayer for HexSparseLayer<T>
where
    T: Tile,
{
    type Tile = T;

    fn new_empty() -> Self
    where
        Self: Sized,
    {
        Self {
            radius: 0,
            default: <T as Default>::default(),
            data: HashMap::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.radius == 0
    }

    fn clear(&mut self) {
        self.radius = 0;
        self.data.clear();
    }

    fn load(&mut self, data: &[u8]) -> Result<(), MapError> {
        todo!()
    }

    fn save(&self) -> Vec<u8> {
        todo!()
    }
}

impl<T> From<HexLayerConfig<T>> for HexSparseLayer<T>
where
    T: Tile,
{
    fn from(config: HexLayerConfig<T>) -> Self {
        Self::new(&config)
    }
}

impl<T> HexLayer for HexSparseLayer<T>
where
    T: Tile,
{
    fn radius(&self) -> u32 {
        self.radius
    }

    fn try_get(&self, coord: AxialCoord) -> Option<&Self::Tile> {
        if self.is_in_bounds(coord) {
            self.data.get(&coord).or(Some(&self.default))
        } else {
            None
        }
    }

    fn get(&self, coord: AxialCoord) -> &Self::Tile {
        if self.is_in_bounds(coord) {
            self.data.get(&coord).unwrap_or(&self.default)
        } else {
            panic!("Out of bounds access");
        }
    }
}
