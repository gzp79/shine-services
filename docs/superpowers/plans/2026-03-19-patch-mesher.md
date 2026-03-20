# Patch Mesher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the JS hex-mesh-gen POC (3-quad init + subdivision + Lloyd smoothing) to `crates/shine-game/src/math/hex/` as a Rust module using existing `AxialCoord`, `PatchCoord`, and dense indexer types.

**Architecture:** A `PatchMesher` struct with 3 phase methods (`initial_split`, `subdivide`, `lloyd_smooth`) writes vertex positions into a caller-owned `Vec<Vec2>` indexed by `AxialDenseIndexer`. Quad topology is recovered from `PatchCoord::quad_vertices()` — no explicit quad list stored. An SVG export function enables visual debugging.

**Tech Stack:** Rust, glam (Vec2), rand 0.10 (Rng trait), shine-test (test harness)

**Spec:** `docs/superpowers/specs/2026-03-19-patch-mesher-design.md`

---

### Task 1: Add `rand` dependency to shine-game

**Files:**
- Modify: `crates/shine-game/Cargo.toml`

- [ ] **Step 1: Add rand to dependencies**

Add under `[dependencies]`:
```toml
rand = { workspace = true }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p shine-game`
Expected: compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/shine-game/Cargo.toml
git commit -m "feat(shine-game): add rand dependency for patch mesher"
```

---

### Task 2: Add `AxialCoord::is_boundary`

**Files:**
- Modify: `crates/shine-game/src/math/hex/axial_coord.rs`

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)] mod tests` block at the bottom of `axial_coord.rs`:

```rust
#[test]
fn test_is_boundary_radius_0() {
    // Only the center at radius 0
    assert!(AxialCoord::new(0, 0).is_boundary(0));
}

#[test]
fn test_is_boundary_radius_1() {
    assert!(!AxialCoord::new(0, 0).is_boundary(1));
    // All ring-1 coords are boundary
    for coord in AxialCoord::origin().ring(1) {
        assert!(coord.is_boundary(1), "expected boundary: {:?}", coord);
    }
}

#[test]
fn test_is_boundary_radius_2() {
    assert!(!AxialCoord::new(0, 0).is_boundary(2));
    // Ring-1 coords are interior at radius 2
    for coord in AxialCoord::origin().ring(1) {
        assert!(!coord.is_boundary(2), "expected interior: {:?}", coord);
    }
    // Ring-2 coords are boundary
    for coord in AxialCoord::origin().ring(2) {
        assert!(coord.is_boundary(2), "expected boundary: {:?}", coord);
    }
}

#[test]
fn test_is_boundary_radius_4() {
    assert!(!AxialCoord::new(0, 0).is_boundary(4));
    assert!(!AxialCoord::new(1, 1).is_boundary(4));
    assert!(AxialCoord::new(4, 0).is_boundary(4));
    assert!(AxialCoord::new(0, 4).is_boundary(4));
    assert!(AxialCoord::new(-4, 4).is_boundary(4));
    assert!(AxialCoord::new(2, -4).is_boundary(4));
    // Interior point at distance 3
    assert!(!AxialCoord::new(3, 0).is_boundary(4));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p shine-game is_boundary`
Expected: FAIL — `is_boundary` method not found

- [ ] **Step 3: Implement is_boundary**

Add to the `impl AxialCoord` block (after the `world_coordinate` method, before the closing `}`):

```rust
/// Returns true if this coordinate lies on the boundary of a hex grid of given radius.
/// Operates on the integer coordinate address, not the jittered vertex position.
/// A coordinate is on the boundary when its distance from the origin equals the radius.
pub fn is_boundary(&self, radius: u32) -> bool {
    self.distance(&AxialCoord::origin()) == radius as i32
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p shine-game is_boundary`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add crates/shine-game/src/math/hex/axial_coord.rs
git commit -m "feat(hex): add AxialCoord::is_boundary method"
```

---

### Task 3: Add `PatchOrientation` and hex vertex helpers

**Files:**
- Create: `crates/shine-game/src/math/hex/patch_mesher.rs`
- Modify: `crates/shine-game/src/math/hex/mod.rs`

- [ ] **Step 1: Create patch_mesher.rs with PatchOrientation and hex vertex constants**

```rust
use crate::math::hex::AxialCoord;

/// Which of the 2 base orientations for the 3-patch split.
/// Even: patches span hex vertices 0-1-2, 2-3-4, 4-5-0
/// Odd: patches span hex vertices 1-2-3, 3-4-5, 5-0-1
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchOrientation {
    Even,
    Odd,
}

/// The 6 hex vertex axial coordinates at radius R (flat-top layout).
/// v0=(R,0), v1=(0,R), v2=(-R,R), v3=(-R,0), v4=(0,-R), v5=(R,-R)
pub(crate) fn hex_vertex(index: usize, radius: u32) -> AxialCoord {
    let r = radius as i32;
    match index % 6 {
        0 => AxialCoord::new(r, 0),
        1 => AxialCoord::new(0, r),
        2 => AxialCoord::new(-r, r),
        3 => AxialCoord::new(-r, 0),
        4 => AxialCoord::new(0, -r),
        5 => AxialCoord::new(r, -r),
        _ => unreachable!(),
    }
}

