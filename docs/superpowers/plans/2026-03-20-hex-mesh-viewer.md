# Hex Mesh Viewer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a three.js client that renders hex quad meshes from the Rust `PatchMesher` via wasm bindings, with interactive controls for subdivision, smoothing, and parameters.

**Architecture:** A wasm-bindgen API layer (`wasm_api.rs`) in the existing `shine-game` crate exposes mesh generation to JavaScript via JSON config. A separate `client/` TypeScript project uses Vite + three.js to render the mesh in 3D with lil-gui controls.

**Tech Stack:** Rust/wasm-bindgen, wasm-pack, TypeScript, Vite, three.js, lil-gui, pnpm

**Spec:** `docs/superpowers/specs/2026-03-20-hex-mesh-viewer-design.md`

---

## File Map

### Rust (shine-game)

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/shine-game/Cargo.toml` | Modify | Add `[lib] crate-type` |
| `crates/shine-game/src/lib.rs` | Modify | Add `wasm_api` module behind `cfg(wasm32)` |
| `crates/shine-game/src/wasm_api.rs` | Create | Wasm-bindgen API: `WasmPatchMesh`, `generate_mesh`, xorshift RNG |

### Client (TypeScript)

| File | Action | Responsibility |
|------|--------|---------------|
| `client/package.json` | Create | Dependencies, scripts |
| `client/tsconfig.json` | Create | TypeScript config |
| `client/vite.config.ts` | Create | Vite + wasm plugins |
| `client/index.html` | Create | HTML entry point |
| `client/src/main.ts` | Create | Init wasm, scene, controls, wire together |
| `client/src/scene.ts` | Create | Three.js scene, camera, lights, renderer, OrbitControls |
| `client/src/mesh-builder.ts` | Create | Wasm output → BufferGeometry + edge LineSegments |
| `client/src/controls.ts` | Create | lil-gui panel: subdivision, orientation, smoothing, advanced |

### Config

| File | Action | Responsibility |
|------|--------|---------------|
| `.gitignore` | Modify | Add `client/pkg/` |

---

## Task 1: Wasm API — Rust Bindings

**Files:**
- Modify: `crates/shine-game/Cargo.toml`
- Modify: `crates/shine-game/src/lib.rs`
- Create: `crates/shine-game/src/wasm_api.rs`

### Context

The `PatchMesher` (in `crates/shine-game/src/math/hex/patch_mesher.rs`) generates quad meshes inside hexagons. It takes a `Box<dyn StableRng>` (trait in `crates/shine-game/src/math/rand.rs` — requires only `fn next_u32(&mut self) -> u32`). The wasm API wraps this with a JSON config interface.

Key types to use:
- `PatchMesher::new(subdivision, orientation, rng)` — constructor
- `mesher.create_vertex_buffer()` → `Vec<Vec2>` — allocates zero-filled buffer
- `mesher.generate_uniform(&mut vertices)` — places vertices at axial positions
- `mesher.smooth_weighted_lloyd(iterations, strength, (weight_min, weight_max), &mut vertices)`
- `mesher.smooth_noise(amplitude, frequency, &mut vertices)`
- `mesher.smooth_cotangent(iterations, strength, &mut vertices)`
- `mesher.smooth_spring(iterations, dt, spring_strength, shape_strength, &mut vertices)`
- `mesher.smooth_jitter(amplitude, &mut vertices)`
- `mesher.fix_quads(min_quality, max_iterations, &vertices)`
- `PatchOrientation::Even` / `PatchOrientation::Odd`
- `PatchDenseIndexer::new(subdivision)` — iterates quads
- `PatchCoord::quad_vertices(orientation, subdivision)` → `[AxialCoord; 4]`
- `AxialDenseIndexer::new(radius)` — maps `AxialCoord` to dense index

Reference `crates/shine-game/src/math/hex/patch_mesh_svg.rs` for the quad iteration pattern.

- [ ] **Step 1: Update Cargo.toml**

Add crate-type for wasm-pack support. In `crates/shine-game/Cargo.toml`:

After the `[package]` section, before `[dependencies]`, add:

```toml
[lib]
crate-type = ["cdylib", "rlib"]
```

Note: `serde_json` is already a regular dependency (line 8), no need to add it again.

- [ ] **Step 2: Create wasm_api.rs**

Create `crates/shine-game/src/wasm_api.rs`:

```rust
use crate::math::{
    hex::{AxialDenseIndexer, PatchCoord, PatchDenseIndexer, PatchMesher, PatchOrientation},
    rand::StableRng,
};
use serde::Deserialize;
use wasm_bindgen::prelude::*;

