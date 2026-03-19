// tools/hex-mesh-gen/lib/prng.mjs

/**
 * Seeded xorshift128 PRNG.
 * @param {number} seed - Integer seed
 * @returns {{ next(): number, nextInRange(min: number, max: number): number, nextInt(max: number): number }}
 */
export function createPRNG(seed) {
  let s0 = seed >>> 0;
  let s1 = (seed * 1812433253 + 1) >>> 0;
  let s2 = (s1 * 1812433253 + 1) >>> 0;
  let s3 = (s2 * 1812433253 + 1) >>> 0;

  if ((s0 | s1 | s2 | s3) === 0) s0 = 1;

  function next() {
    let t = s3;
    t ^= t << 11;
    t ^= t >>> 8;
    s3 = s2;
    s2 = s1;
    s1 = s0;
    t ^= s0;
    t ^= s0 >>> 19;
    s0 = t;
    return (t >>> 0) / 4294967296;
  }

  function nextInRange(min, max) {
    return min + next() * (max - min);
  }

  function nextInt(max) {
    return Math.floor(next() * max);
  }

  return { next, nextInRange, nextInt };
}
