use crate::map::{MapError, MapLayer, MapLayerIO, RectCoord, RectLayer, RectLayerConfig, Tile, VoldemortIOToken};
use bevy::ecs::component::Component;
use serde::{Deserialize, Serialize};
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
    type Config = RectLayerConfig<T>;

    fn new() -> Self
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

    fn clear(&mut self) {
        self.width = 0;
        self.height = 0;
        self.data.clear();
    }

    fn initialize(&mut self, config: &Self::Config) {
        self.width = config.width;
        self.height = config.height;
        self.default = <T as Default>::default();
        self.data.clear();
    }

    fn is_empty(&self) -> bool {
        self.width == 0 && self.height == 0
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

impl<T> MapLayerIO for RectSparseLayer<T>
where
    T: Tile,
{
    fn load(&mut self, bytes: &[u8], _token: VoldemortIOToken) -> Result<(), MapError> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[serde(bound = "U: Tile")]
        struct SnapshotV1<U>
        where
            U: Tile,
        {
            width: u32,
            height: u32,
            data: HashMap<RectCoord, U>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[serde(bound = "U: Tile")]
        struct Snapshot<U>
        where
            U: Tile,
        {
            v1: Option<SnapshotV1<U>>,
        }

        let decoded: Snapshot<T> = rmp_serde::from_slice(bytes).map_err(MapError::LoadLayerError)?;
        if let Some(decoded) = decoded.v1 {
            self.width = decoded.width;
            self.height = decoded.height;
            self.data = decoded.data;
            Ok(())
        } else {
            Err(MapError::LoadLayerError(rmp_serde::decode::Error::Syntax(
                "Unsupported snapshot version".into(),
            )))
        }
    }

    fn save(&self, _token: VoldemortIOToken) -> Result<Vec<u8>, MapError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        #[serde(bound = "U: Tile")]
        struct SnapshotLatest<'a, U>
        where
            U: Tile,
        {
            width: u32,
            height: u32,
            data: &'a HashMap<RectCoord, U>,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        #[serde(bound = "U: Tile")]
        struct Snapshot<'a, U>
        where
            U: Tile,
        {
            v1: Option<SnapshotLatest<'a, U>>,
        }

        rmp_serde::to_vec(&Snapshot {
            v1: Some(SnapshotLatest {
                width: self.width,
                height: self.height,
                data: &self.data,
            }),
        })
        .map_err(MapError::SaveLayerError)
    }
}
