// tools/hex-mesh-gen/__tests__/prng.test.mjs
import { describe, it, expect } from 'vitest';
import { createPRNG } from '../lib/prng.mjs';

describe('PRNG', () => {
  it('produces floats in [0, 1)', () => {
    const rng = createPRNG(42);
    for (let i = 0; i < 1000; i++) {
      const v = rng.next();
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThan(1);
    }
  });

  it('is deterministic for the same seed', () => {
    const a = createPRNG(42);
    const b = createPRNG(42);
    for (let i = 0; i < 100; i++) {
      expect(a.next()).toBe(b.next());
    }
  });

  it('produces different sequences for different seeds', () => {
    const a = createPRNG(1);
    const b = createPRNG(2);
    const valuesA = Array.from({ length: 10 }, () => a.next());
    const valuesB = Array.from({ length: 10 }, () => b.next());
    expect(valuesA).not.toEqual(valuesB);
  });

  it('nextInRange returns value in [min, max)', () => {
    const rng = createPRNG(99);
    for (let i = 0; i < 100; i++) {
      const v = rng.nextInRange(-5, 5);
      expect(v).toBeGreaterThanOrEqual(-5);
      expect(v).toBeLessThan(5);
    }
  });

  it('nextInt returns integer in [0, max)', () => {
    const rng = createPRNG(7);
    for (let i = 0; i < 100; i++) {
      const v = rng.nextInt(6);
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThan(6);
      expect(Number.isInteger(v)).toBe(true);
    }
  });
});
