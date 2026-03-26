# Hex Mesh Gen

Proof-of-concept for hexagonal quad-mesh subdivision. Generates SVG visualizations of hex grids subdivided into quad patches with various initialization strategies, jitter modes, and smoothing algorithms.

## Prerequisites

- Node.js (ESM support required)
- [Vitest](https://vitest.dev/) for running tests (`npx vitest`)

No `package.json` — all imports are bare ESM modules.

## Scripts

### Generate a single mesh (SVG to stdout)

```sh
node hex-mesh-gen.mjs [--seed <n>]
```

Outputs an SVG of a subdivided hex mesh (radius 10, depth 3) to stdout.

### Generate variant comparison (HTML report)

```sh
node test-variants.mjs --seed 42 --open
```

Produces `test-variants.html` and opens it in the default browser. Compares:

- **Rows**: initialization strategies (3-quad, 6-quad mid, 2-quad diag) x jitter modes (none, normal, edge-only)
- **Columns**: smoothing algorithms (raw, Lloyd, cotangent)
- **Multi-seed rows**: same config across multiple random seeds
- **Tile grids**: hex tiles arranged in a flat-top honeycomb layout

Each panel shows the area ratio (max/min quad area) — lower means more uniform.

Use `--open` to auto-open the HTML file in the default browser (Windows).

### Run tests

```sh
npx vitest run
```

Tests cover geometry utilities, PRNG determinism, mesh construction, and SVG output.

## Project Structure

```
hex-mesh-gen.mjs          # Single mesh CLI
test-variants.mjs         # Variant comparison report generator
test-variants.html        # Generated report (not committed)
lib/
  geometry.mjs            # Hex vertices, convexity checks, jitter
  mesh.mjs                # Mesh creation, initial splits, subdivision
  smooth.mjs              # Smoothing algorithms (Lloyd, cotangent, spring, etc.)
  svg.mjs                 # Mesh-to-SVG rendering
  prng.mjs                # Deterministic PRNG (SplitMix32)
__tests__/
  geometry.test.mjs
  mesh.test.mjs
  svg.test.mjs
  prng.test.mjs
```
