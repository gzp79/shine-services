use crate::world::ChangeLog;
use js_sys::Uint32Array;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = "ChangeLog")]
pub struct WasmChangeLog(ChangeLog<u32>);

#[wasm_bindgen]
impl WasmChangeLog {
    /// All tile values in the layer, dense per tile id.
    pub fn values(&self) -> Uint32Array {
        unsafe { Uint32Array::view(self.0.values()) }
    }

    /// Change-log bit count (matches `values().length`); TS wraps `len`/`packed_words` into a
    /// `BitSetLike`.
    pub fn len(&self) -> usize {
        self.0.log().len()
    }

    /// Internal packed change-log storage: one bit per tile id, set if it changed during
    /// `update`. Not a public bit-set API by itself — TS wraps it via `asBitSet`.
    pub fn packed_words(&self) -> Uint32Array {
        unsafe { Uint32Array::view(self.0.log().packed_words()) }
    }
}

impl From<ChangeLog<u32>> for WasmChangeLog {
    fn from(change_log: ChangeLog<u32>) -> Self {
        Self(change_log)
    }
}
