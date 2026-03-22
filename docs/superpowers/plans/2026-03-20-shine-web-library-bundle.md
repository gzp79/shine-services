# Shine-Web Library Bundle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the hex mesh viewer at `client/web/` into a reusable library called "shine-web" that can be mounted into any container element and consumed as a Svelte or React component.

**Architecture:** Extract the core Three.js rendering into a container-aware `createHexMeshViewer(container, options)` API exported from `src/lib.ts`. The existing `main.ts` becomes a thin standalone demo that uses this API. Vite library mode produces an ES module bundle with `three` as a peer dependency. WASM is side-loaded (not inlined) so consumers control asset serving.

**Tech Stack:** TypeScript, Vite (library mode), three.js (peer dep), wasm-bindgen, lil-gui (demo-only)

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `client/web/package.json` | Modify | Rename to `shine-web`, add `main`/`module`/`types`/`exports` fields, move `three` to peerDeps |
| `client/web/tsconfig.json` | Modify | Add `declaration: true`, remove `#wasm` path alias |
| `client/web/vite.config.ts` | Modify | Add `build.lib` config, remove `#wasm` resolve alias |
| `client/web/src/lib.ts` | Create | Public API: `createHexMeshViewer()`, `HexMeshViewerHandle`, re-exports |
| `client/web/src/params.ts` | Create | `MeshParams` type, `defaultParams()`, `paramsToConfigJson()` — pure data, no GUI dependency |
| `client/web/src/scene.ts` | Modify | Accept `container: HTMLElement`, use `ResizeObserver`, return `dispose()` |
| `client/web/src/mesh-builder.ts` | No change | Already pure (no DOM coupling) |
| `client/web/src/controls.ts` | Modify | Import params from `params.ts`, make `createControls` accept optional container |
| `client/web/src/main.ts` | Modify | Thin demo that imports from `./lib` |
| `client/web/src/wasm.d.ts` | Delete | Replaced by wasm-pack generated types in `pkg/shine_game.d.ts` |
| `client/web/index.html` | Modify | Update title to "Shine Web Demo", add `#viewer` div |

---

## Task 1: Decouple scene.ts from global DOM

**Files:**
- Modify: `client/web/src/scene.ts`

### Context

Currently `createScene()` hard-codes `document.body.appendChild`, uses `window.innerWidth/Height`, and attaches a `window` resize listener. For library use it must accept a container element, size to that container, and use `ResizeObserver` for responsive sizing. The `animate()` function starts an infinite rAF loop with no way to stop it — must return a cleanup handle.

- [ ] **Step 1: Refactor `createScene` to accept a container**

Replace the function signature and DOM-related code in `client/web/src/scene.ts`:

```typescript
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';

export interface SceneContext {
    scene: THREE.Scene;
    camera: THREE.PerspectiveCamera;
    renderer: THREE.WebGLRenderer;
    controls: OrbitControls;
    dispose: () => void;
}

export function createScene(container: HTMLElement): SceneContext {
    const scene = new THREE.Scene();
    scene.background = new THREE.Color(0x1a1a2e);

    const width = container.clientWidth || 800;
    const height = container.clientHeight || 600;

    const camera = new THREE.PerspectiveCamera(50, width / height, 0.1, 1000);
    camera.position.set(0, 25, 15);
    camera.lookAt(0, 0, 0);

    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setSize(width, height);
    renderer.setPixelRatio(window.devicePixelRatio);
    container.appendChild(renderer.domElement);

    const controls = new OrbitControls(camera, renderer.domElement);
    controls.target.set(0, 0, 0);
    controls.enableDamping = true;
    controls.dampingFactor = 0.1;
    controls.update();

    // Lighting
    const ambient = new THREE.AmbientLight(0xffffff, 0.6);
    scene.add(ambient);

    const directional = new THREE.DirectionalLight(0xffffff, 0.8);
    directional.position.set(10, 20, 5);
    scene.add(directional);

    // Resize handling via ResizeObserver (container-aware)
    const resizeObserver = new ResizeObserver((entries) => {
        for (const entry of entries) {
            const { width: w, height: h } = entry.contentRect;
            if (w === 0 || h === 0) continue;
            camera.aspect = w / h;
            camera.updateProjectionMatrix();
            renderer.setSize(w, h);
        }
    });
    resizeObserver.observe(container);

    const dispose = () => {
        resizeObserver.disconnect();
        controls.dispose();
        renderer.dispose();
        container.removeChild(renderer.domElement);
    };

    return { scene, camera, renderer, controls, dispose };
}
```

