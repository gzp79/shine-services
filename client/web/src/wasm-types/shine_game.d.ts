/* tslint:disable */
/* eslint-disable */

export class WasmCdt {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    constraints(): Uint32Array;
    error_message(): string | undefined;
    triangles(): Uint32Array;
    vertices(): Float32Array;
}

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

export class WasmWorld {
    free(): void;
    [Symbol.dispose](): void;
    chunk_boundary_indices(q: number, r: number): Uint32Array;
    chunk_dual_polygon_vertices(q: number, r: number): Float32Array;
    chunk_dual_polygons(q: number, r: number): Uint32Array;
    chunk_dual_vertices(q: number, r: number): Float32Array;
    chunk_quad_indices(q: number, r: number): Uint32Array;
    chunk_quad_vertices(q: number, r: number): Float32Array;
    chunk_world_offset(ref_q: number, ref_r: number, q: number, r: number): Float32Array;
    const_cell_world_size(): number;
    const_chunk_world_size(): number;
    init_chunk(q: number, r: number): void;
    constructor();
    remove_chunk(q: number, r: number): void;
}

export class WasmWorldNeighbors {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Returns 12 floats (6 vertices * 2 coords) for the given chunk
     */
    chunk_hex_vertices(chunk_idx: number): Float32Array;
    /**
     * Get edge mesh for the given edge
     */
    edge_mesh(edge_idx: number): WasmIndexedMesh | undefined;
    /**
     * Get interior mesh for the given chunk
     */
    interior_mesh(chunk_idx: number): WasmIndexedMesh | undefined;
    /**
     * Get vertex mesh for the given vertex
     */
    vertex_mesh(vertex_idx: number): WasmIndexedMesh | undefined;
}

/**
 * Generate a CDT from random points and constraint edges.
 * `config_json`: { "n_points": u32, "n_edges": u32, "seed": u32, "bound": i32 }
 */
export function generate_cdt(config_json: string): WasmCdt;

/**
 * Generate a hex quad mesh from a JSON config string.
 */
export function generate_mesh(config_json: string): WasmPatchMesh;

/**
 * Generate world neighbors geometry for visualization
 */
export function generate_world_neighbors(): WasmWorldNeighbors;

export function start(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmpatchmesh_free: (a: number, b: number) => void;
    readonly __wbg_wasmworldneighbors_free: (a: number, b: number) => void;
    readonly generate_mesh: (a: number, b: number) => [number, number, number];
    readonly generate_world_neighbors: () => [number, number, number];
    readonly wasmpatchmesh_dual: (a: number) => number;
    readonly wasmpatchmesh_primal: (a: number) => number;
    readonly wasmpatchmesh_world_size: (a: number) => number;
    readonly wasmworldneighbors_chunk_hex_vertices: (a: number, b: number) => [number, number];
    readonly wasmworldneighbors_edge_mesh: (a: number, b: number) => number;
    readonly wasmworldneighbors_interior_mesh: (a: number, b: number) => number;
    readonly wasmworldneighbors_vertex_mesh: (a: number, b: number) => number;
    readonly start: () => void;
    readonly __wbg_wasmcdt_free: (a: number, b: number) => void;
    readonly generate_cdt: (a: number, b: number) => number;
    readonly wasmcdt_constraints: (a: number) => [number, number];
    readonly wasmcdt_error_message: (a: number) => [number, number];
    readonly wasmcdt_triangles: (a: number) => [number, number];
    readonly wasmcdt_vertices: (a: number) => [number, number];
    readonly __wbg_wasmindexedmesh_free: (a: number, b: number) => void;
    readonly __wbg_wasmworld_free: (a: number, b: number) => void;
    readonly wasmindexedmesh_has_wires: (a: number) => number;
    readonly wasmindexedmesh_indices: (a: number) => [number, number];
    readonly wasmindexedmesh_polygon_ranges: (a: number) => [number, number];
    readonly wasmindexedmesh_vertices: (a: number) => [number, number];
    readonly wasmindexedmesh_wire_indices: (a: number) => [number, number];
    readonly wasmindexedmesh_wire_ranges: (a: number) => [number, number];
    readonly wasmworld_chunk_boundary_indices: (a: number, b: number, c: number) => [number, number];
    readonly wasmworld_chunk_dual_polygon_vertices: (a: number, b: number, c: number) => [number, number];
    readonly wasmworld_chunk_dual_polygons: (a: number, b: number, c: number) => [number, number];
    readonly wasmworld_chunk_dual_vertices: (a: number, b: number, c: number) => [number, number];
    readonly wasmworld_chunk_quad_indices: (a: number, b: number, c: number) => [number, number];
    readonly wasmworld_chunk_quad_vertices: (a: number, b: number, c: number) => [number, number];
    readonly wasmworld_chunk_world_offset: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmworld_const_cell_world_size: (a: number) => number;
    readonly wasmworld_const_chunk_world_size: (a: number) => number;
    readonly wasmworld_init_chunk: (a: number, b: number, c: number) => void;
    readonly wasmworld_new: () => number;
    readonly wasmworld_remove_chunk: (a: number, b: number, c: number) => void;
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
