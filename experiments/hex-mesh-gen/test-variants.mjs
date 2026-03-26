// tools/hex-mesh-gen/test-variants.mjs
// Visual test: compares init/jitter variants x smoothing pipelines.
// Usage: node test-variants.mjs [--seed <n>] [--open]

import { createPRNG } from './lib/prng.mjs';
import { initialSplit, initialSplitMid, initialSplitDiag, subdivide } from './lib/mesh.mjs';
import { lloyd, cotangentSmooth } from './lib/smooth.mjs';
import { meshToSVG } from './lib/svg.mjs';
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import { execSync } from 'node:child_process';

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

function buildMesh(seed, initFn, subdivOpts, depth = DEPTH) {
  const rng = createPRNG(seed);
  const m = initFn(RADIUS, rng, subdivOpts);
  for (let d = 0; d < depth; d++) subdivide(m, rng, RADIUS, d, subdivOpts);
  return m;
}

function areaRatio(m) {
  const areas = m.quads.map(q => {
    let a = 0;
    for (let i = 0; i < 4; i++) {
      const p = q[i], n = q[(i + 1) % 4];
      a += m.points[p * 2] * m.points[n * 2 + 1] - m.points[n * 2] * m.points[p * 2 + 1];
    }
    return Math.abs(a) / 2;
  });
  const sorted = [...areas].sort((a, b) => a - b);
  return (sorted[sorted.length - 1] / sorted[0]).toFixed(1);
}

function applyPipeline(seed, initFn, subdivOpts, smoothFn) {
  const m = buildMesh(seed, initFn, subdivOpts);
  if (smoothFn) smoothFn(m);
  const ratio = areaRatio(m);
  const svg = meshToSVG(m.points, m.quads);
  return { svg, ratio };
}

const seed = parseSeed(process.argv);
const shouldOpen = process.argv.includes('--open');
console.log(`Seed: ${seed}`);

const R = RADIUS;

const rowVariants = [
  { label: '3-quad init, no jitter', initFn: initialSplit, opts: { noJitter: true } },
  { label: '3-quad init, normal jitter', initFn: initialSplit, opts: {} },
  { label: '3-quad init, edge-only jitter', initFn: initialSplit, opts: { edgeOnly: true } },
  { label: '6-quad init (mid), no jitter', initFn: initialSplitMid, opts: { noJitter: true } },
  { label: '6-quad init (mid), normal jitter', initFn: initialSplitMid, opts: {} },
  { label: '6-quad init (mid), edge-only jitter', initFn: initialSplitMid, opts: { edgeOnly: true } },
  { label: '2-quad init (diag), no jitter', initFn: initialSplitDiag, opts: { noJitter: true } },
  { label: '2-quad init (diag), normal jitter', initFn: initialSplitDiag, opts: {} },
  { label: '2-quad init (diag), edge-only jitter', initFn: initialSplitDiag, opts: { edgeOnly: true } },
];

const smoothVariants = [
  { label: 'Raw', fn: null },
  { label: 'Lloyd (5 iter)', fn: m => lloyd(m, R, 5) },
  { label: 'Cotangent', fn: m => cotangentSmooth(m, R) },
];

const MULTI_SEEDS = 8;
const cols = Math.max(smoothVariants.length, MULTI_SEEDS);

let panelsHtml = '';
for (const rv of rowVariants) {
  panelsHtml += `    <div class="row-label">${rv.label}</div>\n`;
  for (const sv of smoothVariants) {
    const { svg, ratio } = applyPipeline(seed, rv.initFn, rv.opts, sv.fn);
    panelsHtml += `    <div class="panel"><h3>${sv.label} <span class="ratio">${ratio}x</span></h3>${svg}</div>\n`;
  }
}

// Multi-seed row: 3-quad init, edge-only jitter with different random seeds
const multiSeedSmooths = [
  { label: 'Raw', fn: null },
  { label: 'Lloyd (2 iter)', fn: m => lloyd(m, R, 2) },
  { label: 'Lloyd (3 iter)', fn: m => lloyd(m, R, 3) },
  { label: 'Lloyd (5 iter)', fn: m => lloyd(m, R, 5) },
];
for (const sv of multiSeedSmooths) {
  panelsHtml += `    <div class="row-label">Multi-seed: 3-quad, normal jitter — ${sv.label}</div>\n`;
  for (let i = 0; i < MULTI_SEEDS; i++) {
    const s = seed + i;
    const { svg, ratio } = applyPipeline(s, initialSplit, {}, sv.fn);
    panelsHtml += `    <div class="panel"><h3>seed ${s} <span class="ratio">${ratio}x</span></h3>${svg}</div>\n`;
  }
}

