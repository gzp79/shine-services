// Fixed size of the wasm linear memory, in bytes; must be a multiple of the 64 KiB wasm page.
// Included verbatim by build.rs to pin initial == max. Tune above the observed `heap_peak` with
// margin; overflow becomes an OOM abort rather than a silent Memory.buffer swap.
pub const HEAP_BYTES: usize = 256 * 1024 * 1024;