- [ ] **Step 2: Refactor `animate` to return a stop function**

Replace the `animate` function in the same file:

```typescript
export function animate(ctx: SceneContext): () => void {
    let animationId: number | null = null;

    function loop() {
        animationId = requestAnimationFrame(loop);
        ctx.controls.update();
        ctx.renderer.render(ctx.scene, ctx.camera);
    }
    loop();

    return () => {
        if (animationId !== null) {
            cancelAnimationFrame(animationId);
            animationId = null;
        }
    };
}
```

- [ ] **Step 3: Commit**

```
refactor: decouple scene.ts from global DOM, accept container element
```

---

## Task 2: Extract params from controls.ts and make controls container-aware

**Files:**
- Create: `client/web/src/params.ts`
- Modify: `client/web/src/controls.ts`

### Context

`controls.ts` currently contains both pure data (`MeshParams`, `defaultParams`, `paramsToConfigJson`) and the lil-gui UI (`createControls`). For the library bundle, consumers who don't use lil-gui should not pull it in. Split the pure params into `params.ts` so `lib.ts` can import them without a lil-gui dependency.

Also, `lil-gui` by default appends to `document.body`. Add an optional `container` parameter for library use.

- [ ] **Step 1: Create `params.ts`**

Create `client/web/src/params.ts` with the `MeshParams` interface, `defaultParams()`, and `paramsToConfigJson()` — everything currently in `controls.ts` except `createControls` and the `import GUI` line:

```typescript
export interface MeshParams {
    subdivision: number;
    orientation: string;
    smoothing: string;
    seed: number;
    // Lloyd
    lloyd_iterations: number;
    lloyd_strength: number;
    lloyd_weight_min: number;
    lloyd_weight_max: number;
    // Noise
    noise_amplitude: number;
    noise_frequency: number;
    // Cotangent
    cotangent_iterations: number;
    cotangent_strength: number;
    // Spring
    spring_iterations: number;
    spring_dt: number;
    spring_spring_strength: number;
    spring_shape_strength: number;
    // Jitter
    jitter_amplitude: number;
    // Fix quads
    fix_enabled: boolean;
    fix_min_quality: number;
    fix_max_iterations: number;
}

export function defaultParams(): MeshParams {
    return {
        subdivision: 3,
        orientation: 'Even',
        smoothing: 'None',
        seed: 42,
        lloyd_iterations: 20,
        lloyd_strength: 0.4,
        lloyd_weight_min: 2.5,
        lloyd_weight_max: 15.5,
        noise_amplitude: 0.5,
        noise_frequency: 5.0,
        cotangent_iterations: 10,
        cotangent_strength: 0.5,
        spring_iterations: 50,
        spring_dt: 0.1,
        spring_spring_strength: 0.3,
        spring_shape_strength: 0.5,
        jitter_amplitude: 2.0,
        fix_enabled: true,
        fix_min_quality: 0.15,
        fix_max_iterations: 50
    };
}

export function paramsToConfigJson(p: MeshParams): string {
    const smoothing: Record<string, unknown> = { method: p.smoothing };

    switch (p.smoothing) {
        case 'Lloyd':
            smoothing.iterations = p.lloyd_iterations;
            smoothing.strength = p.lloyd_strength;
            smoothing.weight_min = p.lloyd_weight_min;
            smoothing.weight_max = p.lloyd_weight_max;
            break;
        case 'Noise':
            smoothing.amplitude = p.noise_amplitude;
            smoothing.frequency = p.noise_frequency;
            break;
        case 'Cotangent':
            smoothing.iterations = p.cotangent_iterations;
            smoothing.strength = p.cotangent_strength;
            break;
        case 'Spring':
            smoothing.iterations = p.spring_iterations;
            smoothing.dt = p.spring_dt;
            smoothing.spring_strength = p.spring_spring_strength;
            smoothing.shape_strength = p.spring_shape_strength;
            break;
        case 'Jitter':
            smoothing.amplitude = p.jitter_amplitude;
            break;
    }

    return JSON.stringify({
        subdivision: p.subdivision,
        orientation: p.orientation,
        seed: p.seed,
        smoothing,
        fix_quads: {
            enabled: p.fix_enabled,
            min_quality: p.fix_min_quality,
            max_iterations: p.fix_max_iterations
        }
    });
}
```

