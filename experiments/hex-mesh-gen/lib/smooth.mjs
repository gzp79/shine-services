import { isQuadConvex, isInsideHex } from './geometry.mjs';

/**
 * Analyze boundary edges to classify vertices as:
 * - fixed (any vertex on a boundary edge) — never move during smoothing
 * - interior (not on any boundary edge) — free to move in 2D
 */
function getBoundaryInfo(mesh) {
  const fixedSet = new Set();

  for (const key of mesh.boundaryEdges.keys()) {
    const [a, b] = key.split(',').map(Number);
    fixedSet.add(a);
    fixedSet.add(b);
  }

  const slideMap = new Map();
  return { fixedSet, slideMap };
}

/**
 * Constrain a slidable boundary vertex to its hex edge segment.
 * Projects the vertex position onto the edge direction and clamps to [0,1].
 */
function constrainToEdge(mesh, i, edgeInfo) {
  const { sx, sy, dx, dy, len } = edgeInfo;
  const px = mesh.points[i * 2] - sx;
  const py = mesh.points[i * 2 + 1] - sy;
  let t = (px * dx + py * dy) / (len * len);
  t = Math.max(0, Math.min(1, t));
  mesh.points[i * 2] = sx + dx * t;
  mesh.points[i * 2 + 1] = sy + dy * t;
}

/**
 * Constrain vertex i after movement: slide along edge if boundary, revert if outside hex.
 * Returns true if vertex was reverted (not just constrained).
 */
function constrainVertex(mesh, i, radius, slideMap, savedX, savedY) {
  const edge = slideMap.get(i);
  if (edge) {
    constrainToEdge(mesh, i, edge);
    return false;
  }
  if (!isInsideHex(mesh.points[i * 2], mesh.points[i * 2 + 1], radius)) {
    mesh.points[i * 2] = savedX;
    mesh.points[i * 2 + 1] = savedY;
    return true;
  }
  return false;
}

/**
 * Post-process: apply repulsive forces between movable vertices.
 * Each vertex pushes nearby vertices away (inverse-distance repulsion).
 * Corner vertices are fixed. Boundary vertices slide along their hex edge.
 */
export function repulse(mesh, radius, iterations = 10, strength = 0.02) {
  const numPoints = mesh.points.length / 2;
  const { fixedSet, slideMap } = getBoundaryInfo(mesh);

  const movable = [];
  for (let i = 0; i < numPoints; i++) {
    if (!fixedSet.has(i)) movable.push(i);
  }

  for (let iter = 0; iter < iterations; iter++) {
    const forces = new Map();
    for (const i of movable) {
      forces.set(i, { fx: 0, fy: 0 });
    }

    for (let a = 0; a < movable.length; a++) {
      const i = movable[a];
      const ix = mesh.points[i * 2];
      const iy = mesh.points[i * 2 + 1];

      for (let b = a + 1; b < movable.length; b++) {
        const j = movable[b];
        const dx = ix - mesh.points[j * 2];
        const dy = iy - mesh.points[j * 2 + 1];
        const distSq = dx * dx + dy * dy;
        if (distSq < 1e-12) continue;

        const dist = Math.sqrt(distSq);
        const force = strength / distSq;
        const nx = dx / dist;
        const ny = dy / dist;

        forces.get(i).fx += nx * force;
        forces.get(i).fy += ny * force;
        forces.get(j).fx -= nx * force;
        forces.get(j).fy -= ny * force;
      }

      // Fixed corners also repel movable vertices
      for (const j of fixedSet) {
        const dx = ix - mesh.points[j * 2];
        const dy = iy - mesh.points[j * 2 + 1];
        const distSq = dx * dx + dy * dy;
        if (distSq < 1e-12) continue;

        const dist = Math.sqrt(distSq);
        const force = strength / distSq;
        forces.get(i).fx += (dx / dist) * force;
        forces.get(i).fy += (dy / dist) * force;
      }
    }

    const maxDisp = radius * 0.05;
    const saved = new Map();
    for (const i of movable) {
      saved.set(i, [mesh.points[i * 2], mesh.points[i * 2 + 1]]);

      let { fx, fy } = forces.get(i);
      const mag = Math.sqrt(fx * fx + fy * fy);
      if (mag > maxDisp) {
        fx = (fx / mag) * maxDisp;
        fy = (fy / mag) * maxDisp;
      }
      mesh.points[i * 2] += fx;
      mesh.points[i * 2 + 1] += fy;

      const [sx, sy] = saved.get(i);
      constrainVertex(mesh, i, radius, slideMap, sx, sy);
    }

    for (const quad of mesh.quads) {
      if (isQuadConvex(mesh.points, quad)) continue;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        if (!saved.has(pi)) continue;
        const [sx, sy] = saved.get(pi);
        mesh.points[pi * 2] = sx;
        mesh.points[pi * 2 + 1] = sy;
      }
    }
  }
}

/**
 * Post-process: area-equalizing pressure.
 * Computes each quad's area relative to the average. Vertices of too-small
 * quads are pushed outward from the quad centroid; too-large quads pull inward.
 * Corner vertices are fixed. Boundary vertices slide along their hex edge.
 */
