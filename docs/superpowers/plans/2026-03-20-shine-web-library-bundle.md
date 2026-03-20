# Shine-Web Library Bundle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor the hex mesh viewer at `client/web/` into a reusable library called "shine-web" that can be mounted into any container element and consumed as a Svelte or React component.

**Architecture:** Extract the core Three.js rendering into a container-aware `createHexMeshViewer(container, options)` API exported from `src/lib.ts`. The existing `main.ts` becomes a thin standalone demo that uses this API. Vite library mode produces an ES module bundle with `three` as a peer dependency. WASM is side-loaded (not inlined) so consumers control asset serving.

**Tech Stack:** TypeScript, Vite (library mode), three.js (peer dep), wasm-bindgen, lil-gui (optional/demo-only)

---

## File Map

| File | Action | Responsibility |
|------|--------|---------------|
| `client/web/package.json` | Modify | Rename to `shine-web`, add `main`/`module`/`types`/`exports` fields, move `three` to peerDeps |
| `client/web/tsconfig.json` | Modify | Add `declaration: true` for type output |
| `client/web/vite.config.ts` | Modify | Add `build.lib` config for library mode |
| `client/web/src/lib.ts` | Create | Public API: `createHexMeshViewer()`, `HexMeshViewerHandle`, re-exports |
| `client/web/src/scene.ts` | Modify | Accept `container: HTMLElement`, use `ResizeObserver`, return `dispose()` |
| `client/web/src/mesh-builder.ts` | No change | Already pure (no DOM coupling) |
| `client/web/src/controls.ts` | Modify | Make `createControls` accept optional container for lil-gui placement |
| `client/web/src/main.ts` | Modify | Thin demo that imports from `./lib` |
| `client/web/index.html` | Modify | Update title to "Shine Web Demo" |

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

- [ ] **Step 3: Verify the file compiles**

Run: `cd client/web && npx tsc --noEmit`
Expected: No errors (main.ts will have errors since it hasn't been updated yet — that's fine, focus on scene.ts)

- [ ] **Step 4: Commit**

```
refactor: decouple scene.ts from global DOM, accept container element
```

---

## Task 2: Make controls container-aware

**Files:**
- Modify: `client/web/src/controls.ts`

### Context

`lil-gui` by default appends to `document.body`. For library use, the consumer may want the GUI in a specific container — or may not want lil-gui at all (they provide their own UI). Add an optional `container` parameter. This keeps the demo working and lets library consumers control placement.

- [ ] **Step 1: Add optional container parameter to `createControls`**

Update the function signature in `client/web/src/controls.ts`:

```typescript
export function createControls(
    params: MeshParams,
    onChange: () => void,
    container?: HTMLElement
): GUI {
    const gui = new GUI({ title: 'Hex Mesh', container });
    // ... rest unchanged
```

Only the first line of the function body changes — `new GUI({ title: 'Hex Mesh', container })`. Everything else stays the same.

- [ ] **Step 2: Commit**

```
refactor: make lil-gui controls accept optional container
```

---

## Task 3: Create the library entry point

**Files:**
- Create: `client/web/src/lib.ts`

### Context

This is the main public API. It wraps scene creation, WASM init, mesh generation, and lifecycle into a single `createHexMeshViewer()` function. Returns a handle with `update()`, `resize()`, and `dispose()` methods. Consumers pass a container element and optional initial params.

The WASM module exports `init(wasmUrl?)` and `generate_mesh(configJson)`. The `init()` function accepts an optional URL/Response for the `.wasm` file, which lets consumers control where it's served from.

- [ ] **Step 1: Create `lib.ts`**

Create `client/web/src/lib.ts`:

```typescript
import init, { generate_mesh } from '../pkg/shine_game.js';

import { SceneContext, animate, createScene } from './scene';
import { HexMeshGroup, MeshData, buildHexMesh } from './mesh-builder';
import { MeshParams, defaultParams, paramsToConfigJson } from './controls';

export type { MeshData, MeshParams, HexMeshGroup };
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

- [ ] **Step 2: Verify the file compiles**

Run: `cd client/web && npx tsc --noEmit`
Expected: No errors on `lib.ts` (main.ts may still have issues, that's next task)

- [ ] **Step 3: Commit**

```
feat: add library entry point with createHexMeshViewer API
```

---

## Task 4: Update main.ts to use lib.ts

**Files:**
- Modify: `client/web/src/main.ts`
- Modify: `client/web/index.html`

### Context

The standalone demo should now use the library API, proving it works end-to-end. It creates a full-page container div, mounts the viewer, and adds lil-gui controls that call `viewer.update()`.

- [ ] **Step 1: Replace main.ts**

Replace `client/web/src/main.ts` with:

```typescript
import { createHexMeshViewer, defaultParams } from './lib';
import { createControls } from './controls';

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

