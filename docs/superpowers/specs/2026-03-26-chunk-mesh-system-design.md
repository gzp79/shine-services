# Chunk Mesh System Design

## Overview

Add chunk mesh rendering to the game client. When a chunk is loaded, the game calls WASM to initialize the chunk, queries mesh data (vertices, quad indices, border edges) and transformation via `WasmWorld` methods, then builds a Three.js group with filled quads and a border outline.

## Conventions

- **2D-to-3D mapping**: All 2D coordinates `(x, y)` from WASM map to Three.js as `(x, 0, y)` — the XZ plane with y=0. This applies to both vertex positions and chunk offsets.
- **Coordinate values**: `CHUNK_WORLD_SIZE = 1000.0` (meters), `SUBDIVISION_BASE = 4`, `SUBDIVISION_COUNT = 16`.

## Decisions

- **All data accessed via `WasmWorld` methods** — no separate WASM chunk struct. TS uses `ChunkId` to reference chunks and wraps convenience in a `Chunk` class.
- **`ChunkId` stays `usize`** with default origin `(1024, 1024)`. Relative position calculations cast to `i32` on the Rust side.
- **WASM uses `u32` for chunk coordinates** — breaking change from existing `i32` in `init_chunk`. Since `ChunkId` is always non-negative (origin 1024), `u32` is the correct type.
- **Visuals: filled quads + border outline only** — no wireframe, no dual edges. The existing `patchIndices` and dual graph data from `MeshData` are intentionally not used.
- **Chunk transformation** is a method on `WasmWorld` taking reference chunk first, target chunk second. Returns 2D world offset. Sign convention: the returned offset is the **position of the target chunk** in a coordinate system centered on the reference chunk.
- **Reference chunk** stored in `Game` class, defaults to `ChunkId.ORIGIN` (1024, 1024).
- **`group.userData`** stores `{ chunkId: { q, r } }`.
- **Uninitialized chunk queries** return empty arrays (zero-length typed arrays). WASM methods do not panic on missing chunks.
- **Chunk lifecycle**: `Chunk` (TS) must not outlive `WasmWorld`. `Game.destroy()` clears all `ChunkObject` references before freeing the world.

## Rust / WASM Layer

### ChunkId

`ChunkId` remains `(usize, usize)`. Default origin: `(1024, 1024)`. `relative_axial_coord` already handles signed math via `isize` cast. The existing `init_chunk` signature changes from `i32` to `u32` to match.

### WasmWorld API

New methods on `WasmWorld`:

```rust
#[wasm_bindgen]
impl WasmWorld {
    // Existing (signature changes from i32 to u32)
    pub fn init_chunk(&mut self, q: u32, r: u32);

    // Remove chunk from world, freeing Rust-side memory
    pub fn remove_chunk(&mut self, q: u32, r: u32);

    // Flat [x, y, x, y, ...] 2D positions, 2 floats per vertex
    pub fn chunk_vertices(&self, q: u32, r: u32) -> Float32Array;

    // Flat [a, b, c, d, ...] 4 indices per quad
    pub fn chunk_quad_indices(&self, q: u32, r: u32) -> Uint32Array;

    // Flat [a, b, a, b, ...] 2 indices per border edge segment
    pub fn chunk_border_indices(&self, q: u32, r: u32) -> Uint32Array;

    // Returns [x, y] world offset of chunk (q, r) relative to reference (ref_q, ref_r)
    pub fn chunk_world_offset(&self, ref_q: u32, ref_r: u32, q: u32, r: u32) -> Float32Array;
}
```

### Border Edge Extraction

Add a method to `Chunk` or `QuadTopology` that identifies boundary edges. After ghost quad construction, real quad edges always have a neighbor — so boundary edges are edges where the neighbor is a **ghost quad** (use `is_ghost_quad(neighbor)`), not edges with `None` neighbors. Returns pairs of vertex indices forming border line segments.

### Error Handling

All chunk query methods (`chunk_vertices`, `chunk_quad_indices`, `chunk_border_indices`, `chunk_world_offset`) return empty/zero-length typed arrays when the chunk has not been initialized. No panics.

