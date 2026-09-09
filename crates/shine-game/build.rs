// On wasm32, pin the linear memory (initial == max == HEAP_BYTES) so it never grows: a growing
// heap swaps Memory.buffer and detaches the zero-copy views handed to JS. See
// temp/wasm-preallocated-heap.md.

include!("src/wasm/heap/config.rs");

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/wasm/heap/config.rs");

    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        println!("cargo:rustc-link-arg=--initial-memory={HEAP_BYTES}");
        println!("cargo:rustc-link-arg=--max-memory={HEAP_BYTES}");
    }
}