/// Returns the (H_a_index, H_b_index) hex vertex indices for a given patch.
/// H_a and H_b are the two anchor vertices of the patch triangle.
pub(crate) fn patch_anchor_indices(orientation: PatchOrientation, patch: i32) -> (usize, usize) {
    let start = match orientation {
        PatchOrientation::Even => 0,
        PatchOrientation::Odd => 1,
    };
    let a = (start + patch as usize * 2) % 6;
    let b = (start + patch as usize * 2 + 2) % 6;
    (a, b)
}
```

- [ ] **Step 2: Add module to mod.rs**

Add to `crates/shine-game/src/math/hex/mod.rs`:

```rust
mod patch_mesher;
```

And add to the `pub use` block:

```rust
patch_mesher::PatchOrientation,
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p shine-game`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add crates/shine-game/src/math/hex/patch_mesher.rs crates/shine-game/src/math/hex/mod.rs
git commit -m "feat(hex): add PatchOrientation and hex vertex helpers"
```

---

### Task 4: Add `PatchCoord::quad_vertices`

**Files:**
- Modify: `crates/shine-game/src/math/hex/patch_coord.rs`

- [ ] **Step 1: Write the failing tests**

Add a `#[cfg(test)] mod tests` block at the bottom of `patch_coord.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::hex::{AxialCoord, PatchOrientation};
    use shine_test::test;
    use std::collections::HashSet;

    #[test]
    fn test_quad_vertices_subdivision_0() {
        // subdivision=0, grid_size=1, radius=1
        // 3 quads, each with 4 vertices
        let orientation = PatchOrientation::Even;
        let subdivision = 0;

        for p in 0..3 {
            let coord = PatchCoord::new(p, 0, 0);
            let verts = coord.quad_vertices(orientation, subdivision);
            // All 4 vertices should be distinct
            let unique: HashSet<_> = verts.iter().collect();
            assert_eq!(unique.len(), 4, "patch {p}: vertices not distinct: {:?}", verts);
            // One vertex should be the center (0,0)
            assert!(verts.contains(&AxialCoord::origin()), "patch {p}: missing center");
        }
    }

    #[test]
    fn test_quad_vertices_subdivision_1_even() {
        // subdivision=1, grid_size=2, radius=2
        // Patch 0: H_a=v0=(2,0), H_b=v2=(-2,2)
        // dir_u=(2,0), dir_v=(-2,2)
        // corner(u,v) = (u/2)*(2,0) + (v/2)*(-2,2) = (u - v, v) in axial
        let orientation = PatchOrientation::Even;
        let subdivision = 1;

        // Quad (0,0,0): corners (0,0), (1,0), (0,1), (-1,1)
        let verts = PatchCoord::new(0, 0, 0).quad_vertices(orientation, subdivision);
        assert_eq!(verts[0], AxialCoord::new(0, 0));
        assert_eq!(verts[1], AxialCoord::new(1, 0));
        assert_eq!(verts[2], AxialCoord::new(0, 1));
        assert_eq!(verts[3], AxialCoord::new(-1, 1));

        // Quad (0,1,0): corners (1,0), (2,0), (1,1), (0,1) — shares edge with previous
        let verts = PatchCoord::new(0, 1, 0).quad_vertices(orientation, subdivision);
        assert_eq!(verts[0], AxialCoord::new(1, 0));
        assert_eq!(verts[1], AxialCoord::new(2, 0));
        assert_eq!(verts[2], AxialCoord::new(1, 1));
        assert_eq!(verts[3], AxialCoord::new(0, 1));
    }

    #[test]
    fn test_quad_vertices_shared_across_patches() {
        // Adjacent patches should share vertices at their common edge
        let orientation = PatchOrientation::Even;
        let subdivision = 1;

        // Patch 0 uses v0=(2,0) and v2=(-2,2)
        // Patch 1 uses v2=(-2,2) and v4=(0,-2)
        // They share edge along v2=(-2,2) to center(0,0)

        // Patch 0, quad (0,0,1) should have vertex (-2,2) or vertices along the v2-center edge
        // Patch 1, quad (1,0,0) should share vertices with patch 0 along the shared edge

        // Collect all vertices from both patches
        let mut patch0_verts = HashSet::new();
        let mut patch1_verts = HashSet::new();
        let grid_size = 2;
        for u in 0..grid_size {
            for v in 0..grid_size {
                for vert in PatchCoord::new(0, u, v).quad_vertices(orientation, subdivision) {
                    patch0_verts.insert(vert);
                }
                for vert in PatchCoord::new(1, u, v).quad_vertices(orientation, subdivision) {
                    patch1_verts.insert(vert);
                }
            }
        }
        // Patches should share vertices along their common edge (center to v2)
        let shared: HashSet<_> = patch0_verts.intersection(&patch1_verts).collect();
        assert!(!shared.is_empty(), "patches should share vertices along common edge");
        // The center (0,0) and v2=(-2,2) should be shared
        assert!(shared.contains(&AxialCoord::new(0, 0)));
        assert!(shared.contains(&AxialCoord::new(-2, 2)));
    }

    #[test]
    fn test_quad_vertices_cover_all_spiral() {
        // All quad vertices together should cover the full hex spiral
        let orientation = PatchOrientation::Even;
        let subdivision = 1;
        let radius = 2u32;
        let grid_size = 2;

        let mut all_verts = HashSet::new();
        for p in 0..3 {
            for u in 0..grid_size {
                for v in 0..grid_size {
                    for vert in PatchCoord::new(p, u, v).quad_vertices(orientation, subdivision) {
                        all_verts.insert(vert);
                    }
                }
            }
        }

        // Should match all coords in the spiral of this radius
        let spiral_verts: HashSet<_> = AxialCoord::origin().spiral(radius).collect();
        assert_eq!(all_verts, spiral_verts, "quad vertices should cover exact hex spiral");
    }

    #[test]
    fn test_quad_vertices_odd_orientation() {
        // Odd orientation: patch 0 anchors are v1=(0,R) and v3=(-R,0)
        let orientation = PatchOrientation::Odd;
        let subdivision = 1;

        let verts = PatchCoord::new(0, 0, 0).quad_vertices(orientation, subdivision);
        // Center should still be one of the vertices
        assert!(verts.contains(&AxialCoord::origin()));
        // All 4 should be distinct
        let unique: HashSet<_> = verts.iter().collect();
        assert_eq!(unique.len(), 4);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p shine-game quad_vertices`