export function areaEqualize(mesh, radius, iterations = 20, strength = 0.05) {
  const numPoints = mesh.points.length / 2;
  const { fixedSet, slideMap } = getBoundaryInfo(mesh);

  const quadCount = new Float64Array(numPoints);
  for (const quad of mesh.quads) {
    for (const pi of quad) quadCount[pi]++;
  }

  for (let iter = 0; iter < iterations; iter++) {
    const areas = mesh.quads.map(quad => {
      let area = 0;
      for (let i = 0; i < 4; i++) {
        const a = quad[i], b = quad[(i + 1) % 4];
        area += mesh.points[a * 2] * mesh.points[b * 2 + 1];
        area -= mesh.points[b * 2] * mesh.points[a * 2 + 1];
      }
      return Math.abs(area) / 2;
    });

    const avgArea = areas.reduce((s, a) => s + a, 0) / areas.length;
    if (avgArea < 1e-12) break;

    const forces = new Float64Array(numPoints * 2);

    for (let qi = 0; qi < mesh.quads.length; qi++) {
      const quad = mesh.quads[qi];
      const raw = (areas[qi] - avgArea) / avgArea;
      const ratio = Math.max(-1, Math.min(1, raw));

      let cx = 0, cy = 0;
      for (const pi of quad) {
        cx += mesh.points[pi * 2];
        cy += mesh.points[pi * 2 + 1];
      }
      cx /= 4;
      cy /= 4;

      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        const dx = cx - mesh.points[pi * 2];
        const dy = cy - mesh.points[pi * 2 + 1];
        forces[pi * 2] += strength * ratio * dx;
        forces[pi * 2 + 1] += strength * ratio * dy;
      }
    }

    for (let i = 0; i < numPoints; i++) {
      if (quadCount[i] > 1) {
        forces[i * 2] /= quadCount[i];
        forces[i * 2 + 1] /= quadCount[i];
      }
    }

    const maxDisp = radius * 0.03;
    const saved = new Float64Array(numPoints * 2);
    for (let i = 0; i < numPoints; i++) {
      if (fixedSet.has(i)) continue;
      saved[i * 2] = mesh.points[i * 2];
      saved[i * 2 + 1] = mesh.points[i * 2 + 1];

      let fx = forces[i * 2];
      let fy = forces[i * 2 + 1];
      const mag = Math.sqrt(fx * fx + fy * fy);
      if (mag > maxDisp) {
        fx = (fx / mag) * maxDisp;
        fy = (fy / mag) * maxDisp;
      }
      mesh.points[i * 2] += fx;
      mesh.points[i * 2 + 1] += fy;

      constrainVertex(mesh, i, radius, slideMap, saved[i * 2], saved[i * 2 + 1]);
    }

    for (const quad of mesh.quads) {
      if (isQuadConvex(mesh.points, quad)) continue;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        mesh.points[pi * 2] = saved[pi * 2];
        mesh.points[pi * 2 + 1] = saved[pi * 2 + 1];
      }
    }
  }
}

/**
 * Post-process: Lennard-Jones-style smoothing.
 * All movable vertex pairs interact: repel at short range, attract at long range.
 * Equilibrium distance is derived from average vertex spacing.
 * Corner vertices are fixed. Boundary vertices slide along their hex edge.
 */
export function lennardJones(mesh, radius, iterations = 15, strength = 0.02) {
  const numPoints = mesh.points.length / 2;
  const { fixedSet, slideMap } = getBoundaryInfo(mesh);

  const movable = [];
  for (let i = 0; i < numPoints; i++) {
    if (!fixedSet.has(i)) movable.push(i);
  }

  const edgeSet = new Set();
  let totalLen = 0, edgeCount = 0;
  for (const quad of mesh.quads) {
    for (let i = 0; i < 4; i++) {
      const a = quad[i], b = quad[(i + 1) % 4];
      const key = a < b ? `${a},${b}` : `${b},${a}`;
      if (edgeSet.has(key)) continue;
      edgeSet.add(key);
      const dx = mesh.points[a * 2] - mesh.points[b * 2];
      const dy = mesh.points[a * 2 + 1] - mesh.points[b * 2 + 1];
      totalLen += Math.sqrt(dx * dx + dy * dy);
      edgeCount++;
    }
  }
  const sigma = totalLen / edgeCount;
  const cutoff = sigma * 2;
  const cutoffSq = cutoff * cutoff;

  function ljForce(dx, dy, distSq) {
    const dist = Math.sqrt(distSq);
    const r = dist / sigma;
    const f = strength * (1 / (r * r) - 1 / r);
    return [f * dx / dist, f * dy / dist];
  }

  for (let iter = 0; iter < iterations; iter++) {
    const forces = new Float64Array(numPoints * 2);

    for (let a = 0; a < movable.length; a++) {
      const i = movable[a];
      const ix = mesh.points[i * 2];
      const iy = mesh.points[i * 2 + 1];

      // Movable-movable pairs
      for (let b = a + 1; b < movable.length; b++) {
        const j = movable[b];
        const dx = ix - mesh.points[j * 2];
        const dy = iy - mesh.points[j * 2 + 1];
        const distSq = dx * dx + dy * dy;
        if (distSq < 1e-12 || distSq > cutoffSq) continue;

        const [fx, fy] = ljForce(dx, dy, distSq);
        forces[i * 2] += fx;
        forces[i * 2 + 1] += fy;
        forces[j * 2] -= fx;
        forces[j * 2 + 1] -= fy;
      }

      // Fixed corners push/pull but don't move
      for (const j of fixedSet) {
        const dx = ix - mesh.points[j * 2];
        const dy = iy - mesh.points[j * 2 + 1];
        const distSq = dx * dx + dy * dy;
        if (distSq < 1e-12 || distSq > cutoffSq) continue;

        const [fx, fy] = ljForce(dx, dy, distSq);
        forces[i * 2] += fx;
        forces[i * 2 + 1] += fy;
      }
    }

    const maxDisp = radius * 0.04;
    const saved = new Float64Array(numPoints * 2);
    for (const i of movable) {
      saved[i * 2] = mesh.points[i * 2];
      saved[i * 2 + 1] = mesh.points[i * 2 + 1];

      let fx = forces[i * 2];
      let fy = forces[i * 2 + 1];
      const mag = Math.sqrt(fx * fx + fy * fy);
      if (mag > maxDisp) {
        fx = (fx / mag) * maxDisp;
        fy = (fy / mag) * maxDisp;
      }
      mesh.points[i * 2] += fx;
      mesh.points[i * 2 + 1] += fy;

      constrainVertex(mesh, i, radius, slideMap, saved[i * 2], saved[i * 2 + 1]);
    }

    for (const quad of mesh.quads) {
      if (isQuadConvex(mesh.points, quad)) continue;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        mesh.points[pi * 2] = saved[pi * 2];
        mesh.points[pi * 2 + 1] = saved[pi * 2 + 1];
      }
    }
  }
}

