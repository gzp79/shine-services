/* tslint:disable */
/* eslint-disable */

export class WasmCdt {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    error_message(): string | undefined;
    fixed_edges(): Uint32Array;
    has_error(): boolean;
    triangle_count(): number;
    triangles(): Uint32Array;
    vertex_count(): number;
    vertices(): Float32Array;
}

export class WasmPatchMesh {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Number of dual edges
     */
    dual_edge_count(): number;
    /**
     * Flat dual edge indices [a, b, ...] (2 indices per dual edge)
     */
    dual_indices(): Uint32Array;
    /**
     * Number of dual vertices
     */
    dual_vertex_count(): number;
    /**
     * Flat dual vertex positions [x, y, x, y, ...] (2 floats per vertex, one per primal quad centroid)
     */
    dual_vertices(): Float32Array;
    /**
     * Patch index per quad (0 for all currently)
     */
    patch_indices(): Uint8Array;
    /**
     * Number of quads
     */
    quad_count(): number;
    /**
     * Flat quad indices [a, b, c, d, ...] (4 indices per quad)
     */
    quad_indices(): Uint32Array;
    /**
     * Number of vertices
     */
    vertex_count(): number;
    /**
     * Flat vertex positions [x, y, x, y, ...] (2 floats per vertex)
     */
    vertices(): Float32Array;
    /**
     * The world size used to generate this mesh
     */
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
    init_chunk(q: number, r: number): void;
    constructor();
    remove_chunk(q: number, r: number): void;
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

export function start(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmpatchmesh_free: (a: number, b: number) => void;
    readonly generate_mesh: (a: number, b: number) => [number, number, number];
    readonly wasmpatchmesh_dual_edge_count: (a: number) => number;
    readonly wasmpatchmesh_dual_indices: (a: number) => [number, number];
    readonly wasmpatchmesh_dual_vertex_count: (a: number) => number;
    readonly wasmpatchmesh_dual_vertices: (a: number) => [number, number];
    readonly wasmpatchmesh_patch_indices: (a: number) => [number, number];
    readonly wasmpatchmesh_quad_count: (a: number) => number;
    readonly wasmpatchmesh_quad_indices: (a: number) => [number, number];
    readonly wasmpatchmesh_vertex_count: (a: number) => number;
    readonly wasmpatchmesh_vertices: (a: number) => [number, number];
    readonly wasmpatchmesh_world_size: (a: number) => number;
    readonly __wbg_wasmworld_free: (a: number, b: number) => void;
    readonly wasmworld_chunk_boundary_indices: (a: number, b: number, c: number) => [number, number];
    readonly wasmworld_chunk_dual_polygon_vertices: (a: number, b: number, c: number) => [number, number];
    readonly wasmworld_chunk_dual_polygons: (a: number, b: number, c: number) => [number, number];
    readonly wasmworld_chunk_dual_vertices: (a: number, b: number, c: number) => [number, number];
    readonly wasmworld_chunk_quad_indices: (a: number, b: number, c: number) => [number, number];
    readonly wasmworld_chunk_quad_vertices: (a: number, b: number, c: number) => [number, number];
    readonly wasmworld_chunk_world_offset: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmworld_init_chunk: (a: number, b: number, c: number) => void;
    readonly wasmworld_new: () => number;
    readonly wasmworld_remove_chunk: (a: number, b: number, c: number) => void;
    readonly __wbg_wasmcdt_free: (a: number, b: number) => void;
    readonly generate_cdt: (a: number, b: number) => number;
    readonly wasmcdt_error_message: (a: number) => [number, number];
    readonly wasmcdt_fixed_edges: (a: number) => [number, number];
    readonly wasmcdt_has_error: (a: number) => number;
    readonly wasmcdt_triangle_count: (a: number) => number;
    readonly wasmcdt_triangles: (a: number) => [number, number];
    readonly wasmcdt_vertex_count: (a: number) => number;
    readonly wasmcdt_vertices: (a: number) => [number, number];
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
