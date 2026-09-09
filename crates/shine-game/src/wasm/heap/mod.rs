//! JS bindings for the heap metric (`crate::heap`) plus the wasm-only pinned-memory view.

use wasm_bindgen::prelude::*;

mod config;

/// Live allocated bytes.
#[wasm_bindgen]
pub fn heap_used() -> u32 {
    crate::heap::current() as u32
}

/// Peak allocated bytes since boot.
#[wasm_bindgen]
pub fn heap_peak() -> u32 {
    crate::heap::peak() as u32
}

/// Bytes committed to the linear memory (pages * 64 KiB); equals the ceiling once pinned.
#[wasm_bindgen]
pub fn heap_reserved() -> u32 {
    (core::arch::wasm32::memory_size(0) as u32) * 65536
}

/// The pinned ceiling in bytes.
#[wasm_bindgen]
pub fn heap_limit() -> u32 {
    config::HEAP_BYTES as u32
}