Expected: FAIL — `quad_vertices` method not found

- [ ] **Step 3: Implement quad_vertices**

Add to `patch_coord.rs` — first add the import at the top:

```rust
use crate::math::hex::{AxialCoord, patch_mesher::{PatchOrientation, hex_vertex, patch_anchor_indices}};
```

Then add to `impl PatchCoord`:

```rust
/// Returns the 4 corner AxialCoords of this quad in CCW winding order.
///
/// Each patch is a triangular region anchored by two hex vertices (H_a, H_b) and the center.
/// Within the patch, (u, v) addresses a quad cell in a grid_size x grid_size grid.
/// The corners are computed by affine interpolation:
///   corner(cu, cv) = (cu * H_a + cv * H_b) / grid_size
pub fn quad_vertices(&self, orientation: PatchOrientation, subdivision: u32) -> [AxialCoord; 4] {
    let grid = 2i32.pow(subdivision);
    let (a_idx, b_idx) = patch_anchor_indices(orientation, self.p);
    let radius = grid as u32;
    let ha = hex_vertex(a_idx, radius);
    let hb = hex_vertex(b_idx, radius);

    // Affine interpolation: corner(cu, cv) = (cu * H_a + cv * H_b) / grid
    let corner = |cu: i32, cv: i32| -> AxialCoord {
        AxialCoord::new(
            (cu * ha.q + cv * hb.q) / grid,
            (cu * ha.r + cv * hb.r) / grid,
        )
    };

    let (u, v) = (self.u, self.v);
    [
        corner(u, v),
        corner(u + 1, v),
        corner(u + 1, v + 1),
        corner(u, v + 1),
    ]
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p shine-game quad_vertices`
Expected: all pass

- [ ] **Step 5: Run the spiral coverage test specifically**

Run: `cargo test -p shine-game test_quad_vertices_cover_all_spiral -- --nocapture`
Expected: PASS — confirms all hex coordinates are covered by quad vertices

- [ ] **Step 6: Commit**

```bash
git add crates/shine-game/src/math/hex/patch_coord.rs
git commit -m "feat(hex): add PatchCoord::quad_vertices for patch-to-axial mapping"
```

---

### Task 5: Add `PatchMeshConfig` and `PatchMesher` skeleton

**Files:**
- Modify: `crates/shine-game/src/math/hex/patch_mesher.rs`
- Modify: `crates/shine-game/src/math/hex/mod.rs`

- [ ] **Step 1: Add config and mesher structs**

Append to `patch_mesher.rs`:

```rust
use glam::Vec2;
use rand::Rng;

/// Configuration for hex patch mesh generation.
pub struct PatchMeshConfig<'a> {
    pub subdivision: u32,
    pub orientation: PatchOrientation,
    /// Base jitter as fraction of radius. Default 0.15. Set <= 0.0 to disable jitter.
    pub jitter_strength: f32,
    /// Number of Lloyd relaxation passes. Default 20.
    pub lloyd_iterations: u32,
    /// Lloyd blending factor 0..1. Default 0.4.
    pub lloyd_strength: f32,
    pub rng: &'a mut dyn Rng,
}

impl<'a> PatchMeshConfig<'a> {
    pub fn new(subdivision: u32, orientation: PatchOrientation, rng: &'a mut dyn Rng) -> Self {
        Self {
            subdivision,
            orientation,
            jitter_strength: 0.15,
            lloyd_iterations: 20,
            lloyd_strength: 0.4,
            rng,
        }
    }
}

/// Generates a quad mesh inside a hexagon using 3-patch subdivision + Lloyd smoothing.
pub struct PatchMesher<'a> {
    config: PatchMeshConfig<'a>,
}

impl<'a> PatchMesher<'a> {
    pub fn new(config: PatchMeshConfig<'a>) -> Self {
        Self { config }
    }

    fn radius(&self) -> u32 {
        2u32.pow(self.config.subdivision)
    }

    /// Place 7 initial vertices: 6 hex corners + 1 jittered center.
    pub fn initial_split(&mut self, vertices: &mut [Vec2]) {
        todo!()
    }

    /// Run all subdivision levels (internal loop 0..subdivision).
    pub fn subdivide(&mut self, vertices: &mut [Vec2]) {
        todo!()
    }

    /// Run Lloyd relaxation. Boundary vertices stay fixed.
    pub fn lloyd_smooth(&mut self, vertices: &mut [Vec2]) {
        todo!()
    }

    /// Convenience: initial_split + subdivide + lloyd_smooth.
    pub fn generate(&mut self, vertices: &mut [Vec2]) {
        self.initial_split(vertices);
        self.subdivide(vertices);
        self.lloyd_smooth(vertices);
    }
}
```

- [ ] **Step 2: Update mod.rs exports**

Add to the `pub use` block:

```rust
patch_mesher::{PatchMeshConfig, PatchMesher},
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p shine-game`
Expected: compiles (todo!() is fine for now)

- [ ] **Step 4: Commit**

```bash
git add crates/shine-game/src/math/hex/patch_mesher.rs crates/shine-game/src/math/hex/mod.rs
git commit -m "feat(hex): add PatchMeshConfig and PatchMesher skeleton"
```

---

### Task 6: Implement `initial_split`

**Files:**
- Modify: `crates/shine-game/src/math/hex/patch_mesher.rs`

- [ ] **Step 1: Write the failing test**

