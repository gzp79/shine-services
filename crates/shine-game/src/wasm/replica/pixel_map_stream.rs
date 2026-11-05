use crate::replica::{
    ClientStream, PixelMapCommand, PixelMapState, StreamId, StreamStatus, PIXEL_MAP_COLORS, PIXEL_MAP_HEIGHT,
    PIXEL_MAP_WIDTH,
};
use uuid::Uuid;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TS_WASM_PIXEL_MAP_STREAM: &str = r#"
export type PixelMapStreamStatus = "waiting" | "live" | "unsynced";

interface PixelMapStream {
    status(): PixelMapStreamStatus;
}
"#;

/// The pixel map stream of one client. It moves no data: the caller sends the frames it returns and
/// feeds it the frames of the server.
#[wasm_bindgen(js_name = "PixelMapStream")]
pub struct WasmPixelMapStream {
    stream: ClientStream<PixelMapState>,
}

fn parse_uuid(text: &str) -> Result<Uuid, JsError> {
    Uuid::parse_str(text).map_err(|_| JsError::new("not a uuid"))
}

#[wasm_bindgen(js_class = "PixelMapStream")]
impl WasmPixelMapStream {
    #[wasm_bindgen(constructor)]
    pub fn new(stream_id: &str, user_id: &str) -> Result<WasmPixelMapStream, JsError> {
        let stream_id = StreamId::parse(stream_id).map_err(|err| JsError::new(&err.to_string()))?;
        Ok(Self {
            stream: ClientStream::new(stream_id, parse_uuid(user_id)?),
        })
    }

    pub fn subscribe_frame(&self) -> String {
        self.stream.subscribe_frame()
    }

    /// Starts over as after a connect; returns the subscribe frame to send.
    pub fn resync(&mut self) -> String {
        self.stream.resync()
    }

    /// Reacts to a text frame of the server; returns the frames to send back.
    pub fn handle_message(&mut self, text: &str) -> Vec<String> {
        self.stream.handle_message(text)
    }

    /// Paints a pixel: it shows at once and is pending until the server confirms it. Returns the frame
    /// to send. Throws when the stream is not live or the arguments are not valid.
    pub fn paint(&mut self, x: u32, y: u32, color: u32, command_id: &str) -> Result<String, JsError> {
        if x as usize >= PIXEL_MAP_WIDTH || y as usize >= PIXEL_MAP_HEIGHT || color >= PIXEL_MAP_COLORS as u32 {
            return Err(JsError::new(
                "the pixel is off the map or the color is not in the palette",
            ));
        }
        let command = PixelMapCommand::Paint {
            x: x as u8,
            y: y as u8,
            color: color as u8,
        };
        self.stream
            .submit(parse_uuid(command_id)?, command)
            .map_err(|err| JsError::new(&err.to_string()))
    }

    #[wasm_bindgen(skip_typescript)]
    pub fn status(&self) -> String {
        match self.stream.status() {
            StreamStatus::Waiting => "waiting",
            StreamStatus::Live => "live",
            StreamStatus::Unsynced => "unsynced",
        }
        .to_string()
    }

    /// Changes whenever the colors or the pending mask change.
    pub fn revision(&self) -> u32 {
        self.stream.revision()
    }

    /// The version of the confirmed state.
    pub fn version(&self) -> f64 {
        self.stream.version() as f64
    }

    pub fn pending_count(&self) -> u32 {
        self.stream.pending_count() as u32
    }

    /// The colors as the user sees them (confirmed and pending), row by row.
    pub fn colors(&self) -> Vec<u8> {
        self.stream.view().pixels
    }

    /// One byte per pixel, 1 where a paint of this client is not confirmed yet.
    pub fn pending_mask(&self) -> Vec<u8> {
        let mut mask = vec![0; PIXEL_MAP_WIDTH * PIXEL_MAP_HEIGHT];
        for PixelMapCommand::Paint { x, y, .. } in self.stream.pending_commands() {
            if let Some(cell) = mask.get_mut(*y as usize * PIXEL_MAP_WIDTH + *x as usize) {
                *cell = 1;
            }
        }
        mask
    }

    pub fn width(&self) -> u32 {
        PIXEL_MAP_WIDTH as u32
    }

    pub fn height(&self) -> u32 {
        PIXEL_MAP_HEIGHT as u32
    }

    pub fn palette_size(&self) -> u32 {
        PIXEL_MAP_COLORS as u32
    }
}