- [ ] **Step 2: Update `controls.ts` to import from `params.ts`**

Replace the top of `client/web/src/controls.ts`. Remove the `MeshParams` interface, `defaultParams`, and `paramsToConfigJson` definitions. Replace with imports:

```typescript
import GUI from 'lil-gui';

import { MeshParams } from './params';

export type { MeshParams };

export function createControls(
    params: MeshParams,
    onChange: () => void,
    container?: HTMLElement
): GUI {
    const gui = new GUI({ title: 'Hex Mesh', container });
    // ... rest of createControls unchanged from current code
```

The function body stays the same — only the imports, removed definitions, and `new GUI({ title: 'Hex Mesh', container })` change.

- [ ] **Step 3: Commit**

```
refactor: extract params.ts from controls.ts, add optional GUI container
```

---

## Task 3: Remove `#wasm` alias and `wasm.d.ts`

**Files:**
- Delete: `client/web/src/wasm.d.ts`
- Modify: `client/web/vite.config.ts`
- Modify: `client/web/tsconfig.json`

### Context

The current code uses a `#wasm` import alias defined in `vite.config.ts` (resolve.alias) and `tsconfig.json` (paths), with manual type declarations in `wasm.d.ts`. Since wasm-pack generates its own `.d.ts` in `pkg/`, we can import directly from `../pkg/shine_game.js` and let TypeScript resolve the types from `pkg/shine_game.d.ts`. This must happen before creating `lib.ts` so the import path works.

Note: The current `vite.config.ts` has a `resolve.alias` mapping `#wasm` → `./pkg/shine_game.js` and the current `tsconfig.json` has `paths: { "#wasm": ["./pkg/shine_game.js"] }`. Both need to be removed.

- [ ] **Step 1: Delete `wasm.d.ts`**

Delete: `client/web/src/wasm.d.ts`

- [ ] **Step 2: Remove `#wasm` alias from `vite.config.ts`**

Replace `client/web/vite.config.ts` with (removing the `resolve.alias` block):

```typescript
import { defineConfig } from 'vite';
import topLevelAwait from 'vite-plugin-top-level-await';
import wasm from 'vite-plugin-wasm';

export default defineConfig({
    plugins: [wasm(), topLevelAwait()]
});
```

- [ ] **Step 3: Remove `#wasm` paths from `tsconfig.json`**

Remove the `paths` and `baseUrl` entries from `client/web/tsconfig.json` (if present). The result should be:

```json
{
    "compilerOptions": {
        "target": "ES2020",
        "module": "ESNext",
        "moduleResolution": "bundler",
        "strict": true,
        "esModuleInterop": true,
        "skipLibCheck": true,
        "forceConsistentCasingInFileNames": true,
        "outDir": "dist",
        "sourceMap": true
    },
    "include": ["src", "eslint.config.ts", "vite.config.ts"]
}
```

