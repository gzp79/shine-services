import { hexVertices, isQuadConvex, jitterPoint, jitterBoundaryPoint } from './geometry.mjs';

// Re-export all smoothers from dedicated module
export { repulse, areaEqualize, lennardJones, springSmooth, repulse2, springSmooth2, equidistant, laplacian, angleEqualize, lloyd, energySmooth, cotangentSmooth } from './smooth.mjs';

export function createMesh() {
  const points = [];
  const quads = [];
  const boundaryEdges = new Map();

  function addPoint(x, y) {
    const idx = points.length / 2;
    points.push(x, y);
    return idx;
  }

  function addQuad(a, b, c, d) {
    quads.push([a, b, c, d]);
  }

  function edgeKey(a, b) {
    return a < b ? `${a},${b}` : `${b},${a}`;
  }

  function markBoundaryEdge(a, b, sx, sy, ex, ey) {
    boundaryEdges.set(edgeKey(a, b), { sx, sy, ex, ey });
  }

  function isBoundaryEdge(a, b) {
    return boundaryEdges.get(edgeKey(a, b));
  }

  return { points, quads, boundaryEdges, addPoint, addQuad, edgeKey, markBoundaryEdge, isBoundaryEdge };
}

export function initialSplit(radius, rng, { noJitter = false } = {}) {
  const mesh = createMesh();
  const verts = hexVertices(radius);
  const vi = verts.map(([x, y]) => mesh.addPoint(x, y));

  const maxOffset = noJitter ? 0 : radius * 0.1;
  const cx = maxOffset ? rng.nextInRange(-maxOffset, maxOffset) : 0;
  const cy = maxOffset ? rng.nextInRange(-maxOffset, maxOffset) : 0;
  const center = mesh.addPoint(cx, cy);

  const s = rng.nextInt(6);

  for (let i = 0; i < 3; i++) {
    const ai = (s + i * 2) % 6;
    const bi = (s + i * 2 + 1) % 6;
    const ci = (s + i * 2 + 2) % 6;
    mesh.addQuad(vi[ai], vi[bi], vi[ci], center);
    mesh.markBoundaryEdge(vi[ai], vi[bi], verts[ai][0], verts[ai][1], verts[bi][0], verts[bi][1]);
    mesh.markBoundaryEdge(vi[bi], vi[ci], verts[bi][0], verts[bi][1], verts[ci][0], verts[ci][1]);
  }

  return mesh;
}

/**
 * Alternative initial split: connect center to midpoints of hex edges.
 * Creates 6 quads (vertex, midpoint, center, prev-midpoint).
 * More uniform starting topology than initialSplit's 3-quad layout.
 */
export function initialSplitMid(radius, rng, { noJitter = false } = {}) {
  const mesh = createMesh();
  const verts = hexVertices(radius);
  const vi = verts.map(([x, y]) => mesh.addPoint(x, y));

  // Edge midpoints
  const mids = [];
  const mi = [];
  for (let i = 0; i < 6; i++) {
    const j = (i + 1) % 6;
    const mx = (verts[i][0] + verts[j][0]) / 2;
    const my = (verts[i][1] + verts[j][1]) / 2;
    mids.push([mx, my]);
    mi.push(mesh.addPoint(mx, my));
  }

  // Center (jittered or exact)
  const maxOffset = noJitter ? 0 : radius * 0.1;
  const cx = maxOffset ? rng.nextInRange(-maxOffset, maxOffset) : 0;
  const cy = maxOffset ? rng.nextInRange(-maxOffset, maxOffset) : 0;
  const center = mesh.addPoint(cx, cy);

  // 6 quads: (vertex[i], midpoint[i], center, midpoint[i-1])
  for (let i = 0; i < 6; i++) {
    const prev = (i + 5) % 6;
    mesh.addQuad(vi[i], mi[i], center, mi[prev]);
    // Boundary: vertex[i] → midpoint[i] (first half of hex edge i→i+1)
    mesh.markBoundaryEdge(vi[i], mi[i], verts[i][0], verts[i][1], mids[i][0], mids[i][1]);
    // Boundary: midpoint[prev] → vertex[i] (second half of hex edge prev→i)
    mesh.markBoundaryEdge(mi[prev], vi[i], mids[prev][0], mids[prev][1], verts[i][0], verts[i][1]);
  }

  return mesh;
}