/// Xorshift32 PRNG implementing StableRng.
struct Xorshift32(u32);

impl Xorshift32 {
    fn new(seed: u32) -> Self {
        // Avoid zero state which would produce all zeros
        Self(if seed == 0 { 1 } else { seed })
    }
}

impl StableRng for Xorshift32 {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }
}

#[derive(Deserialize)]
struct MeshConfig {
    subdivision: u32,
    orientation: String,
    seed: u32,
    #[serde(default)]
    smoothing: SmoothingConfig,
    #[serde(default)]
    fix_quads: FixQuadsConfig,
}

#[derive(Deserialize, Default)]
struct SmoothingConfig {
    #[serde(default = "default_method")]
    method: String,
    #[serde(default = "default_iterations")]
    iterations: u32,
    #[serde(default = "default_strength")]
    strength: f32,
    #[serde(default = "default_weight_min")]
    weight_min: f32,
    #[serde(default = "default_weight_max")]
    weight_max: f32,
    #[serde(default = "default_amplitude")]
    amplitude: f32,
    #[serde(default = "default_frequency")]
    frequency: f32,
    #[serde(default = "default_dt")]
    dt: f32,
    #[serde(default = "default_spring_strength")]
    spring_strength: f32,
    #[serde(default = "default_shape_strength")]
    shape_strength: f32,
}

fn default_method() -> String { "None".to_string() }
fn default_iterations() -> u32 { 20 }
fn default_strength() -> f32 { 0.4 }
fn default_weight_min() -> f32 { 2.5 }
fn default_weight_max() -> f32 { 15.5 }
fn default_amplitude() -> f32 { 0.5 }
fn default_frequency() -> f32 { 5.0 }
fn default_dt() -> f32 { 0.1 }
fn default_spring_strength() -> f32 { 0.3 }
fn default_shape_strength() -> f32 { 0.5 }

#[derive(Deserialize)]
struct FixQuadsConfig {
    #[serde(default = "default_fix_enabled")]
    enabled: bool,
    #[serde(default = "default_min_quality")]
    min_quality: f32,
    #[serde(default = "default_fix_max_iterations")]
    max_iterations: u32,
}

fn default_fix_enabled() -> bool { true }
fn default_min_quality() -> f32 { 0.15 }
fn default_fix_max_iterations() -> u32 { 50 }

impl Default for FixQuadsConfig {
    fn default() -> Self {
        Self {
            enabled: default_fix_enabled(),
            min_quality: default_min_quality(),
            max_iterations: default_fix_max_iterations(),
        }
    }
}

#[wasm_bindgen]
pub struct WasmPatchMesh {
    vertices: Vec<f32>,
    indices: Vec<u32>,
    patch_indices: Vec<u8>,
}

#[wasm_bindgen]
impl WasmPatchMesh {
    /// Flat vertex positions [x, y, x, y, ...] (2 floats per vertex)
    pub fn vertices(&self) -> Vec<f32> {
        self.vertices.clone()
    }

    /// Flat quad indices [a, b, c, d, ...] (4 indices per quad)
    pub fn indices(&self) -> Vec<u32> {
        self.indices.clone()
    }

    /// Patch index per quad (0, 1, or 2)
    pub fn patch_indices(&self) -> Vec<u8> {
        self.patch_indices.clone()
    }

    /// Number of vertices
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / 2
    }

    /// Number of quads
    pub fn quad_count(&self) -> usize {
        self.indices.len() / 4
    }
}

