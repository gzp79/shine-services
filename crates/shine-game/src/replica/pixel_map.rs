use crate::replica::{CommandHeader, ReplayState, StableHash, StableHasher};
use serde::{Deserialize, Serialize};

/// Size of the map, in pixels. Constants of the kind: every replica and client must agree on them.
pub const PIXEL_MAP_WIDTH: usize = 32;
pub const PIXEL_MAP_HEIGHT: usize = 32;
/// Number of colors of the palette. Color 0 is the empty pixel.
pub const PIXEL_MAP_COLORS: u8 = 16;

/// A small shared canvas: a grid of palette colors, row by row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PixelMapState {
    pub pixels: Vec<u8>,
}

impl Default for PixelMapState {
    fn default() -> Self {
        Self {
            pixels: vec![0; PIXEL_MAP_WIDTH * PIXEL_MAP_HEIGHT],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PixelMapCommand {
    /// Sets the pixel to a color of the palette; color 0 erases it.
    Paint { x: u8, y: u8, color: u8 },
}

fn in_range(x: u8, y: u8, color: u8) -> bool {
    (x as usize) < PIXEL_MAP_WIDTH && (y as usize) < PIXEL_MAP_HEIGHT && color < PIXEL_MAP_COLORS
}

impl StableHash for PixelMapState {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        (self.pixels.len() as u64).stable_hash(hasher);
        for pixel in &self.pixels {
            pixel.stable_hash(hasher);
        }
    }
}

impl StableHash for PixelMapCommand {
    fn stable_hash(&self, hasher: &mut StableHasher) {
        match self {
            PixelMapCommand::Paint { x, y, color } => {
                0u8.stable_hash(hasher);
                x.stable_hash(hasher);
                y.stable_hash(hasher);
                color.stable_hash(hasher);
            }
        }
    }
}

impl ReplayState for PixelMapState {
    type Command = PixelMapCommand;

    const KIND: &'static str = "pixel-map";

    fn apply(&mut self, _header: &CommandHeader, command: &PixelMapCommand) {
        match command {
            PixelMapCommand::Paint { x, y, color } if in_range(*x, *y, *color) => {
                // `get_mut`: a state decoded from a corrupt snapshot may be of another size.
                if let Some(pixel) = self.pixels.get_mut(*y as usize * PIXEL_MAP_WIDTH + *x as usize) {
                    *pixel = *color;
                }
            }
            PixelMapCommand::Paint { .. } => {}
        }
    }

    fn is_well_formed(command: &PixelMapCommand) -> bool {
        match command {
            PixelMapCommand::Paint { x, y, color } => in_range(*x, *y, *color),
        }
    }
}