- [ ] **Step 4: Update `main.ts` import**

In `client/web/src/main.ts`, change the wasm import from:

```typescript
import init, { generate_mesh } from '#wasm';
```

to:

```typescript
import init, { generate_mesh } from '../pkg/shine_game.js';
```

- [ ] **Step 5: Verify everything still works**

Run: `cd client/web && pnpm dev`
Expected: Demo works as before. No import errors in browser console.

Run: `cd client/web && npx tsc --noEmit`
Expected: Compiles without errors (wasm-pack generated `pkg/shine_game.d.ts` provides the types).

- [ ] **Step 6: Commit**

```
refactor: remove #wasm alias, use direct pkg import path
```

---

## Task 4: Create the library entry point

**Files:**
- Create: `client/web/src/lib.ts`

### Context

This is the main public API. It wraps scene creation, WASM init, mesh generation, and lifecycle into a single `createHexMeshViewer()` function. Returns a handle with `update()` and `dispose()` methods. Consumers pass a container element and optional initial params.

The WASM module exports `init(wasmUrl?)` and `generate_mesh(configJson)`. The `init()` function accepts an optional URL/Response for the `.wasm` file, which lets consumers control where it's served from.

Note: `lib.ts` imports params from `params.ts` (no lil-gui dependency), not from `controls.ts`. This ensures consumers who don't use the built-in controls don't pull in lil-gui.

- [ ] **Step 1: Create `lib.ts`**

Create `client/web/src/lib.ts`:

```typescript
import init, { generate_mesh } from '../pkg/shine_game.js';

import { MeshData, HexMeshGroup, buildHexMesh } from './mesh-builder';
import { MeshParams, defaultParams, paramsToConfigJson } from './params';
import { SceneContext, animate, createScene } from './scene';

export type { MeshData, MeshParams, HexMeshGroup, SceneContext };
export { defaultParams, paramsToConfigJson, buildHexMesh };

export interface HexMeshViewerOptions {
    /** Initial mesh parameters. Uses defaults for any omitted fields. */
    params?: Partial<MeshParams>;
    /** URL or Response for the .wasm file. If omitted, uses the default fetch. */
    wasmUrl?: string | URL | Response;
}

export interface HexMeshViewerHandle {
    /** Update mesh parameters and regenerate. Merges with current params. */
    update(params: Partial<MeshParams>): void;
    /** Get current parameters (copy). */
    getParams(): MeshParams;
    /** Access the Three.js scene context for advanced usage. */
    readonly sceneContext: SceneContext;
    /** Tear down the viewer: stop animation, remove canvas, free GPU resources. */
    dispose(): void;
}

/**
 * Create a hex mesh viewer inside the given container element.
 * The container must have a non-zero size (set width/height via CSS).
 *
 * @throws If WASM initialization fails or mesh generation fails with invalid params.
 */
export async function createHexMeshViewer(
    container: HTMLElement,
    options?: HexMeshViewerOptions
): Promise<HexMeshViewerHandle> {
    await init(options?.wasmUrl);

    const ctx = createScene(container);
    const stopAnimation = animate(ctx);
    let currentParams: MeshParams = { ...defaultParams(), ...options?.params };
    let currentMesh: HexMeshGroup | null = null;

    function regenerate() {
        if (currentMesh) {
            ctx.scene.remove(currentMesh.group);
            currentMesh.dispose();
            currentMesh = null;
        }

        const configJson = paramsToConfigJson(currentParams);
        const wasmMesh = generate_mesh(configJson);

        const data: MeshData = {
            vertices: wasmMesh.vertices(),
            indices: wasmMesh.indices(),
            patchIndices: wasmMesh.patch_indices()
        };

        wasmMesh.free();

        currentMesh = buildHexMesh(data);
        ctx.scene.add(currentMesh.group);
    }

    regenerate();

    return {
        update(params: Partial<MeshParams>) {
            Object.assign(currentParams, params);
            regenerate();
        },
        getParams() {
            return { ...currentParams };
        },
        get sceneContext() {
            return ctx;
        },
        dispose() {
            stopAnimation();
            if (currentMesh) {
                ctx.scene.remove(currentMesh.group);
                currentMesh.dispose();
                currentMesh = null;
            }
            ctx.dispose();
        }
    };
}
```