/// Generate a hex quad mesh from a JSON config string.
#[wasm_bindgen]
pub fn generate_mesh(config_json: &str) -> Result<WasmPatchMesh, JsValue> {
    let config: MeshConfig =
        serde_json::from_str(config_json).map_err(|e| JsValue::from_str(&e.to_string()))?;

    if config.subdivision > 5 {
        return Err(JsValue::from_str("subdivision must be 0-5"));
    }

    let orientation = match config.orientation.as_str() {
        "Even" => PatchOrientation::Even,
        "Odd" => PatchOrientation::Odd,
        _ => return Err(JsValue::from_str("orientation must be 'Even' or 'Odd'")),
    };

    let rng = Xorshift32::new(config.seed);
    let mut mesher = PatchMesher::new(config.subdivision, orientation, rng);

    // Step 3-4: Generate base vertices
    let mut vertices = mesher.create_vertex_buffer();
    mesher.generate_uniform(&mut vertices);

    // Step 5: Apply smoothing
    match config.smoothing.method.as_str() {
        "None" => {}
        "Lloyd" => {
            mesher.smooth_weighted_lloyd(
                config.smoothing.iterations,
                config.smoothing.strength,
                (config.smoothing.weight_min, config.smoothing.weight_max),
                &mut vertices,
            );
        }
        "Noise" => {
            mesher.smooth_noise(
                config.smoothing.amplitude,
                config.smoothing.frequency,
                &mut vertices,
            );
        }
        "Cotangent" => {
            mesher.smooth_cotangent(
                config.smoothing.iterations,
                config.smoothing.strength,
                &mut vertices,
            );
        }
        "Spring" => {
            mesher.smooth_spring(
                config.smoothing.iterations,
                config.smoothing.dt,
                config.smoothing.spring_strength,
                config.smoothing.shape_strength,
                &mut vertices,
            );
        }
        "Jitter" => {
            mesher.smooth_jitter(config.smoothing.amplitude, &mut vertices);
        }
        other => {
            return Err(JsValue::from_str(&format!("unknown smoothing method: {other}")));
        }
    }

    // Step 6: Fix quads
    if config.fix_quads.enabled {
        mesher.fix_quads(
            config.fix_quads.min_quality,
            config.fix_quads.max_iterations,
            &mut vertices,
        );
    }

    // Step 7-8: Build index arrays
    let radius = 2u32.pow(config.subdivision);
    let axial_indexer = AxialDenseIndexer::new(radius);
    let patch_indexer = PatchDenseIndexer::new(config.subdivision);
    let grid = 2i32.pow(config.subdivision);

    let mut indices = Vec::with_capacity(patch_indexer.get_total_size() * 4);
    let mut patch_indices_out = Vec::with_capacity(patch_indexer.get_total_size());

    for p in 0..3i32 {
        for u in 0..grid {
            for v in 0..grid {
                let patch = PatchCoord::new(p, u, v);
                let quad = patch.quad_vertices(orientation, config.subdivision);
                for coord in &quad {
                    indices.push(axial_indexer.get_dense_index(coord) as u32);
                }
                patch_indices_out.push(p as u8);
            }
        }
    }

    // Flatten Vec2 positions to [x, y, x, y, ...]
    let flat_vertices: Vec<f32> = vertices.iter().flat_map(|v| [v.x, v.y]).collect();

    Ok(WasmPatchMesh {
        vertices: flat_vertices,
        indices,
        patch_indices: patch_indices_out,
    })
}
```

- [ ] **Step 3: Update lib.rs**

In `crates/shine-game/src/lib.rs`, add the wasm_api module:

```rust
pub mod math;
pub mod world;

