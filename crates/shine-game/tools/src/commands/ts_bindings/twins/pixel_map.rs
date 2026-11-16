use postcard_bindgen::PostcardBindings;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PostcardBindings)]
pub struct PixelMapState {
    pub pixels: Vec<u8>,
}

#[derive(Serialize, Deserialize, PostcardBindings)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PixelMapCommand {
    Paint { x: u8, y: u8, color: u8 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::ts_bindings::twins::tests::assert_twin;
    use shine_game::replica::{self, PIXEL_MAP_HEIGHT, PIXEL_MAP_WIDTH};

    #[test]
    fn pixel_map_command_matches_twin() {
        assert_twin::<_, PixelMapCommand>(&replica::PixelMapCommand::Paint { x: 3, y: 4, color: 5 });
    }

    #[test]
    fn pixel_map_state_matches_twin() {
        assert_twin::<_, PixelMapState>(&replica::PixelMapState::default());

        let pixels = (0..PIXEL_MAP_WIDTH * PIXEL_MAP_HEIGHT)
            .map(|i| (i % 251) as u8)
            .collect();
        assert_twin::<_, PixelMapState>(&replica::PixelMapState { pixels });
    }
}
