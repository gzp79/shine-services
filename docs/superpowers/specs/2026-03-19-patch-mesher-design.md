# Patch Mesher Design

Port the hex-mesh-gen JS POC (3-quad init + subdivision + Lloyd smoothing) to a Rust module in `crates/shine-game/src/math/hex/`.

## Context

The `experiments/hex-mesh-gen/` POC generates quad meshes inside hexagons by:
1. Splitting a hex into 3 quad patches via a jittered center point
2. Recursively subdividing each quad into 4 children with decreasing jitter
3. Applying Lloyd relaxation to equalize quad areas

The Rust port targets the same pipeline, using existing `AxialCoord`, `PatchCoord`, and dense indexer types for vertex addressing and quad topology.

**Deviation from POC:** The JS POC's default pipeline uses `repulse` + `springSmooth`. Lloyd was selected for the Rust port for simplicity and good area uniformity. Additional smoothing strategies can be added later if needed.

**Deviation from POC:** The JS POC randomly selects a starting vertex (`rng.nextInt(6)`) to determine patch orientation. In the Rust port, orientation is an explicit config parameter (`PatchOrientation`). The RNG stream therefore differs from the POC — this is intentional.

## Radius and Subdivision

The vertex grid radius is always `radius = 2^subdivision`. This constraint arises because:
- The initial 3-quad split places vertices at distance `radius` from center.
- Each subdivision halves edge lengths.
- After `n` subdivisions the finest grid spacing is `radius / 2^n`.
- For integer axial coordinates, this spacing must equal 1, hence `radius = 2^n`.

This means the mesher only works for these specific radius/subdivision pairs. When `subdivision = 0`, `radius = 1`, yielding 7 vertices and 3 quads with no subdivision applied.

## Types

### PatchOrientation

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOrientation {
    Even, // patches span hex vertices 0-1-2, 2-3-4, 4-5-0
    Odd,  // patches span hex vertices 1-2-3, 3-4-5, 5-0-1
}
```

Two base orientations for the 3-patch split. Rotating by 2 vertices gives the same 3 patches, so only 2 distinct orientations exist.

### PatchMeshConfig

```rust
pub struct PatchMeshConfig<'a> {
    pub subdivision: u32,
    pub orientation: PatchOrientation,
    pub jitter_strength: f32,    // base jitter as fraction of radius, default 0.15
    pub lloyd_iterations: u32,   // number of Lloyd passes, default 20
    pub lloyd_strength: f32,     // Lloyd blending factor 0..1, default 0.4
    pub rng: &'a mut dyn Rng,
}
```

- When `jitter_strength <= 0.0`, jitter is disabled entirely and `rng` is never called.
- Constructor `PatchMeshConfig::new(subdivision, orientation, rng)` with defaults for numeric fields.

### PatchMesher

```rust
pub struct PatchMesher<'a> {
    config: PatchMeshConfig<'a>,
}

