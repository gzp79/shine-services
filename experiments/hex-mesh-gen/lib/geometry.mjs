// tools/hex-mesh-gen/lib/geometry.mjs

export function hexVertices(radius) {
  const verts = [];
  for (let i = 0; i < 6; i++) {
    const angle = (Math.PI / 3) * i;
    verts.push([radius * Math.cos(angle), radius * Math.sin(angle)]);
  }
  return verts;
}

export function isQuadConvex(points, quad) {
  let sign = 0;
  for (let i = 0; i < 4; i++) {
    const a = quad[i];
    const b = quad[(i + 1) % 4];
    const c = quad[(i + 2) % 4];
    const ax = points[a * 2], ay = points[a * 2 + 1];
    const bx = points[b * 2], by = points[b * 2 + 1];
    const cx = points[c * 2], cy = points[c * 2 + 1];
    const cross = (bx - ax) * (cy - by) - (by - ay) * (cx - bx);
    if (cross === 0) continue;
    const s = cross > 0 ? 1 : -1;
    if (sign === 0) sign = s;
    else if (s !== sign) return false;
  }
  return true;
}

/**
 * Check if a point is inside a regular hexagon centered at origin.
 * Uses half-plane intersection of the 6 edges.
 */
export function isInsideHex(x, y, radius) {
  const verts = hexVertices(radius);
  for (let i = 0; i < 6; i++) {
    const [ax, ay] = verts[i];
    const [bx, by] = verts[(i + 1) % 6];
    const cross = (bx - ax) * (y - ay) - (by - ay) * (x - ax);
    if (cross < -1e-10) return false;
  }
  return true;
}

export function jitterPoint(x, y, rng, maxJitter) {
  const dx = rng.nextInRange(-maxJitter, maxJitter);
  const dy = rng.nextInRange(-maxJitter, maxJitter);
  return [x + dx, y + dy];
}

export function jitterBoundaryPoint(x, y, sx, sy, ex, ey, rng, maxJitter) {
  const dx = ex - sx;
  const dy = ey - sy;
  const len = Math.sqrt(dx * dx + dy * dy);
  if (len === 0) return [x, y];
  const ux = dx / len;
  const uy = dy / len;
  const offset = rng.nextInRange(-maxJitter, maxJitter);
  let nx = x + ux * offset;
  let ny = y + uy * offset;
  const t = ((nx - sx) * dx + (ny - sy) * dy) / (len * len);
  const clamped = Math.max(0, Math.min(1, t));
  nx = sx + dx * clamped;
  ny = sy + dy * clamped;
  return [nx, ny];
}