/**
 * Alternative initial split: diagonal across the hex.
 * Connects two opposite vertices, creating 2 quads (half-hexes).
 * Produces bilateral flow instead of radial.
 * Random orientation picks which pair of opposite vertices to use.
 */
export function initialSplitDiag(radius, rng, opts) {
  const mesh = createMesh();
  const verts = hexVertices(radius);
  const vi = verts.map(([x, y]) => mesh.addPoint(x, y));

  // Pick a random pair of opposite vertices (3 choices: 0-3, 1-4, 2-5)
  const s = rng.nextInt(3);
  const a = s;       // 0, 1, or 2
  const b = s + 3;   // 3, 4, or 5

  // Quad 1: a → a+1 → a+2 → b  (3 edges + diagonal)
  mesh.addQuad(vi[a], vi[a + 1], vi[a + 2], vi[b]);
  // Quad 2: b → b+1 → (b+2)%6 → a  (3 edges + diagonal)
  mesh.addQuad(vi[b], vi[(b + 1) % 6], vi[(b + 2) % 6], vi[a]);

  // Mark all 6 hex edges as boundary
  for (let i = 0; i < 6; i++) {
    const j = (i + 1) % 6;
    mesh.markBoundaryEdge(vi[i], vi[j], verts[i][0], verts[i][1], verts[j][0], verts[j][1]);
  }

  return mesh;
}

