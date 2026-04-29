/* tslint:disable */
/* eslint-disable */

/**
 * WASM wrapper for IndexedMesh - exposes mesh geometry to TypeScript
 */
export class WasmIndexedMesh {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Check if mesh has wire data
     */
    has_wires(): boolean;
    /**
     * Get polygon index buffer
     */
    indices(): Uint32Array;
    /**
     * Get polygon ranges [start0, end0, start1, end1, ...]
     */
    polygon_ranges(): Uint32Array;
    /**
     * Get flat vertex buffer [x,y,x,y,...]
     */
    vertices(): Float32Array;
    /**
     * Get wire index buffer (empty if no wires)
     */
    wire_indices(): Uint32Array;
    /**
     * Get wire ranges [start0, end0, start1, end1, ...]
     */
    wire_ranges(): Uint32Array;
}

export class WasmPatchMesh {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Get the dual mesh (dual polygons)
     */
    dual(): WasmIndexedMesh;
    /**
     * Get the primal mesh (quads with anchor edges as wires)
     */
    primal(): WasmIndexedMesh;
    world_size(): number;
}

/**
 * Generate a hex quad mesh from a JSON config string.
 */
export function generate_mesh(config_json: string): WasmPatchMesh;

export function start(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmindexedmesh_free: (a: number, b: number) => void;
    readonly wasmindexedmesh_has_wires: (a: number) => number;
    readonly wasmindexedmesh_indices: (a: number) => [number, number];
    readonly wasmindexedmesh_polygon_ranges: (a: number) => [number, number];
    readonly wasmindexedmesh_vertices: (a: number) => [number, number];
    readonly wasmindexedmesh_wire_indices: (a: number) => [number, number];
    readonly wasmindexedmesh_wire_ranges: (a: number) => [number, number];
    readonly __wbg_wasmpatchmesh_free: (a: number, b: number) => void;
    readonly generate_mesh: (a: number, b: number) => [number, number, number];
    readonly wasmpatchmesh_dual: (a: number) => number;
    readonly wasmpatchmesh_primal: (a: number) => number;
    readonly wasmpatchmesh_world_size: (a: number) => number;
    readonly start: () => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