/**
 * Post-process: spring smoothing along mesh edges.
 * Each edge acts as a spring pulling its two vertices toward a rest length.
 * Rest length is the average edge length across the whole mesh.
 * Corner vertices are fixed. Boundary vertices slide along their hex edge.
 */
export function springSmooth(mesh, radius, iterations = 15, stiffness = 0.3) {
  const numPoints = mesh.points.length / 2;
  const { fixedSet, slideMap } = getBoundaryInfo(mesh);

  const edgeSet = new Set();
  const edges = [];
  for (const quad of mesh.quads) {
    for (let i = 0; i < 4; i++) {
      const a = quad[i];
      const b = quad[(i + 1) % 4];
      const key = a < b ? `${a},${b}` : `${b},${a}`;
      if (!edgeSet.has(key)) {
        edgeSet.add(key);
        edges.push([a, b]);
      }
    }
  }

  let totalLen = 0;
  for (const [a, b] of edges) {
    const dx = mesh.points[a * 2] - mesh.points[b * 2];
    const dy = mesh.points[a * 2 + 1] - mesh.points[b * 2 + 1];
    totalLen += Math.sqrt(dx * dx + dy * dy);
  }
  const restLength = totalLen / edges.length;

  for (let iter = 0; iter < iterations; iter++) {
    const forces = new Float64Array(numPoints * 2);

    for (const [a, b] of edges) {
      const ax = mesh.points[a * 2], ay = mesh.points[a * 2 + 1];
      const bx = mesh.points[b * 2], by = mesh.points[b * 2 + 1];
      const dx = bx - ax;
      const dy = by - ay;
      const dist = Math.sqrt(dx * dx + dy * dy);
      if (dist < 1e-12) continue;

      const displacement = dist - restLength;
      const fx = stiffness * displacement * (dx / dist);
      const fy = stiffness * displacement * (dy / dist);

      if (!fixedSet.has(a)) {
        forces[a * 2] += fx;
        forces[a * 2 + 1] += fy;
      }
      if (!fixedSet.has(b)) {
        forces[b * 2] -= fx;
        forces[b * 2 + 1] -= fy;
      }
    }

    const maxDisp = radius * 0.05;
    const saved = new Float64Array(numPoints * 2);
    for (let i = 0; i < numPoints; i++) {
      if (fixedSet.has(i)) continue;
      saved[i * 2] = mesh.points[i * 2];
      saved[i * 2 + 1] = mesh.points[i * 2 + 1];

      let fx = forces[i * 2];
      let fy = forces[i * 2 + 1];
      const mag = Math.sqrt(fx * fx + fy * fy);
      if (mag > maxDisp) {
        fx = (fx / mag) * maxDisp;
        fy = (fy / mag) * maxDisp;
      }
      mesh.points[i * 2] += fx;
      mesh.points[i * 2 + 1] += fy;

      constrainVertex(mesh, i, radius, slideMap, saved[i * 2], saved[i * 2 + 1]);
    }

    for (const quad of mesh.quads) {
      if (isQuadConvex(mesh.points, quad)) continue;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        mesh.points[pi * 2] = saved[pi * 2];
        mesh.points[pi * 2 + 1] = saved[pi * 2 + 1];
      }
    }
  }
}

/**
 * Post-process: repulsion with boundary awareness.
 * Like repulse(), but fixed corner vertices also repel movable vertices,
 * preventing quads near the hex edge from collapsing.
 * Boundary vertices slide along their hex edge.
 */
