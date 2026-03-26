// tools/hex-mesh-gen/hex-mesh-gen.mjs
import { createPRNG } from './lib/prng.mjs';
import { initialSplit, subdivide, repulse, springSmooth } from './lib/mesh.mjs';
import { meshToSVG } from './lib/svg.mjs';

const RADIUS = 10;
const DEPTH = 3;

function parseSeed(args) {
  const idx = args.indexOf('--seed');
  if (idx !== -1 && args[idx + 1] != null) {
    const seed = parseInt(args[idx + 1], 10);
    if (!Number.isNaN(seed)) return seed;
  }
  return Date.now() ^ (Math.random() * 0xffffffff);
}

const seed = parseSeed(process.argv);
const rng = createPRNG(seed);

const mesh = initialSplit(RADIUS, rng);
for (let d = 0; d < DEPTH; d++) {
  subdivide(mesh, rng, RADIUS, d);
}

repulse(mesh, RADIUS);
springSmooth(mesh, RADIUS);

const svg = meshToSVG(mesh.points, mesh.quads);
process.stdout.write(svg + '\n');