Note: `regenerate()` intentionally lets errors propagate (no try/catch). The JSDoc on `createHexMeshViewer` documents this. `update()` can also throw if params produce invalid config — consumers should catch if needed.

- [ ] **Step 2: Verify it compiles**

Run: `cd client/web && npx tsc --noEmit`
Expected: No errors.

- [ ] **Step 3: Commit**

```
feat: add library entry point with createHexMeshViewer API
```

---

## Task 5: Update main.ts to use lib.ts

**Files:**
- Modify: `client/web/src/main.ts`
- Modify: `client/web/index.html`

### Context

The standalone demo should now use the library API, proving it works end-to-end. It creates a full-page container div, mounts the viewer, and adds lil-gui controls that call `viewer.update()`.

- [ ] **Step 1: Replace main.ts**

Replace `client/web/src/main.ts` with:

```typescript
import { createControls } from './controls';
import { createHexMeshViewer, defaultParams } from './lib';

async function main() {
    const container = document.getElementById('viewer')!;
    const params = defaultParams();

    const viewer = await createHexMeshViewer(container, { params });

    createControls(params, () => {
        viewer.update(params);
    });
}

void main();
```

- [ ] **Step 2: Update index.html**

Replace `client/web/index.html` with:

```html
<!doctype html>
<html lang="en">
    <head>
        <meta charset="UTF-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1.0" />
        <title>Shine Web Demo</title>
        <style>
            body {
                margin: 0;
                overflow: hidden;
                background: #1a1a2e;
            }
            #viewer {
                width: 100vw;
                height: 100vh;
            }
            canvas {
                display: block;
            }
        </style>
    </head>
    <body>
        <div id="viewer"></div>
        <script type="module" src="/src/main.ts"></script>
    </body>
</html>
```

- [ ] **Step 3: Verify demo still works**

Run: `cd client/web && pnpm wasm && pnpm dev`
Expected: Same behavior as before — hex mesh renders, controls work, orbit controls work. The only visual difference is the page title.

- [ ] **Step 4: Commit**

```
refactor: update demo to use library API
```

---

## Task 6: Configure Vite library mode and package metadata

**Files:**
- Modify: `client/web/package.json`
- Modify: `client/web/vite.config.ts`
- Modify: `client/web/tsconfig.json`

### Context

Vite library mode builds `src/lib.ts` as the entry point into a single ES module. `three` is externalized as a peer dependency (consumers bring their own). The WASM `.wasm` file is **not** inlined — it ships alongside the bundle and consumers either serve it statically or pass a URL via `wasmUrl`.

The demo (`main.ts` + `index.html`) is only used during development (`pnpm dev`). The library build only includes `lib.ts` and its dependencies.

- [ ] **Step 1: Update package.json**

Replace `client/web/package.json`:

