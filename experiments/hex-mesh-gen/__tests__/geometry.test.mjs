// tools/hex-mesh-gen/__tests__/geometry.test.mjs
import { describe, it, expect } from 'vitest';
import { hexVertices, isQuadConvex, jitterPoint, jitterBoundaryPoint } from '../lib/geometry.mjs';
import { createPRNG } from '../lib/prng.mjs';

describe('hexVertices', () => {
  it('returns 6 vertices for radius 10', () => {
    const verts = hexVertices(10);
    expect(verts).toHaveLength(6);
    verts.forEach(([x, y]) => {
      expect(Math.sqrt(x * x + y * y)).toBeCloseTo(10, 5);
    });
  });

  it('vertices are in CCW order', () => {
    const verts = hexVertices(10);
    let area = 0;
    for (let i = 0; i < 6; i++) {
      const [x1, y1] = verts[i];
      const [x2, y2] = verts[(i + 1) % 6];
      area += (x1 * y2 - x2 * y1);
    }
    expect(area).toBeGreaterThan(0);
  });
});

describe('isQuadConvex', () => {
  it('returns true for a simple convex quad', () => {
    const points = [0, 0, 1, 0, 1, 1, 0, 1];
    expect(isQuadConvex(points, [0, 1, 2, 3])).toBe(true);
  });

  it('returns false for a concave quad', () => {
    const points = [0, 0, 1, 0, 0.2, 0.2, 0, 1];
    expect(isQuadConvex(points, [0, 1, 2, 3])).toBe(false);
  });
});

describe('jitterPoint', () => {
  it('returns a point within maxJitter of the original', () => {
    const rng = createPRNG(42);
    for (let i = 0; i < 50; i++) {
      const [jx, jy] = jitterPoint(5, 3, rng, 1.0);
      const dist = Math.sqrt((jx - 5) ** 2 + (jy - 3) ** 2);
      expect(dist).toBeLessThanOrEqual(1.0 * Math.SQRT2 + 1e-9);
    }
  });
});

describe('jitterBoundaryPoint', () => {
  it('stays on the edge line', () => {
    const rng = createPRNG(42);
    const midX = 5, midY = 0;
    for (let i = 0; i < 50; i++) {
      const [jx, jy] = jitterBoundaryPoint(midX, midY, 0, 0, 10, 0, rng, 1.0);
      expect(jy).toBeCloseTo(0, 10);
      expect(jx).toBeGreaterThanOrEqual(0 - 1e-9);
      expect(jx).toBeLessThanOrEqual(10 + 1e-9);
    }
  });
});