export function repulse2(mesh, radius, iterations = 10, strength = 0.02) {
  const numPoints = mesh.points.length / 2;
  const { fixedSet, slideMap } = getBoundaryInfo(mesh);

  const movable = [];
  for (let i = 0; i < numPoints; i++) {
    if (!fixedSet.has(i)) movable.push(i);
  }

  for (let iter = 0; iter < iterations; iter++) {
    const forces = new Map();
    for (const i of movable) {
      forces.set(i, { fx: 0, fy: 0 });
    }

    for (let a = 0; a < movable.length; a++) {
      const i = movable[a];
      const ix = mesh.points[i * 2];
      const iy = mesh.points[i * 2 + 1];

      // Movable-movable repulsion
      for (let b = a + 1; b < movable.length; b++) {
        const j = movable[b];
        const dx = ix - mesh.points[j * 2];
        const dy = iy - mesh.points[j * 2 + 1];
        const distSq = dx * dx + dy * dy;
        if (distSq < 1e-12) continue;

        const dist = Math.sqrt(distSq);
        const force = strength / distSq;
        const nx = dx / dist;
        const ny = dy / dist;

        forces.get(i).fx += nx * force;
        forces.get(i).fy += ny * force;
        forces.get(j).fx -= nx * force;
        forces.get(j).fy -= ny * force;
      }

      // Fixed corners repel movable vertices
      for (const j of fixedSet) {
        const dx = ix - mesh.points[j * 2];
        const dy = iy - mesh.points[j * 2 + 1];
        const distSq = dx * dx + dy * dy;
        if (distSq < 1e-12) continue;

        const dist = Math.sqrt(distSq);
        const force = strength / distSq;
        forces.get(i).fx += (dx / dist) * force;
        forces.get(i).fy += (dy / dist) * force;
      }
    }

    const maxDisp = radius * 0.05;
    const saved = new Map();
    for (const i of movable) {
      saved.set(i, [mesh.points[i * 2], mesh.points[i * 2 + 1]]);

      let { fx, fy } = forces.get(i);
      const mag = Math.sqrt(fx * fx + fy * fy);
      if (mag > maxDisp) {
        fx = (fx / mag) * maxDisp;
        fy = (fy / mag) * maxDisp;
      }
      mesh.points[i * 2] += fx;
      mesh.points[i * 2 + 1] += fy;

      const [sx, sy] = saved.get(i);
      constrainVertex(mesh, i, radius, slideMap, sx, sy);
    }

    for (const quad of mesh.quads) {
      if (isQuadConvex(mesh.points, quad)) continue;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        if (!saved.has(pi)) continue;
        const [sx, sy] = saved.get(pi);
        mesh.points[pi * 2] = sx;
        mesh.points[pi * 2 + 1] = sy;
      }
    }
  }
}

/**
 * Post-process: spring smoothing excluding fixed corner edges.
 * Like springSmooth(), but only operates on edges where neither vertex is a
 * fixed corner. Boundary vertices slide along their hex edge.
 */
export function springSmooth2(mesh, radius, iterations = 15, stiffness = 0.3) {
  const numPoints = mesh.points.length / 2;
  const { fixedSet, slideMap } = getBoundaryInfo(mesh);

  const edgeSet = new Set();
  const edges = [];
  for (const quad of mesh.quads) {
    for (let i = 0; i < 4; i++) {
      const a = quad[i];
      const b = quad[(i + 1) % 4];
      if (fixedSet.has(a) || fixedSet.has(b)) continue;
      const key = a < b ? `${a},${b}` : `${b},${a}`;
      if (!edgeSet.has(key)) {
        edgeSet.add(key);
        edges.push([a, b]);
      }
    }
  }

  if (edges.length === 0) return;

  let totalLen = 0;
  for (const [a, b] of edges) {
    const dx = mesh.points[a * 2] - mesh.points[b * 2];
    const dy = mesh.points[a * 2 + 1] - mesh.points[b * 2 + 1];
    totalLen += Math.sqrt(dx * dx + dy * dy);
  }
  const restLength = totalLen / edges.length;

  for (let iter = 0; iter < iterations; iter++) {
    const forces = new Float64Array(numPoints * 2);

    for (const [a, b] of edges) {
      const ax = mesh.points[a * 2], ay = mesh.points[a * 2 + 1];
      const bx = mesh.points[b * 2], by = mesh.points[b * 2 + 1];
      const dx = bx - ax;
      const dy = by - ay;
      const dist = Math.sqrt(dx * dx + dy * dy);
      if (dist < 1e-12) continue;

      const displacement = dist - restLength;
      const fx = stiffness * displacement * (dx / dist);
      const fy = stiffness * displacement * (dy / dist);

      forces[a * 2] += fx;
      forces[a * 2 + 1] += fy;
      forces[b * 2] -= fx;
      forces[b * 2 + 1] -= fy;
    }

    const maxDisp = radius * 0.05;
    const saved = new Float64Array(numPoints * 2);
    for (let i = 0; i < numPoints; i++) {
      if (fixedSet.has(i)) continue;
      saved[i * 2] = mesh.points[i * 2];
      saved[i * 2 + 1] = mesh.points[i * 2 + 1];

      let fx = forces[i * 2];
      let fy = forces[i * 2 + 1];
      const mag = Math.sqrt(fx * fx + fy * fy);
      if (mag > maxDisp) {
        fx = (fx / mag) * maxDisp;
        fy = (fy / mag) * maxDisp;
      }
      mesh.points[i * 2] += fx;
      mesh.points[i * 2 + 1] += fy;

      constrainVertex(mesh, i, radius, slideMap, saved[i * 2], saved[i * 2 + 1]);
    }

    for (const quad of mesh.quads) {
      if (isQuadConvex(mesh.points, quad)) continue;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        mesh.points[pi * 2] = saved[pi * 2];
        mesh.points[pi * 2 + 1] = saved[pi * 2 + 1];
      }
    }
  }
}

