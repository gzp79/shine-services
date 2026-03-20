# shine-web

Three.js hex quad mesh viewer powered by the `shine-game` Rust crate via WebAssembly.

## Prerequisites

- [pnpm](https://pnpm.io/)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)
- Rust toolchain with `wasm32-unknown-unknown` target

## Development

```bash
pnpm install
pnpm dev
```

The dev server automatically builds the wasm if `pkg/` doesn't exist and watches `crates/shine-game/src/**/*.rs` for changes — rebuilding and restarting on every Rust file change.

To rebuild wasm manually:

```bash
pnpm wasm
```

## Build

```bash
pnpm build
```

Produces a library bundle in `dist/`:

- `shine-web.js` — ESM module
- `shine-web.umd.cjs` — UMD module
- `shine_game_bg.wasm` — WebAssembly binary (separate asset, not inlined)

## Usage as a component

```ts
import { createHexMeshViewer } from 'shine-web';

const viewer = await createHexMeshViewer(document.getElementById('container')!);

// Clean up when done
viewer.destroy();
```

The container element must have explicit dimensions (width/height). The viewer fills the container and responds to resize.

Peer dependencies: `three`, `lil-gui`.

## Linting

```bash
pnpm lint        # format + eslint + type check
pnpm format      # auto-format with prettier
```

## Wasm type sync

TypeScript types for the wasm API live in `src/wasm-types/` and are auto-copied from `pkg/` after every wasm build (via `postwasm` script and the vite dev plugin). Commit changes to `src/wasm-types/` when the Rust API changes.