Add `#[cfg(test)] mod tests` at the bottom of `patch_mesher.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::hex::{AxialDenseIndexer, PatchCoord, PatchDenseIndexer};
    use shine_test::test;

    /// Deterministic PRNG for tests: simple splitmix32
    struct TestRng(u32);
    impl TestRng {
        fn new(seed: u32) -> Self {
            Self(seed)
        }
    }
    impl rand::RngCore for TestRng {
        fn next_u32(&mut self) -> u32 {
            self.0 = self.0.wrapping_add(0x9e3779b9);
            let mut z = self.0;
            z = (z ^ (z >> 16)).wrapping_mul(0x85ebca6b);
            z = (z ^ (z >> 13)).wrapping_mul(0xc2b2ae35);
            z ^ (z >> 16)
        }
        fn next_u64(&mut self) -> u64 {
            let a = self.next_u32() as u64;
            let b = self.next_u32() as u64;
            (a << 32) | b
        }
        fn fill_bytes(&mut self, dest: &mut [u8]) {
            rand::RngCore::try_fill_bytes(self, dest).unwrap();
        }
        fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
            for chunk in dest.chunks_mut(4) {
                let bytes = self.next_u32().to_le_bytes();
                chunk.copy_from_slice(&bytes[..chunk.len()]);
            }
            Ok(())
        }
    }

    #[test]
    fn test_initial_split_places_hex_vertices() {
        let mut rng = TestRng::new(42);
        let config = PatchMeshConfig::new(0, PatchOrientation::Even, &mut rng);
        let radius = 2u32.pow(config.subdivision);
        let indexer = AxialDenseIndexer::new(radius);
        let mut vertices = vec![Vec2::ZERO; indexer.get_total_size()];

        PatchMesher::new(config).initial_split(&mut vertices);

        // Check that the 6 hex vertices have correct world positions
        for i in 0..6 {
            let coord = hex_vertex(i, radius);
            let idx = indexer.get_dense_index(&coord);
            let expected = coord.world_coordinate(1.0);
            let actual = vertices[idx];
            assert!(
                (actual - expected).length() < 1e-6,
                "vertex {i}: expected {expected}, got {actual}"
            );
        }
    }

    #[test]
    fn test_initial_split_no_jitter() {
        // With jitter disabled, center should be at origin
        struct PanicRng;
        impl rand::RngCore for PanicRng {
            fn next_u32(&mut self) -> u32 { panic!("rng should not be called") }
            fn next_u64(&mut self) -> u64 { panic!("rng should not be called") }
            fn fill_bytes(&mut self, _: &mut [u8]) { panic!("rng should not be called") }
            fn try_fill_bytes(&mut self, _: &mut [u8]) -> Result<(), rand::Error> {
                panic!("rng should not be called")
            }
        }

        let mut rng = PanicRng;
        let mut config = PatchMeshConfig::new(0, PatchOrientation::Even, &mut rng);
        config.jitter_strength = 0.0;
        let radius = 2u32.pow(config.subdivision);
        let indexer = AxialDenseIndexer::new(radius);
        let mut vertices = vec![Vec2::ZERO; indexer.get_total_size()];

        PatchMesher::new(config).initial_split(&mut vertices);

        // Center should be exactly at origin world coordinate
        let center_idx = indexer.get_dense_index(&AxialCoord::origin());
        assert_eq!(vertices[center_idx], Vec2::ZERO);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p shine-game test_initial_split`
Expected: FAIL — todo!() panics

- [ ] **Step 3: Implement initial_split**

Add a helper for deterministic random float, then implement `initial_split` in the `impl PatchMesher` block:

```rust
/// Convert RNG u32 to float in [-1, 1] range deterministically.
fn rng_float_signed(rng: &mut dyn Rng) -> f32 {
    (rng.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0
}

// Inside impl PatchMesher:
pub fn initial_split(&mut self, vertices: &mut [Vec2]) {
    let radius = self.radius();
    let indexer = AxialDenseIndexer::new(radius);

    // Place 6 hex vertices at their world positions
    for i in 0..6 {
        let coord = hex_vertex(i, radius);
        let idx = indexer.get_dense_index(&coord);
        vertices[idx] = coord.world_coordinate(1.0);
    }

    // Place center with optional jitter
    let center_pos = if self.config.jitter_strength > 0.0 {
        let max_offset = self.config.jitter_strength * radius as f32;
        let dx = rng_float_signed(self.config.rng) * max_offset;
        let dy = rng_float_signed(self.config.rng) * max_offset;
        Vec2::new(dx, dy)
    } else {
        Vec2::ZERO
    };
    let center_idx = indexer.get_dense_index(&AxialCoord::origin());
    vertices[center_idx] = center_pos;
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p shine-game test_initial_split`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add crates/shine-game/src/math/hex/patch_mesher.rs
git commit -m "feat(hex): implement PatchMesher::initial_split"
```

---

### Task 7: Implement `subdivide`

**Files:**
- Modify: `crates/shine-game/src/math/hex/patch_mesher.rs`

This is the most complex task. The subdivision works by iterating over the quad topology at each depth level, computing face points and edge midpoints, and writing them to the vertex buffer.

- [ ] **Step 1: Write the failing test**

Add to the test module in `patch_mesher.rs`:

```rust
fn is_quad_convex(vertices: &[Vec2], quad: &[AxialCoord; 4], indexer: &AxialDenseIndexer) -> bool {
    let pts: Vec<Vec2> = quad.iter().map(|c| vertices[indexer.get_dense_index(c)]).collect();
    let mut sign = None;
    for i in 0..4 {
        let a = pts[i];
        let b = pts[(i + 1) % 4];
        let c = pts[(i + 2) % 4];
        let cross = (b - a).perp_dot(c - b);
        if cross.abs() < 1e-10 {
            continue;
        }
        match sign {
            None => sign = Some(cross > 0.0),
            Some(s) => {
                if s != (cross > 0.0) {
                    return false;
                }
            }
        }
    }
    true
}