impl<'a> PatchMesher<'a> {
    pub fn new(config: PatchMeshConfig<'a>) -> Self;

    /// Place 7 initial vertices: 6 hex corners + 1 jittered center.
    pub fn initial_split(&mut self, vertices: &mut [Vec2]);

    /// Run all subdivision levels (internal loop 0..subdivision).
    pub fn subdivide(&mut self, vertices: &mut [Vec2]);

    /// Run Lloyd relaxation. Boundary vertices stay fixed.
    pub fn lloyd_smooth(&mut self, vertices: &mut [Vec2]);

    /// Convenience: initial_split + subdivide + lloyd_smooth.
    pub fn generate(&mut self, vertices: &mut [Vec2]);
}
```

- Vertices stored in caller-owned `&mut [Vec2]` indexed by `AxialDenseIndexer`.
- Caller allocates: `vec![Vec2::ZERO; AxialDenseIndexer::new(2u32.pow(subdivision)).get_total_size()]`.

## Additions to Existing Types

### AxialCoord::is_boundary

```rust
impl AxialCoord {
    /// Returns true if this coordinate lies on the boundary of a hex grid of given radius.
    /// Operates on the integer coordinate address, not the jittered vertex position.
    pub fn is_boundary(&self, radius: u32) -> bool;
}
```

A coordinate is on the boundary when any of its cube coordinates `(q, r, s)` equals `radius` in absolute value. Used by Lloyd smoothing to keep boundary vertices fixed.

### PatchCoord::quad_vertices

```rust
impl PatchCoord {
    /// Returns the 4 corner AxialCoords of this quad in CCW winding order.
    pub fn quad_vertices(
        &self,
        orientation: PatchOrientation,
        subdivision: u32,
    ) -> [AxialCoord; 4];
}
```

#### Patch-to-Axial Mapping

Each patch is a triangular region of the hex, defined by two consecutive hex vertices and the center. Within each patch, `(u, v)` addresses a quad cell in a `grid_size x grid_size` grid where `grid_size = 2^subdivision`.

The mapping works as follows:
1. **Patch anchor vertices**: Based on `orientation` and patch index `p`, determine the two hex vertices `H_a` and `H_b` that form the patch boundary (with the center as the third point).
   - Even, p=0: vertices 0 and 2; p=1: vertices 2 and 4; p=2: vertices 4 and 0
   - Odd, p=0: vertices 1 and 3; p=1: vertices 3 and 5; p=2: vertices 5 and 1
2. **Affine interpolation**: The 4 corners of quad `(u, v)` are computed by bilinear interpolation within the triangle `(center, H_a, H_b)`:
   - Corner `(u, v)` maps to: `center + (u/grid_size) * (H_a - center) + (v/grid_size) * (H_b - center)`
   - The 4 corners are at `(u, v)`, `(u+1, v)`, `(u+1, v+1)`, `(u, v+1)` in this parametric space.
3. **Integer coordinates**: Since `radius = 2^subdivision = grid_size`, and hex vertices are at integer axial coordinates, all interpolated positions land on integer axial coordinates.

**Hex vertex axial coordinates** at radius R (flat-top, matching existing `AxialCoord::world_coordinate`):
- v0=(R, 0), v1=(0, R), v2=(-R, R), v3=(-R, 0), v4=(0, -R), v5=(R, -R)

**Worked example** (Even orientation, subdivision=1, grid_size=2, radius=2):
- Hex vertices: v0=(2,0), v1=(0,2), v2=(-2,2), v3=(-2,0), v4=(0,-2), v5=(2,-2), center=(0,0)
- Patch 0 anchors: H_a=v0=(2,0), H_b=v2=(-2,2)
- Direction vectors: dir_u = H_a = (2,0), dir_v = H_b = (-2,2)
- Mapping: corner(u,v) = (u/2)\*(2,0) + (v/2)\*(-2,2) = axial (u-v, v)
- Quad (p=0, u=0, v=0): corners (0,0), (1,0), (0,1), (-1,1) — 4 distinct vertices, valid quad
- Quad (p=0, u=1, v=0): corners (1,0), (2,0), (1,1), (0,1) — shares edge with previous quad
- Quad (p=0, u=1, v=1): corners (0,1), (1,1), (0,2), (-1,2) — note (0,2) = v1, the vertex between H_a and H_b

The diagonal corner (grid,grid) always lands on the hex vertex between H_a and H_b (v1 for patch 0), confirming correct patch coverage.

**Key invariant:** Adjacent quads (within or across patches) share the same `AxialCoord` for shared vertices, ensuring a watertight mesh.

## Algorithm Details

### Initial Split

1. Compute 6 hex vertices at radius `2^subdivision` using flat-top hex geometry.
2. Place them at the corresponding `AxialCoord` positions in the vertex buffer.
3. Compute a jittered center point (jitter = `jitter_strength * radius`). If jitter disabled, exact center. Note: the JS POC uses a fixed `radius * 0.1` for center jitter; the Rust port scales by `jitter_strength` instead for a single unified jitter knob.
4. Based on `PatchOrientation`, connect alternating hex vertex pairs to center, forming 3 quads.

### Subdivision (loop-based)

For each depth level `d` in `0..subdivision`:
1. For each existing quad (from current topology at depth `d`):
   - Compute face point (centroid, optionally jittered). If jitter makes child quads non-convex, halve jitter up to 3 times; if still non-convex, use exact centroid.
   - Compute 4 edge midpoints (optionally jittered; boundary edges never jittered).
   - Jitter magnitude: `jitter_strength * radius / 2^d` (halves each level, scaled by radius).
   - Create 4 child quads from corners + edge midpoints + face point.
2. Post-pass: if any child quad is non-convex, blend interior vertices toward neighbor average (70% old + 30% average, up to 20 correction passes).

Vertex positions are written to the dense buffer at their `AxialCoord` locations. The topology at each level is derived from `PatchCoord` iteration at that subdivision depth.

### Lloyd Smoothing

For each iteration (up to `lloyd_iterations`):
1. For each interior vertex (where `!AxialCoord::is_boundary(radius)`):
   - Find all quads touching this vertex (via `PatchCoord` neighbor lookup).
   - Compute area-weighted centroid of those quads.
   - Blend: `new_pos = old_pos + lloyd_strength * (target - old_pos)`.
2. Revert any vertex move that creates a non-convex quad.

### Cross-Platform Determinism

- Random floats produced by converting integer PRNG output to float via division (e.g., `rng.next_u32() as f32 / u32::MAX as f32`), avoiding platform-dependent float PRNG implementations.
- When `jitter_strength <= 0.0`, `rng` is never called — fully deterministic output.

## SVG Export

```rust
/// Export mesh as standalone SVG string for visualization/debugging.
pub fn patch_mesh_to_svg(
    vertices: &[Vec2],
    orientation: PatchOrientation,
    subdivision: u32,
) -> String;
```

- Constructs `AxialDenseIndexer` internally from `2^subdivision`.
- One `<polygon>` per quad, iterated via `PatchCoord` + `quad_vertices`.
- Thin stroke, semi-transparent fill (matching JS POC style).
- ViewBox derived from vertex bounds.
- Standalone SVG, no HTML wrapper.

## File Organization

```
crates/shine-game/src/math/hex/
  mod.rs                    # add new exports
  axial_coord.rs            # + is_boundary() method
  patch_coord.rs            # + quad_vertices() method
  axial_dense_indexer.rs    # unchanged
  patch_dense_indexer.rs    # unchanged
  patch_mesher.rs           # PatchOrientation, PatchMeshConfig, PatchMesher
  patch_mesh_svg.rs         # patch_mesh_to_svg()
```

## Tests

- `AxialCoord::is_boundary` — verify at various radii (center, edge, corner, interior)
- `PatchCoord::quad_vertices` — verify for subdivision 0 and 1 with both orientations; verify shared vertices between adjacent quads within and across patches
- Round-trip: generate mesh, verify all quads are convex via cross-product winding check
- Vertex count matches `AxialDenseIndexer::new(2^subdivision).get_total_size()`
- Jitter disabled (`jitter_strength = 0.0`): verify rng is not consumed (pass a panicking rng)
- SVG output: verify it contains the expected number of `<polygon>` elements

## Usage Example

```rust
let subdivision = 3;
let radius = 2u32.pow(subdivision);
let indexer = AxialDenseIndexer::new(radius);
let mut vertices = vec![Vec2::ZERO; indexer.get_total_size()];
let mut rng = MyDeterministicRng::new(seed);

let config = PatchMeshConfig::new(subdivision, PatchOrientation::Even, &mut rng);
PatchMesher::new(config).generate(&mut vertices);

// Access quad topology
let patch_indexer = PatchDenseIndexer::new(subdivision);
for i in 0..patch_indexer.get_total_size() {
    let patch = patch_indexer.get_coord(i);
    let [a, b, c, d] = patch.quad_vertices(PatchOrientation::Even, subdivision);
    let va = vertices[indexer.get_dense_index(&a)];
    // ... use quad corners
}

// Debug visualization
let svg = patch_mesh_to_svg(&vertices, PatchOrientation::Even, subdivision);
std::fs::write("mesh.svg", svg).unwrap();
```

## Dependencies

- `glam` — `Vec2` for vertex positions (already in workspace)
- `rand` traits — `Rng` for the `&mut dyn Rng` parameter (verify in workspace Cargo.toml; if not present, add as dependency)