export function subdivide(mesh, rng, radius, depth, { edgeOnly = false, edgeOnlyAfter = -1, noJitter = false } = {}) {
  const effectiveEdgeOnly = edgeOnly || depth > edgeOnlyAfter && edgeOnlyAfter >= 0;
  const maxJitter = noJitter ? 0 : radius * 0.15 / Math.pow(2, depth);
  const edgePointMap = new Map();
  const oldQuads = [...mesh.quads];
  mesh.quads.length = 0;

  const newBoundaryEdges = new Map();

  for (const quad of oldQuads) {
    const [c0, c1, c2, c3] = quad;

    // Face point: centroid + jitter
    let fx = 0, fy = 0;
    for (const pi of quad) {
      fx += mesh.points[pi * 2];
      fy += mesh.points[pi * 2 + 1];
    }
    fx /= 4;
    fy /= 4;
    const [jfx, jfy] = effectiveEdgeOnly ? [fx, fy] : jitterPoint(fx, fy, rng, maxJitter);
    const faceIdx = mesh.addPoint(jfx, jfy);

    // Edge points
    const edges = [[c0, c1], [c1, c2], [c2, c3], [c3, c0]];
    const edgeIndices = [];

    for (const [a, b] of edges) {
      const key = mesh.edgeKey(a, b);
      if (edgePointMap.has(key)) {
        const idx = edgePointMap.get(key);
        edgeIndices.push(idx);
        // Propagate boundary edges even on second encounter
        const boundary = mesh.isBoundaryEdge(a, b);
        if (boundary) {
          newBoundaryEdges.set(mesh.edgeKey(a, idx), { sx: boundary.sx, sy: boundary.sy, ex: boundary.ex, ey: boundary.ey });
          newBoundaryEdges.set(mesh.edgeKey(idx, b), { sx: boundary.sx, sy: boundary.sy, ex: boundary.ex, ey: boundary.ey });
        }
      } else {
        let mx = (mesh.points[a * 2] + mesh.points[b * 2]) / 2;
        let my = (mesh.points[a * 2 + 1] + mesh.points[b * 2 + 1]) / 2;

        const boundary = mesh.isBoundaryEdge(a, b);
        if (boundary) {
          // Boundary edges always subdivide uniformly (no jitter)
        } else if (effectiveEdgeOnly) {
          const esx = mesh.points[a * 2], esy = mesh.points[a * 2 + 1];
          const eex = mesh.points[b * 2], eey = mesh.points[b * 2 + 1];
          [mx, my] = jitterBoundaryPoint(mx, my, esx, esy, eex, eey, rng, maxJitter);
        } else {
          [mx, my] = jitterPoint(mx, my, rng, maxJitter);
        }

        const idx = mesh.addPoint(mx, my);
        edgePointMap.set(key, idx);

        if (boundary) {
          newBoundaryEdges.set(mesh.edgeKey(a, idx), { sx: boundary.sx, sy: boundary.sy, ex: boundary.ex, ey: boundary.ey });
          newBoundaryEdges.set(mesh.edgeKey(idx, b), { sx: boundary.sx, sy: boundary.sy, ex: boundary.ex, ey: boundary.ey });
        }

        edgeIndices.push(idx);
      }
    }

    const [e01, e12, e23, e30] = edgeIndices;

    const childQuads = [
      [c0, e01, faceIdx, e30],
      [c1, e12, faceIdx, e01],
      [c2, e23, faceIdx, e12],
      [c3, e30, faceIdx, e23],
    ];

    // Convexity enforcement for face point: halve jitter up to 3 times
    let allConvex = childQuads.every(q => isQuadConvex(mesh.points, q));
    if (!allConvex) {
      let dx = jfx - fx, dy = jfy - fy;
      for (let attempt = 0; attempt < 3 && !allConvex; attempt++) {
        dx /= 2;
        dy /= 2;
        mesh.points[faceIdx * 2] = fx + dx;
        mesh.points[faceIdx * 2 + 1] = fy + dy;
        allConvex = childQuads.every(q => isQuadConvex(mesh.points, q));
      }
      if (!allConvex) {
        mesh.points[faceIdx * 2] = fx;
        mesh.points[faceIdx * 2 + 1] = fy;
      }
    }

    for (const q of childQuads) {
      mesh.addQuad(...q);
    }
  }

  // Post-pass convexity enforcement for shared edge points.
  // Edge points affect quads from two parent quads, so convexity can't be
  // fully checked at creation time. For non-convex quads, iteratively move
  // interior (non-boundary) vertices toward the average of their neighbors.
  const boundaryVertexSet = new Set();
  for (const key of newBoundaryEdges.keys()) {
    const [a, b] = key.split(',').map(Number);
    boundaryVertexSet.add(a);
    boundaryVertexSet.add(b);
  }

  // Build neighbor map once
  const neighbors = new Map();
  for (const quad of mesh.quads) {
    for (let i = 0; i < 4; i++) {
      const pi = quad[i];
      if (!neighbors.has(pi)) neighbors.set(pi, new Set());
      neighbors.get(pi).add(quad[(i + 1) % 4]);
      neighbors.get(pi).add(quad[(i + 3) % 4]);
    }
  }

  for (let pass = 0; pass < 20; pass++) {
    const badVertices = new Set();
    let anyBad = false;
    for (const quad of mesh.quads) {
      if (!isQuadConvex(mesh.points, quad)) {
        anyBad = true;
        for (const pi of quad) {
          if (!boundaryVertexSet.has(pi)) badVertices.add(pi);
        }
      }
    }
    if (!anyBad) break;

    for (const pi of badVertices) {
      const nbrs = neighbors.get(pi);
      let ax = 0, ay = 0;
      for (const ni of nbrs) {
        ax += mesh.points[ni * 2];
        ay += mesh.points[ni * 2 + 1];
      }
      ax /= nbrs.size;
      ay /= nbrs.size;
      mesh.points[pi * 2] = mesh.points[pi * 2] * 0.7 + ax * 0.3;
      mesh.points[pi * 2 + 1] = mesh.points[pi * 2 + 1] * 0.7 + ay * 0.3;
    }
  }

  mesh.boundaryEdges.clear();
  for (const [key, val] of newBoundaryEdges) {
    mesh.boundaryEdges.set(key, val);
  }
}