/**
 * Post-process: equidistant relaxation.
 * Each movable vertex tries to become equidistant from all its topological
 * neighbors. Uses per-vertex local rest length (average of its own edge lengths).
 * Corner vertices are fixed. Boundary vertices slide along their hex edge.
 */
export function equidistant(mesh, radius, iterations = 20, stiffness = 0.25) {
  const numPoints = mesh.points.length / 2;
  const { fixedSet, slideMap } = getBoundaryInfo(mesh);

  const neighbors = new Map();
  for (const quad of mesh.quads) {
    for (let i = 0; i < 4; i++) {
      const a = quad[i];
      const b = quad[(i + 1) % 4];
      if (!neighbors.has(a)) neighbors.set(a, new Set());
      if (!neighbors.has(b)) neighbors.set(b, new Set());
      neighbors.get(a).add(b);
      neighbors.get(b).add(a);
    }
  }

  const restLengths = new Float64Array(numPoints);
  for (let i = 0; i < numPoints; i++) {
    if (fixedSet.has(i)) continue;
    const nbrs = neighbors.get(i);
    if (!nbrs || nbrs.size === 0) continue;
    const ix = mesh.points[i * 2];
    const iy = mesh.points[i * 2 + 1];
    let totalDist = 0;
    for (const j of nbrs) {
      const dx = mesh.points[j * 2] - ix;
      const dy = mesh.points[j * 2 + 1] - iy;
      totalDist += Math.sqrt(dx * dx + dy * dy);
    }
    restLengths[i] = totalDist / nbrs.size;
  }

  for (let iter = 0; iter < iterations; iter++) {
    const forces = new Float64Array(numPoints * 2);

    for (let i = 0; i < numPoints; i++) {
      if (fixedSet.has(i)) continue;
      const nbrs = neighbors.get(i);
      if (!nbrs || nbrs.size === 0) continue;

      const ix = mesh.points[i * 2];
      const iy = mesh.points[i * 2 + 1];
      const rest = restLengths[i];

      for (const j of nbrs) {
        const dx = mesh.points[j * 2] - ix;
        const dy = mesh.points[j * 2 + 1] - iy;
        const dist = Math.sqrt(dx * dx + dy * dy);
        if (dist < 1e-12) continue;

        const displacement = dist - rest;
        forces[i * 2] += stiffness * displacement * (dx / dist);
        forces[i * 2 + 1] += stiffness * displacement * (dy / dist);
      }
    }

    const maxDisp = radius * 0.04;
    const saved = new Float64Array(numPoints * 2);
    for (let i = 0; i < numPoints; i++) {
      if (fixedSet.has(i)) continue;
      saved[i * 2] = mesh.points[i * 2];
      saved[i * 2 + 1] = mesh.points[i * 2 + 1];

      let fx = forces[i * 2];
      let fy = forces[i * 2 + 1];
      const mag = Math.sqrt(fx * fx + fy * fy);
      if (mag > maxDisp) {
        fx = (fx / mag) * maxDisp;
        fy = (fy / mag) * maxDisp;
      }
      mesh.points[i * 2] += fx;
      mesh.points[i * 2 + 1] += fy;

      constrainVertex(mesh, i, radius, slideMap, saved[i * 2], saved[i * 2 + 1]);
    }

    for (const quad of mesh.quads) {
      if (isQuadConvex(mesh.points, quad)) continue;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        mesh.points[pi * 2] = saved[pi * 2];
        mesh.points[pi * 2 + 1] = saved[pi * 2 + 1];
      }
    }
  }
}

/**
 * Post-process: Laplacian smoothing.
 * Each movable vertex moves toward the centroid of its topological neighbors.
 * Corner vertices are fixed. Boundary vertices slide along their hex edge.
 */
export function laplacian(mesh, radius, iterations = 20, strength = 0.3) {
  const numPoints = mesh.points.length / 2;
  const { fixedSet, slideMap } = getBoundaryInfo(mesh);

  const neighbors = new Map();
  for (const quad of mesh.quads) {
    for (let i = 0; i < 4; i++) {
      const a = quad[i], b = quad[(i + 1) % 4];
      if (!neighbors.has(a)) neighbors.set(a, new Set());
      if (!neighbors.has(b)) neighbors.set(b, new Set());
      neighbors.get(a).add(b);
      neighbors.get(b).add(a);
    }
  }

  for (let iter = 0; iter < iterations; iter++) {
    const saved = new Float64Array(numPoints * 2);
    for (let i = 0; i < numPoints; i++) {
      if (fixedSet.has(i)) continue;
      const nbrs = neighbors.get(i);
      if (!nbrs || nbrs.size === 0) continue;

      saved[i * 2] = mesh.points[i * 2];
      saved[i * 2 + 1] = mesh.points[i * 2 + 1];

      let cx = 0, cy = 0;
      for (const j of nbrs) {
        cx += mesh.points[j * 2];
        cy += mesh.points[j * 2 + 1];
      }
      cx /= nbrs.size;
      cy /= nbrs.size;

      mesh.points[i * 2] += strength * (cx - mesh.points[i * 2]);
      mesh.points[i * 2 + 1] += strength * (cy - mesh.points[i * 2 + 1]);

      constrainVertex(mesh, i, radius, slideMap, saved[i * 2], saved[i * 2 + 1]);
    }

    for (const quad of mesh.quads) {
      if (isQuadConvex(mesh.points, quad)) continue;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        mesh.points[pi * 2] = saved[pi * 2];
        mesh.points[pi * 2 + 1] = saved[pi * 2 + 1];
      }
    }
  }
}