// --- Hex tile grid section ---
const TILE_COLS = 6;
const TILE_ROWS = 4;
// Flat-top hex tiling: col spacing = 1.5*R, row spacing = sqrt(3)*R, odd cols offset by sqrt(3)/2*R
const colStep = 1.5 * R;
const rowStep = Math.sqrt(3) * R;
const halfRow = rowStep / 2;

function meshToPolygons(mesh, offsetX, offsetY) {
  return mesh.quads.map(quad => {
    const pts = quad.map(i => {
      const x = (mesh.points[i * 2] + offsetX).toFixed(4);
      const y = (mesh.points[i * 2 + 1] + offsetY).toFixed(4);
      return `${x},${y}`;
    }).join(' ');
    return `  <polygon points="${pts}"/>`;
  }).join('\n');
}

function buildTileGrid(smoothFn, depth = DEPTH) {
  let polygons = '';
  for (let row = 0; row < TILE_ROWS; row++) {
    for (let col = 0; col < TILE_COLS; col++) {
      const ox = col * colStep;
      const oy = row * rowStep + (col % 2 ? halfRow : 0);
      const tileSeed = seed + row * TILE_COLS + col;
      const m = buildMesh(tileSeed, initialSplit, {}, depth);
      if (smoothFn) smoothFn(m);
      polygons += meshToPolygons(m, ox, oy) + '\n';
    }
  }
  const w = (TILE_COLS - 1) * colStep + 2 * R + 4;
  const h = (TILE_ROWS - 1) * rowStep + halfRow + 2 * R + 4;
  const minX = -R - 2;
  const minY = -R - 2;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${minX} ${minY} ${w} ${h}">\n${polygons}</svg>`;
}

const tileRawSvg = buildTileGrid(null);
const tileSvg = buildTileGrid(m => lloyd(m, R, 5, 0.4), DEPTH + 1);

const html = `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Hex Mesh Variants (seed: ${seed})</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: system-ui, sans-serif; background: #1a1a2e; color: #e0e0e0; padding: 24px; }
  h1 { font-size: 1.2rem; margin-bottom: 4px; }
  .info { font-size: 0.85rem; color: #888; margin-bottom: 20px; }
  .scroll-wrapper {
    overflow-x: auto;
    padding-bottom: 12px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(${cols}, 280px);
    gap: 12px;
    width: max-content;
  }
  .row-label {
    grid-column: 1 / -1;
    font-size: 1rem;
    color: #ffd580;
    border-bottom: 1px solid #333;
    padding-bottom: 4px;
    margin-top: 12px;
    position: sticky;
    left: 0;
  }
  .panel { background: #16213e; border-radius: 8px; padding: 10px; }
  .panel h3 { font-size: 0.75rem; margin-bottom: 4px; color: #a0c4ff; }
  .panel h3 .ratio { color: #ff9; font-weight: normal; }
  .panel svg { width: 100%; height: auto; }
  .panel svg polygon { stroke: #5c7cfa; stroke-width: 0.06; fill: rgba(92,124,250,0.08); }
  .panel svg polygon:hover { fill: rgba(92,124,250,0.25); stroke: #fff; }
  .tile-section { margin-top: 32px; }
  .tile-section h2 { font-size: 1.1rem; color: #ffd580; margin-bottom: 12px; }
  .tile-section { overflow: auto; }
  .tile-section svg { width: 3600px; height: auto; }
  .tile-section svg polygon { stroke: #5c7cfa; stroke-width: 0.06; fill: rgba(92,124,250,0.08); }
  .tile-section svg polygon:hover { fill: rgba(92,124,250,0.25); stroke: #fff; }
</style>
</head>
<body>
  <h1>Hex Mesh - All Variants</h1>
  <p class="info">Seed: ${seed} | Depth: ${DEPTH} | Area ratio shown per panel (lower = more uniform)</p>
  <div class="scroll-wrapper"><div class="grid">
${panelsHtml}  </div></div>
  <div class="tile-section">
    <h2>Hex Tile Grid — 3-quad, normal jitter, Raw (no smoothing)</h2>
    ${tileRawSvg}
  </div>
  <div class="tile-section">
    <h2>Hex Tile Grid — 3-quad, normal jitter, Lloyd (5 iter, 0.4 strength)</h2>
    ${tileSvg}
  </div>
</body>
</html>
`;

const dir = dirname(fileURLToPath(import.meta.url));
const outPath = join(dir, 'test-variants.html');
writeFileSync(outPath, html);
console.log(`Written: ${outPath}`);

if (shouldOpen) {
  try {
    execSync(`start "" "${outPath}"`, { stdio: 'ignore' });
  } catch {
    console.log('Could not auto-open. Open the file manually.');
  }
}