#[cfg(target_arch = "wasm32")]
mod wasm_api;
```

- [ ] **Step 4: Verify native build still works**

Run: `cargo check -p shine-game`
Expected: Success (the `wasm_api` module is gated behind `cfg(wasm32)`, so native build ignores it)

- [ ] **Step 5: Verify existing tests still pass**

Run: `cargo test -p shine-game`
Expected: All existing tests pass

- [ ] **Step 6: Build wasm to verify it compiles**

Run: `wasm-pack build crates/shine-game --target web --out-dir ../../client/pkg`
Expected: Success, produces `client/pkg/` with `.wasm`, `.js`, and `.d.ts` files

Note: Requires `wasm-pack` installed. Install with `cargo install wasm-pack` if needed.

- [ ] **Step 7: Commit**

```
feat: add wasm-bindgen API for hex mesh generation
```

---

## Task 2: Client Project Scaffold

**Files:**
- Create: `client/package.json`
- Create: `client/tsconfig.json`
- Create: `client/vite.config.ts`
- Create: `client/index.html`
- Modify: `.gitignore`

- [ ] **Step 1: Update .gitignore**

Add to the end of `.gitignore`:

```
client/pkg/
```

- [ ] **Step 2: Create package.json**

Create `client/package.json`:

```json
{
  "name": "hex-mesh-viewer",
  "private": true,
  "version": "0.0.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "wasm": "wasm-pack build ../crates/shine-game --target web --out-dir ../../client/pkg",
    "wasm:dev": "pnpm wasm && pnpm dev"
  },
  "dependencies": {
    "three": "^0.172",
    "lil-gui": "^0.20"
  },
  "devDependencies": {
    "typescript": "^5.7",
    "vite": "^6.2",
    "vite-plugin-wasm": "^3.4",
    "vite-plugin-top-level-await": "^1.5",
    "@types/three": "^0.172"
  }
}
```

- [ ] **Step 3: Create tsconfig.json**

Create `client/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "moduleResolution": "bundler",
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "forceConsistentCasingInFileNames": true,
    "outDir": "dist",
    "sourceMap": true
  },
  "include": ["src"]
}
```

- [ ] **Step 4: Create vite.config.ts**

Create `client/vite.config.ts`:

```typescript
import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

export default defineConfig({
  plugins: [wasm(), topLevelAwait()],
});
```

- [ ] **Step 5: Create index.html**

Create `client/index.html`:

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Hex Mesh Viewer</title>
    <style>
      body {
        margin: 0;
        overflow: hidden;
        background: #1a1a2e;
      }
      canvas {
        display: block;
      }
    </style>
  </head>
  <body>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

- [ ] **Step 6: Install dependencies**

Run: `cd client && pnpm install`
Expected: `node_modules/` created, `pnpm-lock.yaml` generated

- [ ] **Step 7: Commit**

```
feat: scaffold client project with Vite + three.js + lil-gui
```

---

## Task 3: Three.js Scene Setup

**Files:**
- Create: `client/src/scene.ts`
- Create: `client/src/main.ts` (stub)

### Context

The mesh lies on the XZ ground plane. 2D vertex `(x, y)` from wasm maps to Three.js `(x, 0, y)`. The hex uses a flat-top layout with `hex_size = 1.0`. At subdivision 3, the world extent is roughly radius 8 in axial coords → about 12 units in x, 14 in y.

- [ ] **Step 1: Create scene.ts**

Create `client/src/scene.ts`:

```typescript
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";

export interface SceneContext {
  scene: THREE.Scene;
  camera: THREE.PerspectiveCamera;
  renderer: THREE.WebGLRenderer;
  controls: OrbitControls;
}

export function createScene(): SceneContext {
  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x1a1a2e);

  const camera = new THREE.PerspectiveCamera(
    50,
    window.innerWidth / window.innerHeight,
    0.1,
    1000
  );
  camera.position.set(0, 25, 15);
  camera.lookAt(0, 0, 0);

  const renderer = new THREE.WebGLRenderer({ antialias: true });
  renderer.setSize(window.innerWidth, window.innerHeight);
  renderer.setPixelRatio(window.devicePixelRatio);
  document.body.appendChild(renderer.domElement);

  const controls = new OrbitControls(camera, renderer.domElement);
  controls.target.set(0, 0, 0);
  controls.enableDamping = true;
  controls.dampingFactor = 0.1;
  controls.update();

  // Lighting
  const ambient = new THREE.AmbientLight(0xffffff, 0.6);
  scene.add(ambient);

  const directional = new THREE.DirectionalLight(0xffffff, 0.8);
  directional.position.set(10, 20, 5);
  scene.add(directional);

  // Resize handling
  window.addEventListener("resize", () => {
    camera.aspect = window.innerWidth / window.innerHeight;
    camera.updateProjectionMatrix();
    renderer.setSize(window.innerWidth, window.innerHeight);
  });

  return { scene, camera, renderer, controls };
}