```json
{
    "name": "shine-web",
    "version": "0.1.0",
    "type": "module",
    "main": "dist/shine-web.js",
    "module": "dist/shine-web.js",
    "types": "dist/lib.d.ts",
    "exports": {
        ".": {
            "import": "./dist/shine-web.js",
            "types": "./dist/lib.d.ts"
        },
        "./controls": {
            "import": "./dist/shine-web.js",
            "types": "./dist/controls.d.ts"
        },
        "./wasm": "./pkg/shine_game_bg.wasm"
    },
    "files": [
        "dist",
        "pkg/shine_game_bg.wasm",
        "pkg/shine_game.js",
        "pkg/shine_game.d.ts"
    ],
    "scripts": {
        "dev": "vite",
        "build": "tsc && vite build",
        "build:lib": "tsc --declaration --emitDeclarationOnly --outDir dist && vite build",
        "wasm": "wasm-pack build ../../crates/shine-game --target web --out-dir ../../client/web/pkg",
        "wasm:dev": "pnpm wasm && pnpm dev",
        "format": "prettier --write .",
        "lint": "pnpm run lint:format && pnpm run lint:eslint && pnpm run lint:build",
        "lint:format": "prettier --check .",
        "lint:eslint": "eslint .",
        "lint:build": "tsc -b --noEmit"
    },
    "peerDependencies": {
        "three": ">=0.160"
    },
    "devDependencies": {
        "@eslint/js": "^9.37.0",
        "@stylistic/eslint-plugin": "^5.4.0",
        "@trivago/prettier-plugin-sort-imports": "5.2.2",
        "@types/three": "^0.172",
        "eslint": "^9.24.0",
        "eslint-config-prettier": "^10.1.2",
        "jiti": "^2.6.1",
        "lil-gui": "^0.20",
        "prettier": "^3.5.3",
        "three": "^0.172",
        "typescript": "^5.8.3",
        "typescript-eslint": "^8.30.1",
        "vite": "^6.2",
        "vite-plugin-wasm": "^3.4",
        "vite-plugin-top-level-await": "^1.5"
    }
}
```

Key changes:
- `name` → `shine-web`, removed `"private": true`
- `three` moved to `peerDependencies` (with `>=0.160` for broad compat), kept in `devDependencies` for the demo
- `lil-gui` moved to `devDependencies` — it's only used by `controls.ts` which is demo/optional code; `lib.ts` imports from `params.ts` instead
- Added `main`, `module`, `types`, `exports`, `files` fields
- Added `build:lib` script that emits declarations + builds library

- [ ] **Step 2: Update vite.config.ts**

Replace `client/web/vite.config.ts`:

```typescript
import { defineConfig } from 'vite';
import topLevelAwait from 'vite-plugin-top-level-await';
import wasm from 'vite-plugin-wasm';

export default defineConfig({
    plugins: [wasm(), topLevelAwait()],
    build: {
        lib: {
            entry: 'src/lib.ts',
            formats: ['es'],
            fileName: 'shine-web'
        },
        rollupOptions: {
            external: ['three', /^three\//, 'lil-gui']
        }
    }
});
```

Notes:
- `external: ['three', /^three\//, 'lil-gui']` externalizes three.js and lil-gui
- Only `es` format (no UMD) — modern bundlers all support ESM

- [ ] **Step 3: Update tsconfig.json**

Replace `client/web/tsconfig.json`:

```json
{
    "compilerOptions": {
        "target": "ES2020",
        "module": "ESNext",
        "moduleResolution": "bundler",
        "strict": true,
        "esModuleInterop": true,
        "skipLibCheck": true,
        "forceConsistentCasingInFileNames": true,
        "outDir": "dist",
        "declaration": true,
        "declarationMap": true,
        "sourceMap": true
    },
    "include": ["src", "eslint.config.ts", "vite.config.ts"]
}
```

Changes from Task 3: Added `declaration: true` and `declarationMap: true` for `.d.ts` output.

- [ ] **Step 4: Verify library build**