## Task 5: Configure Vite library mode and package metadata

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
    "dependencies": {
        "lil-gui": "^0.20"
    },
    "devDependencies": {
        "@eslint/js": "^9.37.0",
        "@stylistic/eslint-plugin": "^5.4.0",
        "@trivago/prettier-plugin-sort-imports": "5.2.2",
        "@types/three": "^0.172",
        "eslint": "^9.24.0",
        "eslint-config-prettier": "^10.1.2",
        "jiti": "^2.6.1",
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
- `three` moved to `peerDependencies` (with `>=0.160` for broad compat) and kept in `devDependencies` for the demo
- Added `main`, `module`, `types`, `exports`, `files` fields
- Added `build:lib` script that emits declarations + builds library
- `lil-gui` stays in `dependencies` since `controls.ts` imports it (consumers who skip controls won't import it and tree-shaking handles it)

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
            external: ['three', /^three\//]
        }
    }
});
```

Notes:
- `external: ['three', /^three\//]` externalizes both `three` and `three/addons/...` imports
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

Changes: Added `declaration: true` and `declarationMap: true` for `.d.ts` output.

- [ ] **Step 4: Verify library build**

Run: `cd client/web && pnpm build:lib`
Expected: Produces `dist/shine-web.js` (ES module), `dist/lib.d.ts`, and related declaration files. The bundle should NOT contain three.js code (it's externalized).

- [ ] **Step 5: Verify the bundle doesn't include three.js**

Run: `grep -c "THREE" client/web/dist/shine-web.js`
Expected: 0 (or very few — only import statements, no three.js library code)

- [ ] **Step 6: Verify demo still works**

Run: `cd client/web && pnpm dev`
Expected: Demo works as before (Vite dev mode ignores library config)

- [ ] **Step 7: Commit**

```
feat: configure shine-web as library bundle with Vite library mode
```

---

## Task 6: Verify end-to-end and clean build

**Files:**
- No new files

- [ ] **Step 1: Clean build from scratch**

Run:
```bash
cd client/web && rm -rf node_modules dist pkg && pnpm install && pnpm wasm && pnpm build:lib
```
Expected: All steps succeed. `dist/shine-web.js` produced.

- [ ] **Step 2: Verify bundle exports**

Run:
```bash
node -e "import('./client/web/dist/shine-web.js').then(m => console.log(Object.keys(m)))"
```
Expected: Lists exports including `createHexMeshViewer`, `defaultParams`, `paramsToConfigJson`, `buildHexMesh`

- [ ] **Step 3: Verify native Rust workspace unaffected**

Run: `cargo check && cargo test -p shine-game`
Expected: All pass

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
| **Controls (lil-gui)** | Optional — library works without them; importable separately for quick demos |
| **Lifecycle** | Full cleanup via `dispose()` — stops animation, removes canvas, frees GPU resources, disconnects ResizeObserver |
| **Framework integration** | Container-based mounting works with any framework's ref/bind pattern |

### Svelte usage example:

```svelte
<script>
    import { onMount, onDestroy } from 'svelte';
    import { createHexMeshViewer } from 'shine-web';

    let container;
    let viewer;
    export let params = {};

    onMount(async () => {
        viewer = await createHexMeshViewer(container, {
            params,
            wasmUrl: '/wasm/shine_game_bg.wasm'
        });
    });

    onDestroy(() => viewer?.dispose());

    $: if (viewer) viewer.update(params);
</script>

<div bind:this={container} style="width:100%;height:100%" />
```

### React usage example:

```tsx
import { useEffect, useRef } from 'react';
import { createHexMeshViewer, HexMeshViewerHandle, MeshParams } from 'shine-web';

function HexMesh({ params }: { params?: Partial<MeshParams> }) {
    const ref = useRef<HTMLDivElement>(null);
    const viewerRef = useRef<HexMeshViewerHandle | null>(null);

    useEffect(() => {
        createHexMeshViewer(ref.current!, { params }).then(v => {
            viewerRef.current = v;
        });
        return () => {
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