export function animate(ctx: SceneContext) {
  function loop() {
    requestAnimationFrame(loop);
    ctx.controls.update();
    ctx.renderer.render(ctx.scene, ctx.camera);
  }
  loop();
}
```

- [ ] **Step 2: Create main.ts stub**

Create `client/src/main.ts`:

```typescript
import { createScene, animate } from "./scene";

const ctx = createScene();
animate(ctx);
```

- [ ] **Step 3: Verify dev server starts**

Run: `cd client && pnpm dev`
Expected: Vite dev server starts, browser shows dark background. No errors in console.

- [ ] **Step 4: Commit**

```
feat: add three.js scene with camera, lights, and orbit controls
```

---

## Task 4: Mesh Builder

**Files:**
- Create: `client/src/mesh-builder.ts`

### Context

The wasm API returns:
- `vertices()`: flat `Float32Array` — `[x0, y0, x1, y1, ...]` (2D positions)
- `indices()`: flat `Uint32Array` — `[a0, b0, c0, d0, ...]` (4 per quad)
- `patch_indices()`: `Uint8Array` — one value (0, 1, 2) per quad

Map 2D `(x, y)` to Three.js `(x, 0, y)` on XZ plane. Each quad → 2 triangles. Edge lines built explicitly from quad edges.

- [ ] **Step 1: Create mesh-builder.ts**

Create `client/src/mesh-builder.ts`:

```typescript
import * as THREE from "three";

// Pastel colors for 3 patches
const PATCH_COLORS: THREE.Color[] = [
  new THREE.Color(0.75, 0.85, 0.95), // soft blue
  new THREE.Color(0.85, 0.95, 0.75), // soft green
  new THREE.Color(0.95, 0.82, 0.75), // soft orange
];

const EDGE_COLOR = 0x333333;

export interface MeshData {
  vertices: Float32Array;
  indices: Uint32Array;
  patchIndices: Uint8Array;
}

export interface HexMeshGroup {
  group: THREE.Group;
  dispose: () => void;
}