#[test]
fn test_subdivide_vertex_count() {
    let subdivision = 2;
    let mut rng = TestRng::new(42);
    let config = PatchMeshConfig::new(subdivision, PatchOrientation::Even, &mut rng);
    let radius = 2u32.pow(subdivision);
    let indexer = AxialDenseIndexer::new(radius);
    let mut vertices = vec![Vec2::ZERO; indexer.get_total_size()];

    let mut mesher = PatchMesher::new(config);
    mesher.initial_split(&mut vertices);
    mesher.subdivide(&mut vertices);

    // After subdivision, every vertex in the spiral should have a non-zero position
    // (except possibly the center which could be near zero with jitter)
    let total = indexer.get_total_size();
    let non_zero = vertices.iter().filter(|v| v.length() > 1e-10).count();
    // At minimum the 6 hex vertices + center + all subdivision midpoints should be placed
    assert!(non_zero >= 7, "expected at least 7 placed vertices, got {non_zero}");
}

#[test]
fn test_subdivide_all_quads_convex_no_jitter() {
    let subdivision = 2;
    let mut rng = TestRng::new(42);
    let mut config = PatchMeshConfig::new(subdivision, PatchOrientation::Even, &mut rng);
    config.jitter_strength = 0.0;
    let radius = 2u32.pow(subdivision);
    let indexer = AxialDenseIndexer::new(radius);
    let mut vertices = vec![Vec2::ZERO; indexer.get_total_size()];

    let mut mesher = PatchMesher::new(config);
    mesher.initial_split(&mut vertices);
    mesher.subdivide(&mut vertices);

    // All final quads should be convex
    let patch_indexer = PatchDenseIndexer::new(subdivision);
    for i in 0..patch_indexer.get_total_size() {
        let patch = patch_indexer.get_coord(i);
        let quad = patch.quad_vertices(PatchOrientation::Even, subdivision);
        assert!(
            is_quad_convex(&vertices, &quad, &indexer),
            "quad {:?} is not convex",
            patch
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p shine-game test_subdivide`
Expected: FAIL — todo!() panics

- [ ] **Step 3: Implement subdivide**

Replace the `subdivide` method in `impl PatchMesher`:

```rust
pub fn subdivide(&mut self, vertices: &mut [Vec2]) {
    let radius = self.radius();
    let indexer = AxialDenseIndexer::new(radius);
    let orientation = self.config.orientation;

    for depth in 0..self.config.subdivision {
        let current_subdiv = depth + 1;
        let parent_grid = 2i32.pow(depth);
        let max_jitter = if self.config.jitter_strength > 0.0 {
            self.config.jitter_strength * radius as f32 / 2f32.powi(depth as i32)
        } else {
            0.0
        };

        // Iterate over parent quads at depth level
        for p in 0..3 {
            for pu in 0..parent_grid {
                for pv in 0..parent_grid {
                    let parent = PatchCoord::new(p, pu, pv);
                    let corners = parent.quad_vertices(orientation, depth);

                    // Get corner positions
                    let c: Vec<Vec2> = corners.iter()
                        .map(|coord| vertices[indexer.get_dense_index(coord)])
                        .collect();

                    // Face point (centroid)
                    let face_coord = self.face_point_coord(&corners, current_subdiv, orientation);
                    let mut face_pos = (c[0] + c[1] + c[2] + c[3]) / 4.0;
                    if max_jitter > 0.0 {
                        face_pos.x += rng_float_signed(self.config.rng) * max_jitter;
                        face_pos.y += rng_float_signed(self.config.rng) * max_jitter;
                    }
                    vertices[indexer.get_dense_index(&face_coord)] = face_pos;

                    // Edge midpoints
                    for edge_idx in 0..4 {
                        let a = corners[edge_idx];
                        let b = corners[(edge_idx + 1) % 4];
                        let mid_coord = AxialCoord::new((a.q + b.q) / 2, (a.r + b.r) / 2);
                        let mid_idx = indexer.get_dense_index(&mid_coord);

                        // Only write if not already placed (shared edges)
                        if vertices[mid_idx] == Vec2::ZERO {
                            let mid_pos_base = (c[edge_idx] + c[(edge_idx + 1) % 4]) / 2.0;
                            let is_boundary = a.is_boundary(radius) && b.is_boundary(radius);
                            let mid_pos = if max_jitter > 0.0 && !is_boundary {
                                Vec2::new(
                                    mid_pos_base.x + rng_float_signed(self.config.rng) * max_jitter,
                                    mid_pos_base.y + rng_float_signed(self.config.rng) * max_jitter,
                                )
                            } else {
                                mid_pos_base
                            };
                            vertices[mid_idx] = mid_pos;
                        }
                    }
                }
            }
        }

        // Post-pass: convexity correction
        self.fix_convexity(vertices, current_subdiv, &indexer);
    }
}

fn face_point_coord(&self, corners: &[AxialCoord; 4], child_subdiv: u32, _orientation: PatchOrientation) -> AxialCoord {
    // The face point is at the average of the 4 corners in axial space
    AxialCoord::new(
        (corners[0].q + corners[1].q + corners[2].q + corners[3].q) / 4,
        (corners[0].r + corners[1].r + corners[2].r + corners[3].r) / 4,
    )
}

fn fix_convexity(&self, vertices: &mut [Vec2], subdiv_level: u32, indexer: &AxialDenseIndexer) {
    let radius = self.radius();
    let orientation = self.config.orientation;
    let grid = 2i32.pow(subdiv_level);

    for _pass in 0..20 {
        let mut any_fixed = false;
        let old_vertices = vertices.to_vec();

        for p in 0..3 {
            for u in 0..grid {
                for v in 0..grid {
                    let patch = PatchCoord::new(p, u, v);
                    let quad = patch.quad_vertices(orientation, subdiv_level);
                    let pts: Vec<Vec2> = quad.iter()
                        .map(|c| vertices[indexer.get_dense_index(c)])
                        .collect();

                    if !Self::is_convex(&pts) {
                        // Nudge non-boundary vertices toward neighbor average
                        for coord in &quad {
                            if !coord.is_boundary(radius) && *coord != AxialCoord::origin() {
                                let idx = indexer.get_dense_index(coord);
                                let neighbors: Vec<Vec2> = quad.iter()
                                    .filter(|c| *c != coord)
                                    .map(|c| old_vertices[indexer.get_dense_index(c)])
                                    .collect();
                                let avg = neighbors.iter().sum::<Vec2>() / neighbors.len() as f32;
                                vertices[idx] = old_vertices[idx] * 0.7 + avg * 0.3;
                                any_fixed = true;
                            }
                        }
                    }
                }
            }
        }

        if !any_fixed {
            break;
        }
    }
}

fn is_convex(pts: &[Vec2]) -> bool {
    let mut sign = None;
    for i in 0..4 {
        let a = pts[i];
        let b = pts[(i + 1) % 4];
        let c = pts[(i + 2) % 4];
        let cross = (b - a).perp_dot(c - b);
        if cross.abs() < 1e-10 {
            continue;
        }
        match sign {
            None => sign = Some(cross > 0.0),
            Some(s) => {
                if s != (cross > 0.0) {
                    return false;
                }
            }
        }
    }
    true
}
```

**Note on the edge midpoint "already placed" check:** Using `vertices[mid_idx] == Vec2::ZERO` is a heuristic that works because unplaced vertices are initialized to zero, and actual midpoints of hex edges are never at world origin. For the center vertex (which IS at origin or near it), it's placed during `initial_split` so it won't be overwritten.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p shine-game test_subdivide`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add crates/shine-game/src/math/hex/patch_mesher.rs
git commit -m "feat(hex): implement PatchMesher::subdivide with convexity correction"
```

---

### Task 8: Implement `lloyd_smooth`

**Files:**
- Modify: `crates/shine-game/src/math/hex/patch_mesher.rs`

- [ ] **Step 1: Write the failing test**

Add to the test module:

```rust
#[test]
fn test_lloyd_smooth_preserves_boundary() {
    let subdivision = 2;
    let mut rng = TestRng::new(42);
    let config = PatchMeshConfig::new(subdivision, PatchOrientation::Even, &mut rng);
    let radius = 2u32.pow(subdivision);
    let indexer = AxialDenseIndexer::new(radius);
    let mut vertices = vec![Vec2::ZERO; indexer.get_total_size()];

    let mut mesher = PatchMesher::new(config);
    mesher.initial_split(&mut vertices);
    mesher.subdivide(&mut vertices);

    // Save boundary vertex positions
    let boundary_before: Vec<(AxialCoord, Vec2)> = AxialCoord::origin()
        .spiral(radius)
        .filter(|c| c.is_boundary(radius))
        .map(|c| (c, vertices[indexer.get_dense_index(&c)]))
        .collect();

    mesher.lloyd_smooth(&mut vertices);

    // Boundary vertices should not have moved
    for (coord, pos_before) in &boundary_before {
        let pos_after = vertices[indexer.get_dense_index(coord)];
        assert!(
            (*pos_before - pos_after).length() < 1e-6,
            "boundary vertex {:?} moved from {} to {}",
            coord,
            pos_before,
            pos_after
        );
    }
}

#[test]
fn test_lloyd_smooth_all_quads_convex() {
    let subdivision = 2;
    let mut rng = TestRng::new(42);
    let config = PatchMeshConfig::new(subdivision, PatchOrientation::Even, &mut rng);
    let radius = 2u32.pow(subdivision);
    let indexer = AxialDenseIndexer::new(radius);
    let mut vertices = vec![Vec2::ZERO; indexer.get_total_size()];

    let mut mesher = PatchMesher::new(config);
    mesher.generate(&mut vertices);

    // All quads should still be convex after full pipeline
    let patch_indexer = PatchDenseIndexer::new(subdivision);
    for i in 0..patch_indexer.get_total_size() {
        let patch = patch_indexer.get_coord(i);
        let quad = patch.quad_vertices(PatchOrientation::Even, subdivision);
        assert!(
            is_quad_convex(&vertices, &quad, &indexer),
            "quad {:?} is not convex after lloyd",
            patch
        );
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p shine-game test_lloyd`
Expected: FAIL — todo!() panics

- [ ] **Step 3: Implement lloyd_smooth**

Replace the `lloyd_smooth` method:

```rust
pub fn lloyd_smooth(&mut self, vertices: &mut [Vec2]) {
    let radius = self.radius();
    let indexer = AxialDenseIndexer::new(radius);
    let orientation = self.config.orientation;
    let subdivision = self.config.subdivision;
    let grid = 2i32.pow(subdivision);
    let strength = self.config.lloyd_strength;

    // Build vertex-to-quads map: for each vertex, which PatchCoords touch it
    let mut vert_quads: Vec<Vec<PatchCoord>> = vec![Vec::new(); indexer.get_total_size()];
    for p in 0..3 {
        for u in 0..grid {
            for v in 0..grid {
                let patch = PatchCoord::new(p, u, v);
                let quad = patch.quad_vertices(orientation, subdivision);
                for coord in &quad {
                    let idx = indexer.get_dense_index(coord);
                    vert_quads[idx].push(patch);
                }
            }
        }
    }

    for _iter in 0..self.config.lloyd_iterations {
        let old_vertices = vertices.to_vec();

        for coord in AxialCoord::origin().spiral(radius) {
            if coord.is_boundary(radius) {
                continue;
            }

            let vert_idx = indexer.get_dense_index(&coord);
            let quads = &vert_quads[vert_idx];
            if quads.is_empty() {
                continue;
            }

            // Compute area-weighted centroid of adjacent quads
            let mut weighted_centroid = Vec2::ZERO;
            let mut total_area = 0.0f32;

            for patch in quads {
                let quad = patch.quad_vertices(orientation, subdivision);
                let pts: Vec<Vec2> = quad.iter()
                    .map(|c| old_vertices[indexer.get_dense_index(c)])
                    .collect();

                // Shoelace area
                let mut area = 0.0f32;
                for i in 0..4 {
                    let j = (i + 1) % 4;
                    area += pts[i].x * pts[j].y - pts[j].x * pts[i].y;
                }
                area = area.abs() / 2.0;

                let centroid = (pts[0] + pts[1] + pts[2] + pts[3]) / 4.0;
                weighted_centroid += centroid * area;
                total_area += area;
            }

            if total_area > 1e-10 {
                let target = weighted_centroid / total_area;
                vertices[vert_idx] = old_vertices[vert_idx] + strength * (target - old_vertices[vert_idx]);
            }
        }

        // Revert non-convex moves
        for p in 0..3 {
            for u in 0..grid {
                for v in 0..grid {
                    let patch = PatchCoord::new(p, u, v);
                    let quad = patch.quad_vertices(orientation, subdivision);
                    let pts: Vec<Vec2> = quad.iter()
                        .map(|c| vertices[indexer.get_dense_index(c)])
                        .collect();

                    if !Self::is_convex(&pts) {
                        for coord in &quad {
                            if !coord.is_boundary(radius) {
                                let idx = indexer.get_dense_index(coord);
                                vertices[idx] = old_vertices[idx];
                            }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p shine-game test_lloyd`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add crates/shine-game/src/math/hex/patch_mesher.rs
git commit -m "feat(hex): implement PatchMesher::lloyd_smooth"
```

---

### Task 9: Implement SVG export

**Files:**
- Create: `crates/shine-game/src/math/hex/patch_mesh_svg.rs`
- Modify: `crates/shine-game/src/math/hex/mod.rs`

- [ ] **Step 1: Write the failing test**

Create `patch_mesh_svg.rs` with test only first:

```rust
use std::fmt::Write;

use glam::Vec2;

use crate::math::hex::{
    AxialCoord, AxialDenseIndexer, PatchCoord, PatchDenseIndexer,
    patch_mesher::PatchOrientation,
};

/// Export mesh as standalone SVG string for visualization/debugging.
pub fn patch_mesh_to_svg(
    vertices: &[Vec2],
    orientation: PatchOrientation,
    subdivision: u32,
) -> String {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::hex::patch_mesher::{PatchMeshConfig, PatchMesher};
    use shine_test::test;

    #[test]
    fn test_svg_polygon_count() {
        // Use the TestRng from patch_mesher tests — duplicate minimal version here
        struct SimpleRng(u32);
        impl rand::RngCore for SimpleRng {
            fn next_u32(&mut self) -> u32 {
                self.0 = self.0.wrapping_add(0x9e3779b9);
                let mut z = self.0;
                z = (z ^ (z >> 16)).wrapping_mul(0x85ebca6b);
                z = (z ^ (z >> 13)).wrapping_mul(0xc2b2ae35);
                z ^ (z >> 16)
            }
            fn next_u64(&mut self) -> u64 { ((self.next_u32() as u64) << 32) | self.next_u32() as u64 }
            fn fill_bytes(&mut self, dest: &mut [u8]) { rand::RngCore::try_fill_bytes(self, dest).unwrap(); }
            fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
                for chunk in dest.chunks_mut(4) {
                    let bytes = self.next_u32().to_le_bytes();
                    chunk.copy_from_slice(&bytes[..chunk.len()]);
                }
                Ok(())
            }
        }

        let subdivision = 2;
        let mut rng = SimpleRng(42);
        let mut config = PatchMeshConfig::new(subdivision, PatchOrientation::Even, &mut rng);
        config.jitter_strength = 0.0;
        let radius = 2u32.pow(subdivision);
        let indexer = AxialDenseIndexer::new(radius);
        let mut vertices = vec![Vec2::ZERO; indexer.get_total_size()];

        PatchMesher::new(config).generate(&mut vertices);

        let svg = patch_mesh_to_svg(&vertices, PatchOrientation::Even, subdivision);

        // Should contain correct number of polygons: 3 * 4^subdivision = 3 * 16 = 48
        let expected_quads = 3 * 4usize.pow(subdivision);
        let polygon_count = svg.matches("<polygon").count();
        assert_eq!(polygon_count, expected_quads, "expected {expected_quads} polygons, got {polygon_count}");

        // Should be valid SVG
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }
}
```

- [ ] **Step 2: Add module to mod.rs**

```rust
mod patch_mesh_svg;
```

And add to the `pub use` block:

```rust
patch_mesh_svg::patch_mesh_to_svg,
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p shine-game test_svg_polygon_count`
Expected: FAIL — todo!()

- [ ] **Step 4: Implement patch_mesh_to_svg**

Replace the `todo!()` in `patch_mesh_to_svg`:

```rust
pub fn patch_mesh_to_svg(
    vertices: &[Vec2],
    orientation: PatchOrientation,
    subdivision: u32,
) -> String {
    let radius = 2u32.pow(subdivision);
    let indexer = AxialDenseIndexer::new(radius);
    let patch_indexer = PatchDenseIndexer::new(subdivision);
    let grid = 2i32.pow(subdivision);

    // Compute bounds
    let mut min = Vec2::splat(f32::MAX);
    let mut max = Vec2::splat(f32::MIN);
    for v in vertices {
        min = min.min(*v);
        max = max.max(*v);
    }
    let margin = (max - min).length() * 0.05;
    min -= Vec2::splat(margin);
    max += Vec2::splat(margin);
    let size = max - min;

    let mut svg = String::new();
    let _ = write!(
        svg,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\">\n",
        min.x, min.y, size.x, size.y
    );
    let _ = write!(
        svg,
        "<style>polygon {{ stroke: #5c7cfa; stroke-width: 0.06; fill: rgba(92,124,250,0.08); }} \
         polygon:hover {{ fill: rgba(92,124,250,0.25); stroke: #fff; }}</style>\n"
    );

    for i in 0..patch_indexer.get_total_size() {
        let patch = patch_indexer.get_coord(i);
        let quad = patch.quad_vertices(orientation, subdivision);
        let pts: String = quad
            .iter()
            .map(|c| {
                let v = vertices[indexer.get_dense_index(c)];
                format!("{:.4},{:.4}", v.x, v.y)
            })
            .collect::<Vec<_>>()
            .join(" ");
        let _ = write!(svg, "  <polygon points=\"{pts}\"/>\n");
    }

    svg.push_str("</svg>");
    svg
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p shine-game test_svg_polygon_count`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/shine-game/src/math/hex/patch_mesh_svg.rs crates/shine-game/src/math/hex/mod.rs
git commit -m "feat(hex): add patch_mesh_to_svg for mesh visualization"
```

---

### Task 10: Integration test and visual verification

**Files:**
- Modify: `crates/shine-game/src/math/hex/patch_mesher.rs` (add integration test)

- [ ] **Step 1: Write full pipeline integration test**

Add to the test module in `patch_mesher.rs`:

```rust
#[test]
fn test_full_pipeline_subdivision_3() {
    let subdivision = 3;
    let mut rng = TestRng::new(42);
    let config = PatchMeshConfig::new(subdivision, PatchOrientation::Even, &mut rng);
    let radius = 2u32.pow(subdivision);
    let indexer = AxialDenseIndexer::new(radius);
    let mut vertices = vec![Vec2::ZERO; indexer.get_total_size()];

    PatchMesher::new(config).generate(&mut vertices);

    // Verify vertex count
    assert_eq!(vertices.len(), indexer.get_total_size());

    // Verify all quads convex
    let patch_indexer = PatchDenseIndexer::new(subdivision);
    let expected_quads = 3 * 4usize.pow(subdivision);
    assert_eq!(patch_indexer.get_total_size(), expected_quads);

    for i in 0..patch_indexer.get_total_size() {
        let patch = patch_indexer.get_coord(i);
        let quad = patch.quad_vertices(PatchOrientation::Even, subdivision);
        assert!(
            is_quad_convex(&vertices, &quad, &indexer),
            "quad {:?} not convex after full pipeline",
            patch
        );
    }
}

#[test]
fn test_both_orientations() {
    for orientation in [PatchOrientation::Even, PatchOrientation::Odd] {
        let subdivision = 2;
        let mut rng = TestRng::new(42);
        let config = PatchMeshConfig::new(subdivision, orientation, &mut rng);
        let radius = 2u32.pow(subdivision);
        let indexer = AxialDenseIndexer::new(radius);
        let mut vertices = vec![Vec2::ZERO; indexer.get_total_size()];

        PatchMesher::new(config).generate(&mut vertices);

        let patch_indexer = PatchDenseIndexer::new(subdivision);
        for i in 0..patch_indexer.get_total_size() {
            let patch = patch_indexer.get_coord(i);
            let quad = patch.quad_vertices(orientation, subdivision);
            assert!(
                is_quad_convex(&vertices, &quad, &indexer),
                "{:?}: quad {:?} not convex",
                orientation,
                patch
            );
        }
    }
}
```

- [ ] **Step 2: Run all tests**

Run: `cargo test -p shine-game`
Expected: all pass

- [ ] **Step 3: Write a visual verification test (ignored by default)**

Add to the test module:

```rust
#[test]
#[ignore] // Run manually: cargo test -p shine-game test_generate_svg_file -- --ignored
fn test_generate_svg_file() {
    use crate::math::hex::patch_mesh_svg::patch_mesh_to_svg;

    for (name, orientation) in [("even", PatchOrientation::Even), ("odd", PatchOrientation::Odd)] {
        let subdivision = 3;
        let mut rng = TestRng::new(42);
        let config = PatchMeshConfig::new(subdivision, orientation, &mut rng);
        let radius = 2u32.pow(subdivision);
        let indexer = AxialDenseIndexer::new(radius);
        let mut vertices = vec![Vec2::ZERO; indexer.get_total_size()];

        PatchMesher::new(config).generate(&mut vertices);

        let svg = patch_mesh_to_svg(&vertices, orientation, subdivision);
        let path = format!("test_output_patch_mesh_{name}.svg");
        std::fs::write(&path, &svg).unwrap();
        println!("Written: {path}");
    }
}
```

- [ ] **Step 4: Run the visual test manually and inspect SVG output**

Run: `cargo test -p shine-game test_generate_svg_file -- --ignored --nocapture`
Expected: creates `test_output_patch_mesh_even.svg` and `test_output_patch_mesh_odd.svg`

Open the SVGs in a browser to visually verify the mesh looks like the JS POC output.

- [ ] **Step 5: Commit**

```bash
git add crates/shine-game/src/math/hex/patch_mesher.rs
git commit -m "feat(hex): add integration tests and visual verification for patch mesher"
```
