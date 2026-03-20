export interface WasmPatchMesh {
    vertices(): Float32Array;
    indices(): Uint32Array;
    patch_indices(): Uint8Array;
    vertex_count(): number;
    quad_count(): number;
    free(): void;
}
