// tools/hex-mesh-gen/__tests__/mesh.test.mjs
import { describe, it, expect } from 'vitest';
import { createMesh, initialSplit, subdivide } from '../lib/mesh.mjs';
import { createPRNG } from '../lib/prng.mjs';
import { isQuadConvex } from '../lib/geometry.mjs';

describe('createMesh', () => {
  it('creates an empty mesh', () => {
    const mesh = createMesh();
    expect(mesh.points).toEqual([]);
    expect(mesh.quads).toEqual([]);
  });

  it('addPoint returns sequential indices', () => {
    const mesh = createMesh();
    expect(mesh.addPoint(1, 2)).toBe(0);
    expect(mesh.addPoint(3, 4)).toBe(1);
    expect(mesh.points).toEqual([1, 2, 3, 4]);
  });
});

describe('initialSplit', () => {
  it('produces exactly 3 quads and 7 points', () => {
    const rng = createPRNG(42);
    const mesh = initialSplit(10, rng);
    expect(mesh.quads).toHaveLength(3);
    expect(mesh.points).toHaveLength(14);
  });

  it('all 3 quads are convex', () => {
    const rng = createPRNG(42);
    const mesh = initialSplit(10, rng);
    for (const quad of mesh.quads) {
      expect(isQuadConvex(mesh.points, quad)).toBe(true);
    }
  });

  it('is deterministic for same seed', () => {
    const a = initialSplit(10, createPRNG(42));
    const b = initialSplit(10, createPRNG(42));
    expect(a.points).toEqual(b.points);
    expect(a.quads).toEqual(b.quads);
  });
});

describe('subdivide', () => {
  it('one level produces 3*4=12 quads', () => {
    const rng = createPRNG(42);
    const mesh = initialSplit(10, rng);
    subdivide(mesh, rng, 10, 0);
    expect(mesh.quads).toHaveLength(12);
  });

  it('all quads remain convex after subdivision', () => {
    const rng = createPRNG(42);
    const mesh = initialSplit(10, rng);
    subdivide(mesh, rng, 10, 0);
    for (const quad of mesh.quads) {
      expect(isQuadConvex(mesh.points, quad)).toBe(true);
    }
  });

  it('3 levels produces 3*64=192 quads', () => {
    const rng = createPRNG(42);
    const mesh = initialSplit(10, rng);
    for (let d = 0; d < 3; d++) {
      subdivide(mesh, rng, 10, d);
    }
    expect(mesh.quads).toHaveLength(192);
  });

  it('all quads convex after 3 levels', () => {
    const rng = createPRNG(42);
    const mesh = initialSplit(10, rng);
    for (let d = 0; d < 3; d++) {
      subdivide(mesh, rng, 10, d);
    }
    for (const quad of mesh.quads) {
      expect(isQuadConvex(mesh.points, quad)).toBe(true);
    }
  });

  it('is deterministic for same seed', () => {
    function generate(seed) {
      const rng = createPRNG(seed);
      const mesh = initialSplit(10, rng);
      for (let d = 0; d < 3; d++) subdivide(mesh, rng, 10, d);
      return { points: [...mesh.points], quads: mesh.quads.map(q => [...q]) };
    }
    const a = generate(42);
    const b = generate(42);
    expect(a.points).toEqual(b.points);
    expect(a.quads).toEqual(b.quads);
  });
});
