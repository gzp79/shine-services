/* tslint:disable */
/* eslint-disable */

interface WasmWorld {
    chunk_world_offset(ref_q: number, ref_r: number, q: number, r: number): [number, number];
}



/**
 * Which side of a CornerCells polygon a tile belongs to. Matches Rust CornerSide indices exactly.
 */
export enum CornerSide {
    Owner = 0,
    CcwNeighbor = 1,
    CwNeighbor = 2,
}

/**
 * Which side of an EdgeCells polygon a tile belongs to. Matches Rust EdgeSide indices exactly.
 */
export enum EdgeSide {
    Owner = 0,
    Neighbor = 1,
}

/**
 * 6 neighbor direction for a flat-topped hex grid in CCW order. Matches Rust HexFlatDir indices exactly.
 */
export enum HexFlatDir {
    NE = 0,
    N = 1,
    NW = 2,
    SW = 3,
    S = 4,
    SE = 5,
}

/**
 * 6 neighbor direction for a pointy-topped hex grid in CCW order. Matches Rust HexPointyDir indices exactly.
 */
export enum HexPointyDir {
    E = 0,
    NE = 1,
    NW = 2,
    W = 3,
    SW = 4,
    SE = 5,
}

export class WasmCdtMesh {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    constraints(): Uint32Array;
    error_message(): string | undefined;
    triangles(): Uint32Array;
    vertices(): Float32Array;
}

/**
 * Handle to a loaded chunk. Wraps a core `ChunkHandle`, which holds only weak references into
 * the world and revalidates on every access, so a stale handle returns `undefined` instead of
 * reading moved or freed data.
 */
export class WasmChunk {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    corner_cells(corner_idx: HexPointyDir): WasmCornerCells | undefined;
    edge_cells(edge_idx: HexFlatDir): WasmEdgeCells | undefined;
    /**
     * The 6 hexagon boundary corners in chunk-local space as 12 floats `[x, y, ...]`, or
     * `undefined` if the handle is stale.
     */
    hex_vertices(): Float32Array | undefined;
    inner_cells(): WasmInnerCells | undefined;
}

/**
 * Zero-copy WASM view over a CornerCells snapshot. Accessors return views into Wasm linear memory
 * (clone on the JS side to outlive the next call), or `undefined` once the source chunk changed.
 */
export class WasmCornerCells {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    cell_ids(): Uint32Array | undefined;
    /**
     * Packed [tile_id, vertex, tile_id, vertex, ...] pairs of every quad bordering `cell_id` on the given `side`.
     */
    cell_tiles(side: CornerSide, cell_id: number): Uint32Array | undefined;
    indices(): Uint32Array | undefined;
    ranges(): Uint32Array | undefined;
    tile_distortions(): Float32Array | undefined;
    tile_ids(): Uint32Array | undefined;
    tile_vertices(): Uint8Array | undefined;
    /**
     * Whether the source chunk is unchanged; `false` means every accessor returns `undefined`.
     */
    valid(): boolean;
    vertices(): Float32Array | undefined;
}

/**
 * Zero-copy WASM view over an EdgeCells snapshot. Accessors return views into Wasm linear memory
 * (clone on the JS side to outlive the next call), or `undefined` once the source chunk changed.
 */
export class WasmEdgeCells {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    cell_ids(): Uint32Array | undefined;
    /**
     * Packed [tile_id, vertex, tile_id, vertex, ...] pairs of every quad bordering `cell_id` on the given `side`.
     */
    cell_tiles(side: EdgeSide, cell_id: number): Uint32Array | undefined;
    indices(): Uint32Array | undefined;
    ranges(): Uint32Array | undefined;
    tile_distortions(): Float32Array | undefined;
    tile_ids(): Uint32Array | undefined;
    tile_vertices(): Uint8Array | undefined;
    /**
     * Whether the source chunk is unchanged; `false` means every accessor returns `undefined`.
     */
    valid(): boolean;
    vertices(): Float32Array | undefined;
}

export class WasmHexMesh {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    dual(): WiredPolygonMeshHandle;
    primal(): WiredPolygonMeshHandle;
    world_size(): number;
}

/**
 * Zero-copy WASM view over an InnerCells snapshot. Accessors return views into Wasm linear memory
 * (clone on the JS side to outlive the next call), or `undefined` once the source chunk changed.
 */