export function buildHexMesh(data: MeshData): HexMeshGroup {
  const group = new THREE.Group();
  const vertCount = data.vertices.length / 2;
  const quadCount = data.indices.length / 4;

  // Build 3D position array: (x, 0, y)
  const positions = new Float32Array(vertCount * 3);
  for (let i = 0; i < vertCount; i++) {
    positions[i * 3] = data.vertices[i * 2];       // x
    positions[i * 3 + 1] = 0;                       // y (up)
    positions[i * 3 + 2] = data.vertices[i * 2 + 1]; // z (from 2D y)
  }

  // Build non-indexed geometry with per-face vertex colors
  // Each quad (a,b,c,d) → triangles (a,b,c) and (a,c,d)
  const triPositions: number[] = [];
  const triColors: number[] = [];

  for (let q = 0; q < quadCount; q++) {
    const a = data.indices[q * 4];
    const b = data.indices[q * 4 + 1];
    const c = data.indices[q * 4 + 2];
    const d = data.indices[q * 4 + 3];

    const color = PATCH_COLORS[data.patchIndices[q]];

    // Triangle 1: a, b, c
    for (const idx of [a, b, c]) {
      triPositions.push(positions[idx * 3], positions[idx * 3 + 1], positions[idx * 3 + 2]);
      triColors.push(color.r, color.g, color.b);
    }

    // Triangle 2: a, c, d
    for (const idx of [a, c, d]) {
      triPositions.push(positions[idx * 3], positions[idx * 3 + 1], positions[idx * 3 + 2]);
      triColors.push(color.r, color.g, color.b);
    }
  }

  // Filled mesh
  const fillGeom = new THREE.BufferGeometry();
  fillGeom.setAttribute("position", new THREE.Float32BufferAttribute(triPositions, 3));
  fillGeom.setAttribute("color", new THREE.Float32BufferAttribute(triColors, 3));
  fillGeom.computeVertexNormals();

  const fillMat = new THREE.MeshStandardMaterial({
    vertexColors: true,
    flatShading: true,
    side: THREE.DoubleSide,
  });
  const fillMesh = new THREE.Mesh(fillGeom, fillMat);
  group.add(fillMesh);

  // Edge lines — explicitly from quad topology
  const edgePositions: number[] = [];
  for (let q = 0; q < quadCount; q++) {
    const qi = q * 4;
    for (let e = 0; e < 4; e++) {
      const i0 = data.indices[qi + e];
      const i1 = data.indices[qi + (e + 1) % 4];
      edgePositions.push(
        positions[i0 * 3], positions[i0 * 3 + 1] + 0.01, positions[i0 * 3 + 2],
        positions[i1 * 3], positions[i1 * 3 + 1] + 0.01, positions[i1 * 3 + 2]
      );
    }
  }

  const edgeGeom = new THREE.BufferGeometry();
  edgeGeom.setAttribute("position", new THREE.Float32BufferAttribute(edgePositions, 3));
  const edgeMat = new THREE.LineBasicMaterial({ color: EDGE_COLOR });
  const edgeLines = new THREE.LineSegments(edgeGeom, edgeMat);
  group.add(edgeLines);

  const dispose = () => {
    fillGeom.dispose();
    fillMat.dispose();
    edgeGeom.dispose();
    edgeMat.dispose();
  };

  return { group, dispose };
}
```

- [ ] **Step 2: Commit**

```
feat: add mesh builder converting wasm output to three.js geometry
```

---

## Task 5: Controls Panel

**Files:**
- Create: `client/src/controls.ts`

### Context

Uses lil-gui. Main controls always visible, advanced folder collapsed and rebuilt when smoothing method changes. Every change triggers a callback to regenerate the mesh.

- [ ] **Step 1: Create controls.ts**

Create `client/src/controls.ts`:

```typescript
import GUI from "lil-gui";

export interface MeshParams {
  subdivision: number;
  orientation: string;
  smoothing: string;
  seed: number;
  // Lloyd
  lloyd_iterations: number;
  lloyd_strength: number;
  lloyd_weight_min: number;
  lloyd_weight_max: number;
  // Noise
  noise_amplitude: number;
  noise_frequency: number;
  // Cotangent
  cotangent_iterations: number;
  cotangent_strength: number;
  // Spring
  spring_iterations: number;
  spring_dt: number;
  spring_spring_strength: number;
  spring_shape_strength: number;
  // Jitter
  jitter_amplitude: number;
  // Fix quads
  fix_enabled: boolean;
  fix_min_quality: number;
  fix_max_iterations: number;
}

export function defaultParams(): MeshParams {
  return {
    subdivision: 3,
    orientation: "Even",
    smoothing: "None",
    seed: 42,
    lloyd_iterations: 20,
    lloyd_strength: 0.4,
    lloyd_weight_min: 2.5,
    lloyd_weight_max: 15.5,
    noise_amplitude: 0.5,
    noise_frequency: 5.0,
    cotangent_iterations: 10,
    cotangent_strength: 0.5,
    spring_iterations: 50,
    spring_dt: 0.1,
    spring_spring_strength: 0.3,
    spring_shape_strength: 0.5,
    jitter_amplitude: 2.0,
    fix_enabled: true,
    fix_min_quality: 0.15,
    fix_max_iterations: 50,
  };
}