/**
 * Post-process: angle equalizer.
 * Pushes quad interior angles toward 90 degrees.
 * Corner vertices are fixed. Boundary vertices slide along their hex edge.
 */
export function angleEqualize(mesh, radius, iterations = 20, strength = 0.15) {
  const numPoints = mesh.points.length / 2;
  const { fixedSet, slideMap } = getBoundaryInfo(mesh);

  const TARGET = Math.PI / 2;

  for (let iter = 0; iter < iterations; iter++) {
    const forces = new Float64Array(numPoints * 2);

    for (const quad of mesh.quads) {
      for (let i = 0; i < 4; i++) {
        const prev = quad[(i + 3) % 4];
        const curr = quad[i];
        const next = quad[(i + 1) % 4];

        if (fixedSet.has(curr)) continue;

        const px = mesh.points[prev * 2] - mesh.points[curr * 2];
        const py = mesh.points[prev * 2 + 1] - mesh.points[curr * 2 + 1];
        const nx = mesh.points[next * 2] - mesh.points[curr * 2];
        const ny = mesh.points[next * 2 + 1] - mesh.points[curr * 2 + 1];

        const pLen = Math.sqrt(px * px + py * py);
        const nLen = Math.sqrt(nx * nx + ny * ny);
        if (pLen < 1e-12 || nLen < 1e-12) continue;

        const dot = (px * nx + py * ny) / (pLen * nLen);
        const angle = Math.acos(Math.max(-1, Math.min(1, dot)));
        const error = angle - TARGET;

        const bx = px / pLen + nx / nLen;
        const by = py / pLen + ny / nLen;
        const bLen = Math.sqrt(bx * bx + by * by);
        if (bLen < 1e-12) continue;

        const f = strength * error;
        forces[curr * 2] += f * (bx / bLen);
        forces[curr * 2 + 1] += f * (by / bLen);
      }
    }

    const maxDisp = radius * 0.04;
    const saved = new Float64Array(numPoints * 2);
    for (let i = 0; i < numPoints; i++) {
      if (fixedSet.has(i)) continue;
      saved[i * 2] = mesh.points[i * 2];
      saved[i * 2 + 1] = mesh.points[i * 2 + 1];

      let fx = forces[i * 2];
      let fy = forces[i * 2 + 1];
      const mag = Math.sqrt(fx * fx + fy * fy);
      if (mag > maxDisp) {
        fx = (fx / mag) * maxDisp;
        fy = (fy / mag) * maxDisp;
      }
      mesh.points[i * 2] += fx;
      mesh.points[i * 2 + 1] += fy;

      constrainVertex(mesh, i, radius, slideMap, saved[i * 2], saved[i * 2 + 1]);
    }

    for (const quad of mesh.quads) {
      if (isQuadConvex(mesh.points, quad)) continue;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        mesh.points[pi * 2] = saved[pi * 2];
        mesh.points[pi * 2 + 1] = saved[pi * 2 + 1];
      }
    }
  }
}

/**
 * Post-process: Lloyd relaxation.
 * Each movable vertex moves to the area-weighted centroid of its adjacent quads.
 * Corner vertices are fixed. Boundary vertices slide along their hex edge.
 */
export function lloyd(mesh, radius, iterations = 20, strength = 0.4) {
  const numPoints = mesh.points.length / 2;
  const { fixedSet, slideMap } = getBoundaryInfo(mesh);

  const vertQuads = new Map();
  for (let qi = 0; qi < mesh.quads.length; qi++) {
    for (const pi of mesh.quads[qi]) {
      if (!vertQuads.has(pi)) vertQuads.set(pi, []);
      vertQuads.get(pi).push(qi);
    }
  }

  for (let iter = 0; iter < iterations; iter++) {
    const saved = new Float64Array(numPoints * 2);

    const qAreas = new Float64Array(mesh.quads.length);
    const qCentroids = new Float64Array(mesh.quads.length * 2);
    for (let qi = 0; qi < mesh.quads.length; qi++) {
      const quad = mesh.quads[qi];
      let area = 0, cx = 0, cy = 0;
      for (let i = 0; i < 4; i++) {
        const a = quad[i], b = quad[(i + 1) % 4];
        area += mesh.points[a * 2] * mesh.points[b * 2 + 1];
        area -= mesh.points[b * 2] * mesh.points[a * 2 + 1];
        cx += mesh.points[quad[i] * 2];
        cy += mesh.points[quad[i] * 2 + 1];
      }
      qAreas[qi] = Math.abs(area) / 2;
      qCentroids[qi * 2] = cx / 4;
      qCentroids[qi * 2 + 1] = cy / 4;
    }

    for (let i = 0; i < numPoints; i++) {
      if (fixedSet.has(i)) continue;
      const qis = vertQuads.get(i);
      if (!qis || qis.length === 0) continue;

      saved[i * 2] = mesh.points[i * 2];
      saved[i * 2 + 1] = mesh.points[i * 2 + 1];

      let wx = 0, wy = 0, wTotal = 0;
      for (const qi of qis) {
        const area = qAreas[qi];
        wx += area * qCentroids[qi * 2];
        wy += area * qCentroids[qi * 2 + 1];
        wTotal += area;
      }

      if (wTotal < 1e-12) continue;
      const tx = wx / wTotal;
      const ty = wy / wTotal;

      mesh.points[i * 2] += strength * (tx - mesh.points[i * 2]);
      mesh.points[i * 2 + 1] += strength * (ty - mesh.points[i * 2 + 1]);

      constrainVertex(mesh, i, radius, slideMap, saved[i * 2], saved[i * 2 + 1]);
    }

    for (const quad of mesh.quads) {
      if (isQuadConvex(mesh.points, quad)) continue;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        mesh.points[pi * 2] = saved[pi * 2];
        mesh.points[pi * 2 + 1] = saved[pi * 2 + 1];
      }
    }
  }
}