export class WasmInnerCells {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    cell_ids(): Uint32Array | undefined;
    /**
     * Packed [tile_id, vertex, tile_id, vertex, ...] pairs of every quad bordering `cell_id`.
     */
    cell_tiles(cell_id: number): Uint32Array | undefined;
    indices(): Uint32Array | undefined;
    ranges(): Uint32Array | undefined;
    tile_distortions(): Float32Array | undefined;
    tile_ids(): Uint32Array | undefined;
    tile_vertices(): Uint8Array | undefined;
    /**
     * Whether the source chunk is unchanged; `false` means every accessor returns `undefined`.
     */
    valid(): boolean;
    vertices(): Float32Array | undefined;
}

/**
 * The exported world root. Wraps the core `World` handle; the `Rc<RefCell<..>>` graph and all
 * staleness tracking live in core, so this layer only marshals ids and forwards calls.
 */
export class WasmWorld {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Handle to a loaded chunk, or `undefined` if no chunk is loaded at `(q, r)`.
     */
    chunk(q: number, r: number): WasmChunk | undefined;
    const_cell_world_size(): number;
    const_chunk_world_size(): number;
    init_chunk(q: number, r: number): void;
    constructor();
    remove_chunk(q: number, r: number): void;
}

/**
 * Zero-copy WASM view over a WiredPolygonMesh.
 * All accessors return views into Wasm linear memory — clone on the JS side
 * (e.g. `arr.slice()`) if the data must outlive this object or any further Wasm call.
 */
export class WiredPolygonMeshHandle {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    has_wires(): boolean;
    indices(): Uint32Array;
    ranges(): Uint32Array;
    vertices(): Float32Array;
    wire_indices(): Uint32Array;
    wire_ranges(): Uint32Array;
}

/**
 * Generate a CDT from random points and constraint edges.
 * `config_json`: { "n_points": u32, "n_edges": u32, "seed": u32, "bound": i32 }
 */
export function generate_cdt(config_json: string): WasmCdtMesh;

/**
 * Generate a hex quad mesh from a JSON config string.
 */
export function generate_mesh(config_json: string): WasmHexMesh;

/**
 * Axial distance between two hex coordinates.
 */
export function hex_distance(aq: number, ar: number, bq: number, br: number): number;

/**
 * Nearest flat-top hex [q, r] for world position (x, y) with given circumradius size.
 * Inverse of hex_flat_to_position.
 */
export function hex_flat_from_position(x: number, y: number, size: number): Int32Array;

/**
 * Neighbor of (q, r) in the given flat-top direction. Returns [q, r].
 */
export function hex_flat_neighbor(q: number, r: number, dir: HexFlatDir): Int32Array;

/**
 * World position [x, y] of the flat-top hex center at (q, r) with given circumradius size.
 */
export function hex_flat_to_position(q: number, r: number, size: number): Float32Array;

/**
 * Nearest pointy-top hex [q, r] for world position (x, y) with given circumradius size.
 * Inverse of hex_pointy_to_position.
 */
export function hex_pointy_from_position(x: number, y: number, size: number): Int32Array;

/**
 * Neighbor of (q, r) in the given pointy-top direction. Returns [q, r].
 */
export function hex_pointy_neighbor(q: number, r: number, dir: HexPointyDir): Int32Array;

/**
 * World position [x, y] of the pointy-top hex center at (q, r) with given circumradius size.
 */
export function hex_pointy_to_position(q: number, r: number, size: number): Float32Array;

/**
 * Flat [q0,r0, q1,r1, ...] for the ring at given radius from (q, r).
 * Order: starts at direction-0 corner, walks CCW — matches Rust RingIterator.
 */
export function hex_ring(q: number, r: number, radius: number): Int32Array;

export function memory_info(): any;