export function paramsToConfigJson(p: MeshParams): string {
  const smoothing: Record<string, unknown> = { method: p.smoothing };

  switch (p.smoothing) {
    case "Lloyd":
      smoothing.iterations = p.lloyd_iterations;
      smoothing.strength = p.lloyd_strength;
      smoothing.weight_min = p.lloyd_weight_min;
      smoothing.weight_max = p.lloyd_weight_max;
      break;
    case "Noise":
      smoothing.amplitude = p.noise_amplitude;
      smoothing.frequency = p.noise_frequency;
      break;
    case "Cotangent":
      smoothing.iterations = p.cotangent_iterations;
      smoothing.strength = p.cotangent_strength;
      break;
    case "Spring":
      smoothing.iterations = p.spring_iterations;
      smoothing.dt = p.spring_dt;
      smoothing.spring_strength = p.spring_spring_strength;
      smoothing.shape_strength = p.spring_shape_strength;
      break;
    case "Jitter":
      smoothing.amplitude = p.jitter_amplitude;
      break;
  }

  return JSON.stringify({
    subdivision: p.subdivision,
    orientation: p.orientation,
    seed: p.seed,
    smoothing,
    fix_quads: {
      enabled: p.fix_enabled,
      min_quality: p.fix_min_quality,
      max_iterations: p.fix_max_iterations,
    },
  });
}

export function createControls(
  params: MeshParams,
  onChange: () => void
): GUI {
  const gui = new GUI({ title: "Hex Mesh" });

  gui.add(params, "subdivision", 0, 5, 1).onChange(onChange);
  gui.add(params, "orientation", ["Even", "Odd"]).onChange(onChange);
  gui.add(params, "smoothing", ["None", "Lloyd", "Noise", "Cotangent", "Spring", "Jitter"])
    .onChange(() => {
      rebuildAdvanced();
      onChange();
    });
  gui.add(params, "seed", 0, 999999, 1).onChange(onChange);

  let advancedFolder: GUI | null = null;

  function rebuildAdvanced() {
    if (advancedFolder) {
      advancedFolder.destroy();
      advancedFolder = null;
    }

    if (params.smoothing === "None") return;

    advancedFolder = gui.addFolder("Advanced");
    advancedFolder.close();

    switch (params.smoothing) {
      case "Lloyd":
        advancedFolder.add(params, "lloyd_iterations", 1, 50, 1).name("iterations").onChange(onChange);
        advancedFolder.add(params, "lloyd_strength", 0, 1, 0.01).name("strength").onChange(onChange);
        advancedFolder.add(params, "lloyd_weight_min", 0.5, 20, 0.1).name("weight min").onChange(onChange);
        advancedFolder.add(params, "lloyd_weight_max", 1, 30, 0.1).name("weight max").onChange(onChange);
        break;
      case "Noise":
        advancedFolder.add(params, "noise_amplitude", 0, 2, 0.01).name("amplitude").onChange(onChange);
        advancedFolder.add(params, "noise_frequency", 0.5, 20, 0.1).name("frequency").onChange(onChange);
        break;
      case "Cotangent":
        advancedFolder.add(params, "cotangent_iterations", 1, 50, 1).name("iterations").onChange(onChange);
        advancedFolder.add(params, "cotangent_strength", 0, 1, 0.01).name("strength").onChange(onChange);
        break;
      case "Spring":
        advancedFolder.add(params, "spring_iterations", 1, 200, 1).name("iterations").onChange(onChange);
        advancedFolder.add(params, "spring_dt", 0.01, 0.5, 0.01).name("dt").onChange(onChange);
        advancedFolder.add(params, "spring_spring_strength", 0, 2, 0.01).name("spring strength").onChange(onChange);
        advancedFolder.add(params, "spring_shape_strength", 0, 2, 0.01).name("shape strength").onChange(onChange);
        break;
      case "Jitter":
        advancedFolder.add(params, "jitter_amplitude", 0, 5, 0.01).name("amplitude").onChange(onChange);
        break;
    }

  }

  // Fix quads folder — always visible (useful even without smoothing)
  const fixFolder = gui.addFolder("Fix Quads");
  fixFolder.add(params, "fix_enabled").name("enabled").onChange(onChange);
  fixFolder.add(params, "fix_min_quality", 0.01, 0.5, 0.01).name("min quality").onChange(onChange);
  fixFolder.add(params, "fix_max_iterations", 1, 200, 1).name("max iterations").onChange(onChange);
  fixFolder.close();

  // Build initial advanced section
  rebuildAdvanced();

  return gui;
}
```

- [ ] **Step 2: Commit**

```
feat: add lil-gui controls for mesh parameters
```

---

## Task 6: Wire Everything Together

**Files:**
- Modify: `client/src/main.ts`

### Context

This task connects all pieces: load wasm, create scene, create controls, generate mesh on parameter change.

The wasm package is at `../pkg/shine_game.js` (wasm-pack generates the module as `shine_game` from the crate name `shine-game`). It exports `generate_mesh(config_json: string)` which returns a `WasmPatchMesh` with `.vertices()`, `.indices()`, `.patch_indices()` methods. The module also exports a default `init()` function that must be called first.

- [ ] **Step 1: Update main.ts**

Replace `client/src/main.ts` with:

```typescript
import init, { generate_mesh } from "../pkg/shine_game.js";
import { createScene, animate } from "./scene";
import { buildHexMesh, HexMeshGroup } from "./mesh-builder";
import { createControls, defaultParams, paramsToConfigJson } from "./controls";

