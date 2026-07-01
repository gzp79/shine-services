---
name: mesh-topology
description: Triangulation and Quadrangulation topology with ghost elements - use when working with Triangulation, Quadrangulation, mesh construction, topology queries, or any code in shine-game/src/math/{triangulation,quadrangulation}/
---

# Mesh Topology

Quick reference for Triangulation and Quadrangulation in shine-game/math.

## Ghost Topology Pattern

Both mesh types use **ghost elements** (infinite vertex + ghost faces/quads) to create topologically closed meshes:
- **Ghost vertex**: Special vertex at infinity with no position
- **Ghost faces/quads**: Connect boundary edges to ghost vertex
- **Result**: Complete rings around ALL vertices (no boundary special cases)

## Triangulation

### Core Types
```rust
Triangulation<DELAUNAY: bool>  // CDT (true) or CT (false)
- vertices: IdxVec<VertexIndex, Vertex { position: IVec2, triangle: FaceIndex }>
- faces: IdxVec<FaceIndex, Triangle { vertices: [3], neighbors: [3], constraints: [3] }>
- infinite_vertex: VertexIndex
```

**FaceEdge**: `{ triangle: FaceIndex, edge: Rot3Idx }` - references an edge
**FaceVertex**: `{ triangle: FaceIndex, vertex: Rot3Idx }` - references vertex position in triangle
**Rot3Idx**: Index with mod-3 arithmetic (`.increment()`, `.decrement()`)

### Navigation
```rust
tri.twin_edge(edge)                    // Opposite side of edge
tri.vi(VertexClue)                     // Resolve vertex reference
tri.p(VertexClue)                      // Get position
tri.is_infinite_face(fi)               // Ghost face check
tri.is_finite_vertex(vi)               // Real vertex check
tri.find_edge_by_vertex(a, b)          // Find edge connecting vertices
tri.edge_circulator(vi)                // Iterate edges around vertex
```

### Construction (via TriangulationBuilder)
```rust
let mut tri = Triangulation::<true>::new_cdt();
let mut builder = TriangulationBuilder::new(&mut tri);

builder.add_vertex(p, hint)                      // Insert point
builder.add_constraint_segment(p0, p1, c)        // Insert constrained edge
builder.add_constraint_edge(v0, v1, c)           // Constrain existing edge
builder.add_polygon(points, c)                   // Insert closed boundary
```

Maintains Delaunay property automatically. Use `hint: Option<FaceIndex>` for locality (last inserted vertex's face).

### Euler Operations (low-level, internal)
```rust
tri.split_edge(face, edge, p)   // Insert vertex on edge
tri.split_face(face, p)         // Insert vertex in face
tri.extend_dimension(p)         // Add first/second vertex
tri.flip_edge(edge)             // Flip shared edge (Delaunay)
```

## Quadrangulation

### Core Types
```rust
Quadrangulation
- vertices: IdxVec<VertexIndex, Vertex { position: Vec2, quad: QuadIndex }>
- quads: IdxVec<QuadIndex, Quad { vertices: [4], neighbors: [4] }>
- infinite_vertex: VertexIndex
- anchor_vertices: IdxVec<AnchorIndex, VertexIndex>  // Original boundary corners
```

**QuadEdge**: `{ quad: QuadIndex, edge: Rot4Idx }` - references an edge
**QuadVertex**: `{ quad: QuadIndex, local: Rot4Idx }` - references vertex position in quad
**Rot4Idx**: Index with mod-4 arithmetic

### Navigation
```rust
// Pure index arithmetic (no topology access)
qv.next() / qv.prev()           // Adjacent vertex in quad
qv.opposite()                   // Diagonal vertex
qv.outgoing_edge()              // Edge leaving vertex
qv.incoming_edge()              // Edge entering vertex

// Topology queries (require &Quadrangulation)
quad.vi(VertexClue)             // Resolve vertex reference
quad.p(VertexClue) / p_mut()    // Get/set position
quad.qi(QuadClue)               // Resolve quad reference
quad.edge_twin(qe)              // Neighboring edge
quad.edge_vertices(qe)          // (start, end) vertices
quad.is_infinite_quad(qi)       // Ghost quad check
quad.edge_type(a, b)            // Interior | Boundary | NotAnEdge
```

### Vertex Ring Traversal
```rust
for qv in quad.vertex_ring_ccw(vi) {
    let next_vi = quad.vi(qv.next());
    // Process quad around vertex
}

quad.vertex_ring_cw(vi)                // Clockwise
quad.adjacent_vertices(vi)             // Just the neighbor vertices
quad.average_adjacent_positions(vi)    // Weighted average (excludes ghost)
```

**Implementation**: Starts at `vertices[vi].quad`, yields quad, follows `incoming_edge().twin()` to previous quad CCW.

### Boundary Handling
```rust
quad.is_boundary_vertex(vi)         // In any ghost quad
quad.boundary_vertices()            // Iterator in CCW order
quad.boundary_edges()               // [[u32; 2]] pairs
quad.anchor_count()                 // Original boundary corners
quad.anchor_edge(anchor_idx)        // Vertices along subdivided edge
```

### Construction (internal patterns)
Built via specialized builders (`from_patch`). Public API rarely used directly.

## Common Patterns

### Filtering Ghost Elements
```rust
// Triangulation
for vi in tri.vertex_index_iter() {
    if tri.is_finite_vertex(vi) { /* ... */ }
}
for fi in tri.face_index_iter() {
    if tri.is_finite_face(fi) { /* ... */ }
}

// Quadrangulation
for vi in quad.finite_vertex_index_iter() { /* ... */ }
for qi in quad.finite_quad_index_iter() { /* ... */ }
```

### Processing Neighbors (Skip Ghost)
```rust
// Quadrangulation example
for qv in quad.vertex_ring_ccw(vi) {
    let neighbor = quad.vi(qv.next());
    if quad.is_finite_vertex(neighbor) {
        let pos = quad.p(neighbor);
        // Use position
    }
}
```

### Mesh Filters (Quadrangulation)
```rust
for vi in quad.finite_vertex_index_iter() {
    if quad.is_boundary_vertex(vi) {
        continue;  // Don't move boundary
    }
    
    let avg = quad.average_adjacent_positions(vi);
    *quad.p_mut(vi) = /* update from avg */;
}
```

## Key Differences

| | Triangulation | Quadrangulation |
|---|---|---|
| Position type | `IVec2` | `Vec2` |
| Rot index | `Rot3Idx` | `Rot4Idx` |
| Face/Quad | 3 vertices/edges | 4 vertices/edges |
| Constraints | Per-edge `u32` flags | None |
| Delaunay | Optional (DELAUNAY param) | N/A |
| Builder | `TriangulationBuilder` | Internal only |

## Validation

```rust
// Triangulation
tri.validator().validate()?;
tri.validator().validate_topology()?;
tri.validator().validate_geometry()?;
tri.validator().validate_delaunay()?;

// Quadrangulation
quad.validator().validate()?;
```

## Where Things Are

- `shine-game/src/math/triangulation/` - Triangulation types, builder, queries, validation
- `shine-game/src/math/quadrangulation/` - Quadrangulation types, filters, queries, validation
- `shine-game/src/indexed/` - TypedIndex, RotNIdx, IdxVec/IdxArray