### World Offset Calculation

Uses existing `AxialCoord::world_coordinate(hex_size)`:

```rust
fn chunk_world_offset(reference: ChunkId, chunk: ChunkId) -> Vec2 {
    let rel = reference.relative_axial_coord(chunk);
    rel.world_coordinate(CHUNK_WORLD_SIZE)
}
```

## TypeScript Layer

### `ChunkId` update (`world/types.ts`)

Add default origin constant:

```ts
export class ChunkId {
    static readonly ORIGIN = new ChunkId(1024, 1024);
    // ... existing constructor, key()
}
```

### `Chunk` class (`world/chunk.ts`)

Thin wrapper around `WasmWorld` + `ChunkId`:

```ts
export class Chunk {
    constructor(private readonly world: WasmWorld, readonly id: ChunkId) {}

    vertices(): Float32Array {
        return this.world.chunk_vertices(this.id.q, this.id.r);
    }

    quadIndices(): Uint32Array {
        return this.world.chunk_quad_indices(this.id.q, this.id.r);
    }

    borderIndices(): Uint32Array {
        return this.world.chunk_border_indices(this.id.q, this.id.r);
    }

    worldOffset(ref: ChunkId): Float32Array {
        return this.world.chunk_world_offset(ref.q, ref.r, this.id.q, this.id.r);
    }
}
```

### `ChunkObject` interface and `Game` class (`game.ts`)

Modifies existing `ChunkObject` interface (adds `chunk` field):

```ts
interface ChunkObject {
    chunk: Chunk;
    group: THREE.Group;
    dispose(): void;
}

class Game {
    private readonly referenceChunk = ChunkId.ORIGIN;
    private readonly chunks = new Map<string, ChunkObject>();
    // ...
}
```

`Game.init()` changes from `new ChunkId(0, 0)` to `ChunkId.ORIGIN`.

`Game.destroy()` clears all chunk references before calling `world.free()`.

### Chunk Mesh Builder (`world/chunk-mesh-builder.ts`)

New file. Takes a `Chunk`, returns `{ group: THREE.Group, dispose(): void }`.

- Builds filled quad mesh: 2D vertices mapped to XZ plane (y=0), each quad split into 2 triangles
- Builds border outline as `THREE.LineSegments` from border edge indices
- Returns group containing both meshes + dispose function for cleanup

## Data Flow

```
loadChunk(id)
    |
    +- world.init_chunk(q, r)              // Rust: creates Chunk, stores in World
    |
    +- chunk = new Chunk(world, id)
    |
    +- offset = chunk.worldOffset(referenceChunk)
    |                                       // Rust: relative_axial_coord -> world_coordinate
    |
    +- buildChunkMesh(chunk)               // JS: extracts vertices, indices, border
    |   +- chunk.vertices()                //   -> Float32Array from WASM
    |   +- chunk.quadIndices()             //   -> Uint32Array from WASM
    |   +- chunk.borderIndices()           //   -> Uint32Array from WASM
    |   +- build filled quad geometry      //   -> THREE.Mesh
    |   +- build border line geometry      //   -> THREE.LineSegments
    |   +- return { group, dispose }
    |
    +- group.position.set(offset[0], 0, offset[1])
    +- group.userData = { chunkId: { q, r } }
    +- scene.add(group)
    +- chunks.set(key, { chunk, group, dispose })
```

## Files Changed

### Rust (crates/shine-game)
- `src/world/chunk.rs` — add border edge extraction method
- `src/world/world.rs` — add `remove_chunk` method to `World`
- `src/wasm/world/world.rs` — add chunk query methods and `remove_chunk` to `WasmWorld`, change `init_chunk` from `i32` to `u32`

### TypeScript (client/web/src)
- `world/types.ts` — add `ChunkId.ORIGIN`
- `world/chunk.ts` — new file, `Chunk` class
- `world/chunk-mesh-builder.ts` — new file, Three.js mesh builder
- `game.ts` — update `loadChunk` to build mesh, position group, store `Chunk`
- `wasm-types/shine_game.d.ts` — updated by wasm-pack (auto-generated)
