---
name: quad-mesh-topology
description: QuadMesh topology with ghost vertices/quads - use when working with QuadMesh, QuadTopology, mesh filters, vertex rings, or any code in crates/shine-game/src/math/mesh/. Covers navigation, boundary detection, and common mesh operation patterns.
---

# QuadMesh Topology Reference

Quick reference for working with `QuadTopology` in shine-services.

## Core Concept

`QuadTopology` uses **ghost vertices and ghost quads** to close the mesh topology:
- **Ghost vertex**: `VertIdx(vertex_count)` - a topological vertex with no position
- **Ghost quads**: One per 2 boundary edges, connecting 3 boundary vertices + ghost vertex
- **Result**: Complete rings for ALL vertices (interior and boundary)

This eliminates special-case boundary handling in mesh operations.

**Ghost quad structure**: For boundary edges (v0→v1→v2), ghost quad is `[ghost, v2, v1, v0]` - forms a pyramid with ghost vertex as apex, boundary vertices reversed to match neighbor adjacency across the boundary edge.

## Construction

```rust
QuadTopology::new(
    vertex_count: usize,
    polygon: Vec<VertIdx>,      // Boundary vertices in order (must be even length)
    quads: Vec<[VertIdx; 4]>,   // Real quads (CCW winding)
) -> Result<Self, QuadTopologyError>
```

Validates inputs, generates N/2 ghost quads for N boundary vertices, builds neighbor adjacency and vertex→quad maps. Returns error if boundary is odd length, vertices out of range, or topology is invalid.

**QuadMesh vs QuadTopology**: `QuadMesh` bundles `positions: IdxVec<VertIdx, Vec2>` + `topology: QuadTopology`. Use `QuadTopology` methods for connectivity queries, `QuadMesh` for position-dependent operations. Ghost vertex has no position in the positions array.

## Navigation Types

```rust
struct QuadVertex { quad: QuadIdx, local: u8 }  // Vertex position within a quad
struct QuadEdge { quad: QuadIdx, edge: u8 }     // Edge of a quad
```

**QuadVertex navigation** (pure index arithmetic):
- `qv.next()` → next vertex CCW around quad
- `qv.prev()` → previous vertex CCW
- `qv.opposite()` → vertex across the quad
- `qv.outgoing_edge()` → edge leaving this vertex
- `qv.incoming_edge()` → edge entering this vertex

**QuadEdge navigation**:
- `qe.start()` → QuadVertex at edge start
- `qe.end()` → QuadVertex at edge end

**Topology queries** (require `&QuadTopology`):
- `topology.vertex_index(qv)` → actual `VertIdx`
- `topology.edge_vertices(qe)` → `(VertIdx, VertIdx)`
- `topology.quad_neighbor(qe)` → neighboring `QuadEdge`

## Key Operations

### Vertex Ring Traversal

Get all quads around a vertex (works for interior, boundary, and ghost vertices):

```rust
for qv in topology.vertex_ring(vi) {
    let this_vertex = topology.vertex_index(qv);
    let next_vertex = topology.vertex_index(qv.next());
    // Process quad...
}
```

The ring always closes - ghost quads complete boundary vertices' rings.

**How it works**: Start at `vertex_quad[vi]`, yield current quad, move to `incoming_edge()` neighbor (the edge *ending* at this vertex connects to previous quad in CCW order), repeat until back to start. This traverses CCW around the vertex.

### Boundary Detection

```rust
topology.is_boundary_vertex(vi)  // Vertex in a ghost quad
topology.is_ghost_quad(qi)       // Quad contains ghost vertex
topology.edge_type(a, b)         // Interior | Boundary | NotAnEdge
```

### Filtering Ghost Elements

**When iterating vertices:**
```rust
for vi in topology.vertex_indices() {
    // Only real vertices, ghost vertex excluded
}
```

**When iterating quads:**
```rust
for qi in topology.quad_indices() {
    // Only real quads, ghost quads excluded
}

for qi in topology.ghost_quad_indices() {
    // Only ghost quads
}
```

**When processing neighbors:**
```rust
for qv in topology.vertex_ring(vi) {
    let neighbor = topology.vertex_index(qv.next());

    // Ghost vertex has no position - skip it
    if let Some(idx) = neighbor.try_into_index() {
        let pos = positions[idx];
        // Use position...
    }
}
```

Pattern: `try_into_index()` returns `None` for ghost vertex, naturally filtering it.

## Common Patterns

### Computing Neighbor Average (for Laplacian smoothing)

```rust
let mut sum = Vec2::ZERO;
let mut count = 0;

for qv in topology.vertex_ring(vi) {
    let neighbor = topology.vertex_index(qv.next());
    if let Some(idx) = neighbor.try_into_index() {
        sum += positions[idx];
        count += 1;
    }
}

let avg = if count > 0 { sum / count as f32 } else { positions[vi] };
```

### Checking All Edges of a Quad

```rust
for edge_idx in 0..4 {
    let qe = QuadEdge { quad: qi, edge: edge_idx };
    let (v0, v1) = topology.edge_vertices(qe);
    let neighbor = topology.quad_neighbor(qe);
    // Process edge...
}
```

### Boundary Iteration

```rust
for vi in topology.boundary_vertices() {
    // Iterate boundary in order (traverses ghost vertex ring)
}
```

## Mesh Filter Guidelines

When writing mesh filters (LaplacianSmoother, QuadRelax, etc.):

1. **Skip boundary vertices** in position updates:
   ```rust
   for vi in topology.vertex_indices() {
       if topology.is_boundary_vertex(vi) {
           continue;  // Don't move boundary
       }
       // Update position...
   }
   ```

2. **Filter ghost neighbors** when averaging:
   ```rust
   let neighbor = topology.vertex_index(qv.next());
   if let Some(idx) = neighbor.try_into_index() {
       // Real vertex - use it
   }
   ```

3. **Iterate only real quads** when checking quality:
   ```rust
   for qi in topology.quad_indices() {
       // topology.quad_indices() already filters ghosts
   }
   ```

## Error Types

```rust
QuadTopologyError::OddBoundary(len)
QuadTopologyError::BoundaryVertexOutOfRange { vertex, vertex_count }
QuadTopologyError::DuplicateBoundaryVertex(idx)
QuadTopologyError::QuadVertexOutOfRange { vertex, vertex_count }
QuadTopologyError::QuadReferencesGhost(idx)
QuadTopologyError::IncompleteTopology { quad, edge, vertices }
```

## Implementation Notes

- **Storage**: Ghost quads intermixed with real quads (no order constraint)
- **Detection**: Check if quad contains `topology.ghost_vertex()`, not by position
- **Positions**: Ghost vertex has no position - `positions` only has `vertex_count` entries
- **Neighbor adjacency**: Every edge has a neighbor (ghost quads close the topology)
- **Ring direction**: Vertex rings traverse CCW via `incoming_edge()` neighbor

## Where Things Are

- **Core types**: `crates/shine-game/src/math/mesh/quad_topology.rs`
- **Mesh struct**: `crates/shine-game/src/math/mesh/quad_mesh.rs`
- **Filters**: `crates/shine-game/src/math/mesh/filter/*.rs`

## Quick Debugging

If topology seems broken:
1. Check boundary has even length (`polygon.len() % 2 == 0`)
2. Verify ghost quads generated: `topology.ghost_quad_count() == boundary.len() / 2`
3. Check every edge has neighbor: no `QuadIdx::NONE` in `quad_neighbors`
4. Verify rings close: `vertex_ring(vi).count() > 0` for all vertices
