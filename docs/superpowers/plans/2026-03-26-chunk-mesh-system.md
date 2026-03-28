# Chunk Mesh System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Render chunk meshes in the Three.js game client by exposing chunk vertex/quad/border data and world-offset calculations through WasmWorld methods.

**Architecture:** All chunk data lives in the Rust `World` struct. WASM exposes query methods on `WasmWorld` taking `(q, r)` coordinates. TypeScript wraps these in a `Chunk` convenience class and builds Three.js geometry from the data. Each chunk gets a `THREE.Group` positioned by its world offset relative to a reference chunk.

**Tech Stack:** Rust (wasm-bindgen, glam), TypeScript, Three.js, wasm-pack

**Spec:** `docs/superpowers/specs/2026-03-26-chunk-mesh-system-design.md`

---

## File Structure

### Rust (crates/shine-game/src)
| File | Action | Responsibility |
|------|--------|---------------|
| `world/chunk.rs` | Modify | Add `border_edges()` method, add `DEFAULT_ORIGIN` constant |
| `world/world.rs` | Modify | Add `remove_chunk()`, add data accessor methods (`chunk_vertices`, `chunk_quad_indices`, `chunk_border_indices`, `chunk_world_offset`) |
| `wasm/world/world.rs` | Modify | Change `init_chunk` from `i32` to `u32`, add all new WASM-exposed methods |

### TypeScript (client/web/src)
| File | Action | Responsibility |
|------|--------|---------------|
| `world/types.ts` | Modify | Add `ChunkId.ORIGIN` static constant |
| `world/chunk.ts` | Create | `Chunk` class wrapping `WasmWorld` + `ChunkId` |
| `world/chunk-mesh-builder.ts` | Create | Build Three.js group from chunk data (filled quads + border outline) |
| `game.ts` | Modify | Wire up chunk loading with mesh building, positioning, and userData |

---

## Task 1: Add `border_edges()` to `QuadTopology`

**Files:**
- Modify: `crates/shine-game/src/math/mesh/quad_topology.rs`

This method identifies boundary edges by checking if the neighbor quad is a ghost quad (after ghost quad construction, real edges always have a neighbor — boundary edges have ghost quad neighbors).

- [ ] **Step 1: Write the test**

Add to the existing `mod tests` block in `quad_topology.rs`:

```rust
#[test]
fn test_border_edges() {
    let topo = grid_2x2_topo();
    let border = topo.border_edges();
    // 2x2 grid has 8 boundary edges (4 sides of perimeter)
    // Each edge is a pair of vertex indices [a, b]
    assert_eq!(border.len(), 8);

    // All border edge vertices should be boundary vertices
    for &[a, b] in &border {
        assert!(
            topo.is_boundary_vertex(VertIdx::new(a as usize)),
            "border edge vertex {a} should be boundary"
        );
        assert!(
            topo.is_boundary_vertex(VertIdx::new(b as usize)),
            "border edge vertex {b} should be boundary"
        );
    }

    // Check that the expected perimeter edges are present (unordered)
    // Bottom: 0-1, 1-2  Right: 2-5, 5-8  Top: 8-7, 7-6  Left: 6-3, 3-0
    let mut edge_set: std::collections::HashSet<(u32, u32)> = border
        .iter()
        .map(|&[a, b]| if a < b { (a, b) } else { (b, a) })
        .collect();
    for (a, b) in [(0,1), (1,2), (2,5), (5,8), (7,8), (6,7), (3,6), (0,3)] {
        assert!(
            edge_set.remove(&(a, b)),
            "expected border edge ({a}, {b}) not found"
        );
    }
    assert!(edge_set.is_empty(), "unexpected extra border edges: {:?}", edge_set);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p shine-game test_border_edges`
Expected: FAIL — `border_edges` method does not exist

- [ ] **Step 3: Implement `border_edges()`**

Add to `impl QuadTopology` in `quad_topology.rs`, after `is_boundary_edge`:

```rust
/// Returns boundary edges as pairs of real vertex indices `[a, b]`.
/// A boundary edge is a real quad edge whose neighbor is a ghost quad.
pub fn border_edges(&self) -> Vec<[u32; 2]> {
    let mut edges = Vec::new();
    for qi in self.real_quad_indices() {
        let verts = self.quads[qi];
        for k in 0..4 {
            let neighbor = self.quad_neighbors[qi][k];
            if self.is_ghost_quad(neighbor) {
                let a = verts[k].into_index() as u32;
                let b = verts[(k + 1) % 4].into_index() as u32;
                edges.push([a, b]);
            }
        }
    }
    edges
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p shine-game test_border_edges`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/shine-game/src/math/mesh/quad_topology.rs
git commit -m "feat: add border_edges() to QuadTopology

Identifies boundary edges by checking for ghost quad neighbors.
Returns flat [a, b] vertex index pairs for line rendering."
```

---

## Task 2: Add chunk data accessors and `remove_chunk` to `World`

**Files:**
- Modify: `crates/shine-game/src/world/chunk.rs` — add `DEFAULT_ORIGIN` constant
- Modify: `crates/shine-game/src/world/world.rs` — add accessor methods

- [ ] **Step 1: Add `DEFAULT_ORIGIN` to `ChunkId`**

In `chunk.rs`, add after the `ChunkId` struct definition:

```rust
impl ChunkId {
    pub const DEFAULT_ORIGIN: ChunkId = ChunkId(1024, 1024);

    // ... existing methods ...
}
```

- [ ] **Step 2: Add `remove_chunk` and data accessors to `World`**

In `world.rs`, add these methods to `impl World`:

```rust
pub fn remove_chunk(&mut self, id: ChunkId) {
    self.chunks.remove(&id);
}

/// Flat vertex positions [x, y, x, y, ...]. Empty if chunk not found.
pub fn chunk_vertices(&self, id: ChunkId) -> Vec<f32> {
    let Some(chunk) = self.chunks.get(&id) else {
        return Vec::new();
    };
    // vertices IdxVec contains only real vertices (from LatticeMesher, before ghost quads)
    debug_assert_eq!(chunk.vertices.len(), chunk.topology.real_vertex_count());
    let mut flat = Vec::with_capacity(chunk.topology.real_vertex_count() * 2);
    for vi in chunk.topology.vertex_indices() {
        let p = chunk.vertices[vi];
        flat.push(p.x);
        flat.push(p.y);
    }
    flat
}

/// Flat quad indices [a, b, c, d, ...]. Empty if chunk not found.
/// Safety: real quads only reference real vertices (ghost vertices only appear in ghost quads).
pub fn chunk_quad_indices(&self, id: ChunkId) -> Vec<u32> {
    let Some(chunk) = self.chunks.get(&id) else {
        return Vec::new();
    };
    let mut indices = Vec::with_capacity(chunk.topology.real_quad_count() * 4);
    for qi in chunk.topology.real_quad_indices() {
        let verts = chunk.topology.quad_vertices(qi);
        for &v in &verts {
            indices.push(v.into_index() as u32);
        }
    }
    indices
}

/// Flat border edge indices [a, b, ...]. Empty if chunk not found.
pub fn chunk_border_indices(&self, id: ChunkId) -> Vec<u32> {
    let Some(chunk) = self.chunks.get(&id) else {
        return Vec::new();
    };
    let edges = chunk.topology.border_edges();
    let mut flat = Vec::with_capacity(edges.len() * 2);
    for [a, b] in edges {
        flat.push(a);
        flat.push(b);
    }
    flat
}