export function start(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmchunk_free: (a: number, b: number) => void;
    readonly __wbg_wasmworld_free: (a: number, b: number) => void;
    readonly hex_distance: (a: number, b: number, c: number, d: number) => number;
    readonly hex_flat_from_position: (a: number, b: number, c: number) => [number, number];
    readonly hex_flat_neighbor: (a: number, b: number, c: number) => [number, number];
    readonly hex_flat_to_position: (a: number, b: number, c: number) => [number, number];
    readonly hex_pointy_from_position: (a: number, b: number, c: number) => [number, number];
    readonly hex_pointy_to_position: (a: number, b: number, c: number) => [number, number];
    readonly hex_ring: (a: number, b: number, c: number) => [number, number];
    readonly wasmchunk_corner_cells: (a: number, b: number) => number;
    readonly wasmchunk_edge_cells: (a: number, b: number) => number;
    readonly wasmchunk_hex_vertices: (a: number) => [number, number];
    readonly wasmchunk_inner_cells: (a: number) => number;
    readonly wasmworld_chunk: (a: number, b: number, c: number) => number;
    readonly wasmworld_chunk_world_offset: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly wasmworld_const_cell_world_size: (a: number) => number;
    readonly wasmworld_const_chunk_world_size: (a: number) => number;
    readonly wasmworld_init_chunk: (a: number, b: number, c: number) => void;
    readonly wasmworld_new: () => number;
    readonly wasmworld_remove_chunk: (a: number, b: number, c: number) => void;
    readonly hex_pointy_neighbor: (a: number, b: number, c: number) => [number, number];
    readonly __wbg_wasmcornercells_free: (a: number, b: number) => void;
    readonly __wbg_wasmedgecells_free: (a: number, b: number) => void;
    readonly __wbg_wasminnercells_free: (a: number, b: number) => void;
    readonly start: () => void;
    readonly wasmcornercells_cell_ids: (a: number) => any;
    readonly wasmcornercells_cell_tiles: (a: number, b: number, c: number) => any;
    readonly wasmcornercells_indices: (a: number) => any;
    readonly wasmcornercells_ranges: (a: number) => any;
    readonly wasmcornercells_tile_distortions: (a: number) => any;
    readonly wasmcornercells_tile_ids: (a: number) => any;
    readonly wasmcornercells_tile_vertices: (a: number) => any;
    readonly wasmcornercells_valid: (a: number) => number;
    readonly wasmcornercells_vertices: (a: number) => any;
    readonly wasmedgecells_cell_ids: (a: number) => any;
    readonly wasmedgecells_cell_tiles: (a: number, b: number, c: number) => any;
    readonly wasmedgecells_indices: (a: number) => any;
    readonly wasmedgecells_ranges: (a: number) => any;
    readonly wasmedgecells_tile_distortions: (a: number) => any;
    readonly wasmedgecells_tile_ids: (a: number) => any;
    readonly wasmedgecells_tile_vertices: (a: number) => any;
    readonly wasmedgecells_valid: (a: number) => number;
    readonly wasmedgecells_vertices: (a: number) => any;
    readonly wasminnercells_cell_ids: (a: number) => any;
    readonly wasminnercells_cell_tiles: (a: number, b: number) => any;
    readonly wasminnercells_indices: (a: number) => any;
    readonly wasminnercells_ranges: (a: number) => any;
    readonly wasminnercells_tile_distortions: (a: number) => any;
    readonly wasminnercells_tile_ids: (a: number) => any;
    readonly wasminnercells_tile_vertices: (a: number) => any;
    readonly wasminnercells_valid: (a: number) => number;
    readonly wasminnercells_vertices: (a: number) => any;
    readonly memory_info: () => any;
    readonly __wbg_wasmcdtmesh_free: (a: number, b: number) => void;
    readonly __wbg_wasmhexmesh_free: (a: number, b: number) => void;
    readonly __wbg_wiredpolygonmeshhandle_free: (a: number, b: number) => void;
    readonly generate_cdt: (a: number, b: number) => number;
    readonly generate_mesh: (a: number, b: number) => [number, number, number];
    readonly wasmcdtmesh_constraints: (a: number) => any;
    readonly wasmcdtmesh_error_message: (a: number) => [number, number];
    readonly wasmcdtmesh_triangles: (a: number) => any;
    readonly wasmcdtmesh_vertices: (a: number) => any;
    readonly wasmhexmesh_dual: (a: number) => number;
    readonly wasmhexmesh_primal: (a: number) => number;
    readonly wasmhexmesh_world_size: (a: number) => number;
    readonly wiredpolygonmeshhandle_has_wires: (a: number) => number;
    readonly wiredpolygonmeshhandle_indices: (a: number) => any;
    readonly wiredpolygonmeshhandle_ranges: (a: number) => any;
    readonly wiredpolygonmeshhandle_vertices: (a: number) => any;
    readonly wiredpolygonmeshhandle_wire_indices: (a: number) => any;
    readonly wiredpolygonmeshhandle_wire_ranges: (a: number) => any;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
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