async function main() {
  await init();

  const ctx = createScene();
  const params = defaultParams();
  let currentMesh: HexMeshGroup | null = null;

  function regenerate() {
    // Remove old mesh
    if (currentMesh) {
      ctx.scene.remove(currentMesh.group);
      currentMesh.dispose();
      currentMesh = null;
    }

    try {
      const configJson = paramsToConfigJson(params);
      const wasmMesh = generate_mesh(configJson);

      const data = {
        vertices: wasmMesh.vertices(),
        indices: wasmMesh.indices(),
        patchIndices: wasmMesh.patch_indices(),
      };

      console.log(
        `Generated: ${wasmMesh.vertex_count()} vertices, ${wasmMesh.quad_count()} quads`
      );

      // Free wasm-side memory after extracting data
      wasmMesh.free();

      currentMesh = buildHexMesh(data);
      ctx.scene.add(currentMesh.group);
    } catch (e) {
      console.error("Mesh generation failed:", e);
    }
  }

  createControls(params, regenerate);
  regenerate();
  animate(ctx);
}

main();
```

- [ ] **Step 2: Build wasm and run**

First, ensure wasm is built:
Run: `cd client && pnpm wasm`
Expected: `client/pkg/` populated with `shine_game.js`, `shine_game.d.ts`, `shine_game_bg.wasm`

Then start dev server:
Run: `cd client && pnpm dev`
Expected: Browser shows hex mesh rendered in 3D with controls panel. Changing subdivision regenerates the mesh. Orbit controls work.

- [ ] **Step 3: Test all smoothing methods**

Manually verify in the browser:
1. Set smoothing to each of: Lloyd, Noise, Cotangent, Spring, Jitter
2. Verify mesh renders without console errors
3. Expand Advanced, tweak parameters, confirm mesh updates
4. Toggle Fix Quads on/off
5. Change subdivision from 0 to 5, verify each renders
6. Switch orientation between Even and Odd

- [ ] **Step 4: Commit**

```
feat: wire wasm mesh generation to three.js viewer with controls
```

---

## Task 7: Final Polish and Cleanup

**Files:**
- Review all created files

- [ ] **Step 1: Verify clean build from scratch**

Run:
```bash
cd client && rm -rf node_modules pkg && pnpm install && pnpm wasm && pnpm build
```
Expected: Clean install, wasm build, and Vite production build all succeed.

- [ ] **Step 2: Verify native Rust workspace unaffected**

Run: `cargo check && cargo test -p shine-game`
Expected: All checks and tests pass. The `cdylib` crate-type and `wasm_api` module (gated behind `cfg(wasm32)`) do not affect native builds.

- [ ] **Step 3: Commit**

```
chore: verify clean builds for wasm and native targets
```