/// World offset [x, y] of `chunk` relative to `reference`. Empty if target chunk not found.
/// Note: only the target chunk needs to be initialized — the offset is computed from
/// ChunkId math alone, but we check existence to match the "empty for missing" contract.
/// The reference chunk does NOT need to be initialized.
pub fn chunk_world_offset(&self, reference: ChunkId, chunk: ChunkId) -> Vec<f32> {
    if !self.chunks.contains_key(&chunk) {
        return Vec::new();
    }
    let rel = reference.relative_axial_coord(chunk);
    let pos = rel.world_coordinate(CHUNK_WORLD_SIZE);
    vec![pos.x, pos.y]
}
```

- [ ] **Step 3: Write tests for the new `World` methods**

Add a `#[cfg(test)]` module at the bottom of `world.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use shine_test::test;

    #[test]
    fn test_init_and_query_chunk() {
        let mut world = World::new();
        let id = ChunkId::DEFAULT_ORIGIN;
        world.init_chunk(id);

        let verts = world.chunk_vertices(id);
        assert!(!verts.is_empty(), "vertices should not be empty after init");
        assert_eq!(verts.len() % 2, 0, "vertices should have even length (x,y pairs)");

        let indices = world.chunk_quad_indices(id);
        assert!(!indices.is_empty(), "quad indices should not be empty");
        assert_eq!(indices.len() % 4, 0, "quad indices should be multiple of 4");

        let border = world.chunk_border_indices(id);
        assert!(!border.is_empty(), "border indices should not be empty");
        assert_eq!(border.len() % 2, 0, "border indices should be pairs");
    }

    #[test]
    fn test_uninitialized_chunk_returns_empty() {
        let world = World::new();
        let id = ChunkId(999, 999);

        assert!(world.chunk_vertices(id).is_empty());
        assert!(world.chunk_quad_indices(id).is_empty());
        assert!(world.chunk_border_indices(id).is_empty());
        assert!(world.chunk_world_offset(ChunkId::DEFAULT_ORIGIN, id).is_empty());
    }

    #[test]
    fn test_remove_chunk() {
        let mut world = World::new();
        let id = ChunkId::DEFAULT_ORIGIN;
        world.init_chunk(id);
        assert!(!world.chunk_vertices(id).is_empty());

        world.remove_chunk(id);
        assert!(world.chunk_vertices(id).is_empty());
    }

    #[test]
    fn test_chunk_world_offset_origin() {
        let mut world = World::new();
        let origin = ChunkId::DEFAULT_ORIGIN;
        world.init_chunk(origin);

        let offset = world.chunk_world_offset(origin, origin);
        assert_eq!(offset.len(), 2);
        assert!((offset[0]).abs() < f32::EPSILON, "same chunk offset x should be 0");
        assert!((offset[1]).abs() < f32::EPSILON, "same chunk offset y should be 0");
    }

    #[test]
    fn test_chunk_world_offset_neighbor() {
        let mut world = World::new();
        let origin = ChunkId::DEFAULT_ORIGIN;
        let neighbor = ChunkId(1025, 1024); // q+1, same r
        world.init_chunk(origin);
        world.init_chunk(neighbor);

        let offset = world.chunk_world_offset(origin, neighbor);
        assert_eq!(offset.len(), 2);
        // q+1 neighbor: x should be positive (1.5 * CHUNK_WORLD_SIZE), y should be non-zero
        assert!(offset[0] > 0.0, "q+1 neighbor should have positive x offset");
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p shine-game test_init_and_query_chunk test_uninitialized_chunk_returns_empty test_remove_chunk test_chunk_world_offset`
Expected: ALL PASS

- [ ] **Step 5: Commit**

```bash
git add crates/shine-game/src/world/
git commit -m "feat: add chunk data accessors and remove_chunk to World

Exposes chunk_vertices, chunk_quad_indices, chunk_border_indices,
chunk_world_offset methods. Returns empty arrays for uninitialized chunks.
Adds ChunkId::DEFAULT_ORIGIN (1024, 1024)."
```

---

## Task 3: Expose chunk methods via WasmWorld

**Files:**
- Modify: `crates/shine-game/src/wasm/world/world.rs`

- [ ] **Step 1: Update `init_chunk` signature and add all WASM methods**

In `wasm/world/world.rs`: change `init_chunk` parameter types from `i32` to `u32`, remove the `as usize` cast (now `q as usize`), and add the new methods to the existing `impl WasmWorld` block. The full resulting file:

```rust
use crate::world::{ChunkId, World};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct WasmWorld {
    world: World,
}

#[wasm_bindgen]
impl WasmWorld {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { world: World::new() }
    }

    pub fn init_chunk(&mut self, q: u32, r: u32) {
        self.world.init_chunk(ChunkId(q as usize, r as usize));
    }

    pub fn remove_chunk(&mut self, q: u32, r: u32) {
        self.world.remove_chunk(ChunkId(q as usize, r as usize));
    }

    pub fn chunk_vertices(&self, q: u32, r: u32) -> Vec<f32> {
        self.world.chunk_vertices(ChunkId(q as usize, r as usize))
    }

    pub fn chunk_quad_indices(&self, q: u32, r: u32) -> Vec<u32> {
        self.world.chunk_quad_indices(ChunkId(q as usize, r as usize))
    }

    pub fn chunk_border_indices(&self, q: u32, r: u32) -> Vec<u32> {
        self.world.chunk_border_indices(ChunkId(q as usize, r as usize))
    }

    pub fn chunk_world_offset(&self, ref_q: u32, ref_r: u32, q: u32, r: u32) -> Vec<f32> {
        self.world.chunk_world_offset(
            ChunkId(ref_q as usize, ref_r as usize),
            ChunkId(q as usize, r as usize),
        )
    }
}
```

- [ ] **Step 2: Build WASM to verify compilation**

Run: `cargo build -p shine-game --target wasm32-unknown-unknown`
Expected: BUILD SUCCESS (no compile errors)

- [ ] **Step 3: Commit**

```bash
git add crates/shine-game/src/wasm/world/world.rs
git commit -m "feat: expose chunk query methods via WasmWorld