/**
 * Post-process: energy minimizer.
 * Gradient descent on combined energy: edge length variance + area variance
 * + angle deviation from 90 degrees.
 * Corner vertices are fixed. Boundary vertices slide along their hex edge.
 */
export function energySmooth(mesh, radius, iterations = 30, step = 0.02) {
  const numPoints = mesh.points.length / 2;
  const { fixedSet, slideMap } = getBoundaryInfo(mesh);

  const edgeSet = new Set();
  const edges = [];
  for (const quad of mesh.quads) {
    for (let i = 0; i < 4; i++) {
      const a = quad[i], b = quad[(i + 1) % 4];
      const key = a < b ? `${a},${b}` : `${b},${a}`;
      if (!edgeSet.has(key)) { edgeSet.add(key); edges.push([a, b]); }
    }
  }

  const TARGET_ANGLE = Math.PI / 2;

  for (let iter = 0; iter < iterations; iter++) {
    const grad = new Float64Array(numPoints * 2);

    // --- Edge length uniformity ---
    let totalLen = 0;
    const eLens = edges.map(([a, b]) => {
      const dx = mesh.points[a * 2] - mesh.points[b * 2];
      const dy = mesh.points[a * 2 + 1] - mesh.points[b * 2 + 1];
      const l = Math.sqrt(dx * dx + dy * dy);
      totalLen += l;
      return l;
    });
    const avgLen = totalLen / edges.length;

    for (let ei = 0; ei < edges.length; ei++) {
      const [a, b] = edges[ei];
      const dist = eLens[ei];
      if (dist < 1e-12) continue;
      const dx = mesh.points[a * 2] - mesh.points[b * 2];
      const dy = mesh.points[a * 2 + 1] - mesh.points[b * 2 + 1];
      const diff = dist - avgLen;
      const gx = diff * (dx / dist);
      const gy = diff * (dy / dist);
      if (!fixedSet.has(a)) { grad[a * 2] += gx; grad[a * 2 + 1] += gy; }
      if (!fixedSet.has(b)) { grad[b * 2] -= gx; grad[b * 2 + 1] -= gy; }
    }

    // --- Angle deviation ---
    for (const quad of mesh.quads) {
      for (let i = 0; i < 4; i++) {
        const prev = quad[(i + 3) % 4];
        const curr = quad[i];
        const next = quad[(i + 1) % 4];
        if (fixedSet.has(curr)) continue;

        const px = mesh.points[prev * 2] - mesh.points[curr * 2];
        const py = mesh.points[prev * 2 + 1] - mesh.points[curr * 2 + 1];
        const nx = mesh.points[next * 2] - mesh.points[curr * 2];
        const ny = mesh.points[next * 2 + 1] - mesh.points[curr * 2 + 1];
        const pLen = Math.sqrt(px * px + py * py);
        const nLen = Math.sqrt(nx * nx + ny * ny);
        if (pLen < 1e-12 || nLen < 1e-12) continue;

        const dot = (px * nx + py * ny) / (pLen * nLen);
        const angle = Math.acos(Math.max(-1, Math.min(1, dot)));
        const error = angle - TARGET_ANGLE;

        const bx = px / pLen + nx / nLen;
        const by = py / pLen + ny / nLen;
        const bLen = Math.sqrt(bx * bx + by * by);
        if (bLen < 1e-12) continue;

        grad[curr * 2] -= 0.5 * error * (bx / bLen);
        grad[curr * 2 + 1] -= 0.5 * error * (by / bLen);
      }
    }

    // --- Area uniformity ---
    const areas = mesh.quads.map(quad => {
      let area = 0;
      for (let i = 0; i < 4; i++) {
        const a = quad[i], b = quad[(i + 1) % 4];
        area += mesh.points[a * 2] * mesh.points[b * 2 + 1];
        area -= mesh.points[b * 2] * mesh.points[a * 2 + 1];
      }
      return Math.abs(area) / 2;
    });
    const avgArea = areas.reduce((s, a) => s + a, 0) / areas.length;

    for (let qi = 0; qi < mesh.quads.length; qi++) {
      const quad = mesh.quads[qi];
      const ratio = Math.max(-1, Math.min(1, (areas[qi] - avgArea) / avgArea));
      let cx = 0, cy = 0;
      for (const pi of quad) { cx += mesh.points[pi * 2]; cy += mesh.points[pi * 2 + 1]; }
      cx /= 4; cy /= 4;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        grad[pi * 2] -= 0.3 * ratio * (cx - mesh.points[pi * 2]);
        grad[pi * 2 + 1] -= 0.3 * ratio * (cy - mesh.points[pi * 2 + 1]);
      }
    }

    // --- Apply gradient ---
    const maxDisp = radius * 0.04;
    const saved = new Float64Array(numPoints * 2);
    for (let i = 0; i < numPoints; i++) {
      if (fixedSet.has(i)) continue;
      saved[i * 2] = mesh.points[i * 2];
      saved[i * 2 + 1] = mesh.points[i * 2 + 1];

      let fx = -step * grad[i * 2];
      let fy = -step * grad[i * 2 + 1];
      const mag = Math.sqrt(fx * fx + fy * fy);
      if (mag > maxDisp) { fx = (fx / mag) * maxDisp; fy = (fy / mag) * maxDisp; }
      mesh.points[i * 2] += fx;
      mesh.points[i * 2 + 1] += fy;

      constrainVertex(mesh, i, radius, slideMap, saved[i * 2], saved[i * 2 + 1]);
    }

    for (const quad of mesh.quads) {
      if (isQuadConvex(mesh.points, quad)) continue;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        mesh.points[pi * 2] = saved[pi * 2];
        mesh.points[pi * 2 + 1] = saved[pi * 2 + 1];
      }
    }
  }
}