Run: `cd client/web && pnpm build:lib`
Expected: Produces `dist/shine-web.js` (ES module), `dist/lib.d.ts`, and related declaration files. The bundle should NOT contain three.js code (it's externalized).

- [ ] **Step 5: Verify the bundle doesn't include three.js**

Run: `head -20 client/web/dist/shine-web.js`
Expected: Top of file shows `import ... from 'three'` statements (externalized), not inlined three.js code.

- [ ] **Step 6: Verify demo still works**

Run: `cd client/web && pnpm dev`
Expected: Demo works as before (Vite dev mode ignores library config).

- [ ] **Step 7: Commit**

```
feat: configure shine-web as library bundle with Vite library mode
```

---

## Task 7: Verify end-to-end and clean build

**Files:**
- No new files

- [ ] **Step 1: Clean build from scratch**

Run:
```bash
cd client/web && rm -rf node_modules dist pkg && pnpm install && pnpm wasm && pnpm build:lib
```
Expected: All steps succeed. `dist/shine-web.js` produced.

- [ ] **Step 2: Inspect bundle exports**

Run: `head -30 client/web/dist/shine-web.js`
Expected: File starts with ESM imports from `three` and exports `createHexMeshViewer`, `defaultParams`, `paramsToConfigJson`, `buildHexMesh`.

- [ ] **Step 3: Verify native Rust workspace unaffected**

Run: `cargo check && cargo test -p shine-game`
Expected: All pass.

- [ ] **Step 4: Commit**

```
chore: verify clean shine-web library build
```

---

## Summary

After completing all tasks, the `client/web/` package will be:

| Concern | How it's handled |
|---------|-----------------|
| **Library API** | `createHexMeshViewer(container, options)` returns a handle with `update()`, `getParams()`, `dispose()` |
| **Bundle format** | Single ES module at `dist/shine-web.js` |
| **Three.js** | Peer dependency, not bundled |
| **WASM** | Side-loaded from `pkg/`, consumer passes URL via `wasmUrl` option if needed |
| **Controls (lil-gui)** | Not in the library bundle — demo-only. Consumers build their own UI using `MeshParams` + `update()` |
| **Params** | Pure `params.ts` — `MeshParams`, `defaultParams()`, `paramsToConfigJson()` — no GUI dependency |
| **Lifecycle** | Full cleanup via `dispose()` — stops animation, removes canvas, frees GPU resources, disconnects ResizeObserver |
| **Framework integration** | Container-based mounting works with any framework's ref/bind pattern |
| **Error handling** | `createHexMeshViewer()` and `update()` throw on invalid params — consumers should catch if needed |

### Svelte 5 usage example:

```svelte
<script>
    import { onMount } from 'svelte';
    import { createHexMeshViewer } from 'shine-web';
    import type { HexMeshViewerHandle } from 'shine-web';

    let { params = {} } = $props();
    let container = $state<HTMLElement>();
    let viewer = $state<HexMeshViewerHandle>();

    onMount(() => {
        let disposed = false;
        createHexMeshViewer(container!, {
            params,
            wasmUrl: '/wasm/shine_game_bg.wasm'
        }).then(v => {
            if (disposed) { v.dispose(); return; }
            viewer = v;
        });
        return () => { disposed = true; viewer?.dispose(); };
    });

    $effect(() => {
        if (viewer && params) viewer.update(params);
    });
</script>

<div bind:this={container} style="width:100%;height:100%" />
```

### React usage example:

```tsx
import { useEffect, useRef } from 'react';
import { createHexMeshViewer, type HexMeshViewerHandle, type MeshParams } from 'shine-web';

function HexMesh({ params }: { params?: Partial<MeshParams> }) {
    const ref = useRef<HTMLDivElement>(null);
    const viewerRef = useRef<HexMeshViewerHandle | null>(null);

    useEffect(() => {
        let disposed = false;
        createHexMeshViewer(ref.current!, { params }).then(v => {
            if (disposed) { v.dispose(); return; }
            viewerRef.current = v;
        });
        return () => {
            disposed = true;
            viewerRef.current?.dispose();
            viewerRef.current = null;
        };
    }, []);

    useEffect(() => {
        if (params) viewerRef.current?.update(params);
    }, [params]);

    return <div ref={ref} style={{ width: '100%', height: '100%' }} />;
}
```