Changes init_chunk from i32 to u32. Adds remove_chunk, chunk_vertices,
chunk_quad_indices, chunk_border_indices, chunk_world_offset."
```

---

## Task 4: Update TypeScript types and create `Chunk` class

**Files:**
- Modify: `client/web/src/world/types.ts`
- Create: `client/web/src/world/chunk.ts`

- [ ] **Step 1: Add `ChunkId.ORIGIN` to `types.ts`**

In `client/web/src/world/types.ts`, add the static constant to the `ChunkId` class:

```ts
export class ChunkId {
    static readonly ORIGIN = new ChunkId(1024, 1024);

    constructor(
        readonly q: number,
        readonly r: number
    ) {}
    key(): string {
        return `${this.q},${this.r}`;
    }
}
```

- [ ] **Step 2: Create `Chunk` class**

Create `client/web/src/world/chunk.ts`:

```ts
import { WasmWorld } from '#wasm';
import { ChunkId } from './types';

export class Chunk {
    constructor(
        private readonly world: WasmWorld,
        readonly id: ChunkId
    ) {}

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

- [ ] **Step 3: Commit**

```bash
git add client/web/src/world/types.ts client/web/src/world/chunk.ts
git commit -m "feat: add ChunkId.ORIGIN and Chunk wrapper class

ChunkId.ORIGIN defaults to (1024, 1024).
Chunk class wraps WasmWorld methods with ChunkId for convenience."
```

---

## Task 5: Create chunk mesh builder

**Files:**
- Create: `client/web/src/world/chunk-mesh-builder.ts`

Reference: `client/web/src/experiments/hex-mesh/mesh-builder.ts` for the pattern of converting 2D vertex data to Three.js geometry.

- [ ] **Step 1: Create the mesh builder**

Create `client/web/src/world/chunk-mesh-builder.ts`:

```ts
import * as THREE from 'three';
import { Chunk } from './chunk';

const FILL_COLOR = new THREE.Color(0.82, 0.85, 0.88);
const BORDER_COLOR = 0x333333;

export interface ChunkMesh {
    group: THREE.Group;
    dispose(): void;
}

export function buildChunkMesh(chunk: Chunk): ChunkMesh {
    const group = new THREE.Group();

    const vertices2D = chunk.vertices();
    const quadIndices = chunk.quadIndices();
    const borderIndices = chunk.borderIndices();

    const vertCount = vertices2D.length / 2;
    const quadCount = quadIndices.length / 4;

    // Build 3D positions: (x, 0, y) from 2D (x, y)
    const positions = new Float32Array(vertCount * 3);
    for (let i = 0; i < vertCount; i++) {
        positions[i * 3] = vertices2D[i * 2];         // x
        positions[i * 3 + 1] = 0;                      // y (up)
        positions[i * 3 + 2] = vertices2D[i * 2 + 1]; // z (from 2D y)
    }

    // Build non-indexed triangle geometry from quads
    // Each quad (a,b,c,d) -> triangles (a,b,c) and (a,c,d)
    const triPositions: number[] = [];
    const triColors: number[] = [];

    for (let q = 0; q < quadCount; q++) {
        const a = quadIndices[q * 4];
        const b = quadIndices[q * 4 + 1];
        const c = quadIndices[q * 4 + 2];
        const d = quadIndices[q * 4 + 3];

        for (const idx of [a, b, c, a, c, d]) {
            triPositions.push(positions[idx * 3], positions[idx * 3 + 1], positions[idx * 3 + 2]);
            triColors.push(FILL_COLOR.r, FILL_COLOR.g, FILL_COLOR.b);
        }
    }

    const fillGeom = new THREE.BufferGeometry();
    fillGeom.setAttribute('position', new THREE.Float32BufferAttribute(triPositions, 3));
    fillGeom.setAttribute('color', new THREE.Float32BufferAttribute(triColors, 3));
    fillGeom.computeVertexNormals();

    const fillMat = new THREE.MeshStandardMaterial({
        vertexColors: true,
        flatShading: true,
        side: THREE.DoubleSide
    });
    const fillMesh = new THREE.Mesh(fillGeom, fillMat);
    group.add(fillMesh);

    // Build border outline from border edge indices
    const borderEdgeCount = borderIndices.length / 2;
    const borderPositions: number[] = [];
    for (let e = 0; e < borderEdgeCount; e++) {
        const i0 = borderIndices[e * 2];
        const i1 = borderIndices[e * 2 + 1];
        borderPositions.push(
            positions[i0 * 3], positions[i0 * 3 + 1] + 0.01, positions[i0 * 3 + 2],
            positions[i1 * 3], positions[i1 * 3 + 1] + 0.01, positions[i1 * 3 + 2]
        );
    }

    const borderGeom = new THREE.BufferGeometry();
    borderGeom.setAttribute('position', new THREE.Float32BufferAttribute(borderPositions, 3));
    const borderMat = new THREE.LineBasicMaterial({ color: BORDER_COLOR });
    const borderLines = new THREE.LineSegments(borderGeom, borderMat);
    group.add(borderLines);

    return {
        group,
        dispose() {
            fillGeom.dispose();
            fillMat.dispose();
            borderGeom.dispose();
            borderMat.dispose();
        }
    };
}
```

- [ ] **Step 2: Commit**

```bash
git add client/web/src/world/chunk-mesh-builder.ts
git commit -m "feat: add chunk mesh builder for Three.js rendering

Builds filled quad mesh + border outline from Chunk data.
Maps 2D vertices to XZ plane (y=0)."
```

---

## Task 6: Wire up `Game` class

**Files:**
- Modify: `client/web/src/game.ts`

- [ ] **Step 1: Update `game.ts` to use new chunk system**

Modify `game.ts`: add imports for `Chunk`, `ChunkMesh`, `buildChunkMesh`; update `ChunkObject` interface to include `chunk` field; add `referenceChunk` field; update `loadChunk` to build mesh and position group; update `unloadChunk` to call `remove_chunk`; update `init()` to use `ChunkId.ORIGIN`. The full resulting file:

```ts
import init, { WasmWorld } from '#wasm';
import wasmUrl from '#wasm-bin';
import * as THREE from 'three';
import { SceneContext, animate, createScene } from './scene';
import { ChunkId } from './world/types';
import { Chunk } from './world/chunk';
import { ChunkMesh, buildChunkMesh } from './world/chunk-mesh-builder';

interface ChunkObject {
    chunk: Chunk;
    group: THREE.Group;
    dispose(): void;
}

class Game {
    private readonly world: WasmWorld;
    private readonly ctx: SceneContext;
    private readonly referenceChunk = ChunkId.ORIGIN;
    private readonly chunks = new Map<string, ChunkObject>();
    private animationId = 0;

    constructor(container: HTMLElement) {
        this.ctx = createScene(container);
        this.world = new WasmWorld();
    }

    init(): void {
        this.loadChunk(ChunkId.ORIGIN);
        this.animationId = animate(this.ctx);
    }

    loadChunk(id: ChunkId): void {
        const key = id.key();
        if (this.chunks.has(key)) return;

        this.world.init_chunk(id.q, id.r);

        const chunk = new Chunk(this.world, id);
        const chunkMesh = buildChunkMesh(chunk);

        const offset = chunk.worldOffset(this.referenceChunk);
        chunkMesh.group.position.set(offset[0], 0, offset[1]);
        chunkMesh.group.userData = { chunkId: { q: id.q, r: id.r } };

        this.ctx.scene.add(chunkMesh.group);
        this.chunks.set(key, {
            chunk,
            group: chunkMesh.group,
            dispose: () => {
                this.ctx.scene.remove(chunkMesh.group);
                chunkMesh.dispose();
            }
        });
    }

    unloadChunk(id: ChunkId): void {
        const key = id.key();
        const obj = this.chunks.get(key);
        if (!obj) return;
        obj.dispose();
        this.chunks.delete(key);
        this.world.remove_chunk(id.q, id.r);
    }

    destroy(): void {
        cancelAnimationFrame(this.animationId);
        for (const obj of this.chunks.values()) {
            obj.dispose();
        }
        this.chunks.clear();
        this.world.free();
        this.ctx.resizeObserver.disconnect();
        this.ctx.renderer.dispose();
        this.ctx.renderer.domElement.remove();
    }
}

export async function createGame(container: HTMLElement): Promise<Game> {
    await init(wasmUrl);
    const game = new Game(container);
    game.init();
    return game;
}
```

- [ ] **Step 2: Commit**

```bash
git add client/web/src/game.ts
git commit -m "feat: wire up chunk mesh rendering in Game

loadChunk now builds Three.js geometry from WASM data,
positions chunks via worldOffset, stores Chunk reference.
unloadChunk frees both JS and WASM resources."
```

---

## Task 7: Build and verify end-to-end

**Files:** None (verification only)

- [ ] **Step 1: Build WASM package**

Run: `wasm-pack build crates/shine-game --target web --out-dir ../../client/web/wasm-pkg`
Expected: BUILD SUCCESS

Note: The exact wasm-pack command may differ — check existing build scripts or `package.json` for the correct invocation. If this fails, check `client/web/vite.config.ts` for the wasm-pack plugin configuration.

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd client/web && npx tsc --noEmit`
Expected: No type errors

If there are type mismatches between the new WASM methods and the `Chunk` class (e.g., `Float32Array` vs `number[]`), update the `Chunk` class return types to match the actual wasm-bindgen output in `wasm-types/shine_game.d.ts`.

- [ ] **Step 3: Run dev server and visually verify**

Run the dev server (check `package.json` scripts or use `npx vite`). Open the game page in a browser. You should see:
- A single chunk of filled quads (light grey) at the origin
- A border outline (dark lines) around the chunk perimeter
- The mesh should be centered at the origin since the loaded chunk IS the reference chunk (offset = 0,0)

- [ ] **Step 4: Commit any type fixes**

If step 2 required adjustments, commit them:

```bash
git add client/web/src/
git commit -m "fix: align TypeScript types with wasm-bindgen output"
```

---

## Task 8: Verify multi-chunk rendering

**Files:**
- Modify: `client/web/src/game.ts` (temporary test, then revert or keep)

- [ ] **Step 1: Load adjacent chunks to verify offset positioning**

Temporarily modify `Game.init()` to load multiple chunks:

```ts
init(): void {
    this.loadChunk(ChunkId.ORIGIN);
    // Load all 6 hex neighbors for visual verification
    this.loadChunk(new ChunkId(1024, 1023)); // r-1 (North)
    this.loadChunk(new ChunkId(1025, 1023)); // q+1, r-1 (NorthEast)
    this.loadChunk(new ChunkId(1025, 1024)); // q+1 (SouthEast)
    this.loadChunk(new ChunkId(1024, 1025)); // r+1 (South)
    this.loadChunk(new ChunkId(1023, 1025)); // q-1, r+1 (SouthWest)
    this.loadChunk(new ChunkId(1023, 1024)); // q-1 (NorthWest)
    this.animationId = animate(this.ctx);
}
```

- [ ] **Step 2: Visually verify in browser**

Open the game page. You should see:
- 7 chunks arranged in a hex flower pattern (center + 6 neighbors)
- Each chunk is offset correctly (not overlapping, not too far apart)
- Border outlines visible on each chunk
- The camera may need zooming out to see all chunks (use scroll wheel)

- [ ] **Step 3: Revert to single chunk or keep multi-chunk**

Decide whether to keep the multi-chunk loading or revert to single origin chunk. Commit the final state:

```bash
git add client/web/src/game.ts
git commit -m "feat: verify multi-chunk rendering with neighbor offsets"
```