/**
 * Post-process: cotangent-weighted Laplacian.
 * Weights each neighbor by the cotangent of the opposite angles in adjacent quads.
 * Preserves intentional size variation while smoothing shape irregularities.
 * Corner vertices are fixed. Boundary vertices slide along their hex edge.
 */
export function cotangentSmooth(mesh, radius, iterations = 20, strength = 0.3) {
  const numPoints = mesh.points.length / 2;
  const { fixedSet, slideMap } = getBoundaryInfo(mesh);

  const edgeOpposites = new Map();
  for (const quad of mesh.quads) {
    for (let i = 0; i < 4; i++) {
      const a = quad[i], b = quad[(i + 1) % 4];
      const key = a < b ? `${a},${b}` : `${b},${a}`;
      if (!edgeOpposites.has(key)) edgeOpposites.set(key, []);
      edgeOpposites.get(key).push(quad[(i + 2) % 4], quad[(i + 3) % 4]);
    }
  }

  const neighbors = new Map();
  for (const quad of mesh.quads) {
    for (let i = 0; i < 4; i++) {
      const a = quad[i], b = quad[(i + 1) % 4];
      if (!neighbors.has(a)) neighbors.set(a, new Set());
      if (!neighbors.has(b)) neighbors.set(b, new Set());
      neighbors.get(a).add(b);
      neighbors.get(b).add(a);
    }
  }

  for (let iter = 0; iter < iterations; iter++) {
    const saved = new Float64Array(numPoints * 2);

    for (let i = 0; i < numPoints; i++) {
      if (fixedSet.has(i)) continue;
      const nbrs = neighbors.get(i);
      if (!nbrs || nbrs.size === 0) continue;

      saved[i * 2] = mesh.points[i * 2];
      saved[i * 2 + 1] = mesh.points[i * 2 + 1];

      const ix = mesh.points[i * 2], iy = mesh.points[i * 2 + 1];
      let wx = 0, wy = 0, wTotal = 0;

      for (const j of nbrs) {
        const key = i < j ? `${i},${j}` : `${j},${i}`;
        const opposites = edgeOpposites.get(key);
        let w = 0;
        if (opposites) {
          const jx = mesh.points[j * 2], jy = mesh.points[j * 2 + 1];
          for (const o of opposites) {
            const ox = mesh.points[o * 2], oy = mesh.points[o * 2 + 1];
            const oax = ix - ox, oay = iy - oy;
            const obx = jx - ox, oby = jy - oy;
            const dot = oax * obx + oay * oby;
            const cross = Math.abs(oax * oby - oay * obx);
            if (cross > 1e-12) w += dot / cross;
          }
        }
        w = Math.max(w, 0.01);

        wx += w * mesh.points[j * 2];
        wy += w * mesh.points[j * 2 + 1];
        wTotal += w;
      }

      if (wTotal < 1e-12) continue;
      const tx = wx / wTotal;
      const ty = wy / wTotal;

      mesh.points[i * 2] += strength * (tx - ix);
      mesh.points[i * 2 + 1] += strength * (ty - iy);

      constrainVertex(mesh, i, radius, slideMap, saved[i * 2], saved[i * 2 + 1]);
    }

    for (const quad of mesh.quads) {
      if (isQuadConvex(mesh.points, quad)) continue;
      for (const pi of quad) {
        if (fixedSet.has(pi)) continue;
        mesh.points[pi * 2] = saved[pi * 2];
        mesh.points[pi * 2 + 1] = saved[pi * 2 + 1];
      }
    }
  }
}
