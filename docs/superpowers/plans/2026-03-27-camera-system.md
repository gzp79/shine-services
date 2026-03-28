# Camera System Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Camera class with orbit controls, world position tracking, center dot visualization, and world repositioning system with event-driven architecture.

**Architecture:** Hybrid event-driven pattern with GameEvents bus for coordination, GameSystem interface for orchestrators, and RenderContext for rendering infrastructure. Camera calculates world position, WorldReferenceSystem monitors distance and triggers repositioning, World and Camera listen to events and apply transforms.

**Tech Stack:** TypeScript, Three.js, OrbitControls, EventTarget API

---

## File Structure

**New Files:**
- `client/web/src/game-events.ts` - Event bus factory, types, and type-safe helpers
- `client/web/src/game-system.ts` - GameSystem interface for orchestrators
- `client/web/src/world/hex-utils.ts` - Hex coordinate conversion utilities
- `client/web/src/render-context.ts` - Rendering infrastructure (scene, renderer, resize)
- `client/web/src/camera/camera.ts` - Camera with controls and world position tracking
- `client/web/src/systems/world-reference-system.ts` - Monitors camera and triggers repositioning

**Modified Files:**
- `client/web/src/world/world.ts` - Add events parameter, listen to world reference changed
- `client/web/src/game.ts` - Refactor to use RenderContext, Camera, and systems

---

## Task 1: GameEvents Infrastructure

**Files:**
- Create: `client/web/src/game-events.ts`

- [ ] **Step 1: Create event types and factory**

Create `client/web/src/game-events.ts`:

```typescript
import { ChunkId } from './world/types';

// Event types
export type WorldReferenceChangedEvent = {
    oldCenter: ChunkId;
    newCenter: ChunkId;
    oldPosition: [number, number];
    newPosition: [number, number];
    getDelta(): [number, number];
};

export type ViewportResizeEvent = {
    width: number;
    height: number;
};

// Factory
export function createGameEvents(): EventTarget {
    return new EventTarget();
}

// Dispatch helpers
export function dispatchWorldReferenceChanged(
    target: EventTarget,
    oldCenter: ChunkId,
    newCenter: ChunkId,
    oldPosition: [number, number],
    newPosition: [number, number]
): void {
    const event: WorldReferenceChangedEvent = {
        oldCenter,
        newCenter,
        oldPosition,
        newPosition,
        getDelta() {
            return [oldPosition[0] - newPosition[0], oldPosition[1] - newPosition[1]];
        }
    };
    target.dispatchEvent(new CustomEvent('worldreferencechanged', { detail: event }));
}

export function dispatchViewportResize(
    target: EventTarget,
    width: number,
    height: number
): void {
    const event: ViewportResizeEvent = { width, height };
    target.dispatchEvent(new CustomEvent('viewportresize', { detail: event }));
}

// Listener helpers
export function onWorldReferenceChanged(
    target: EventTarget,
    handler: (event: WorldReferenceChangedEvent) => void
): () => void {
    const listener = (e: Event) => {
        const customEvent = e as CustomEvent<WorldReferenceChangedEvent>;
        handler(customEvent.detail);
    };
    target.addEventListener('worldreferencechanged', listener);
    return () => target.removeEventListener('worldreferencechanged', listener);
}

export function onViewportResize(
    target: EventTarget,
    handler: (event: ViewportResizeEvent) => void
): () => void {
    const listener = (e: Event) => {
        const customEvent = e as CustomEvent<ViewportResizeEvent>;
        handler(customEvent.detail);
    };
    target.addEventListener('viewportresize', listener);
    return () => target.removeEventListener('viewportresize', listener);
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd client/web && pnpm run lint:build`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add client/web/src/game-events.ts
git commit -m "feat: add GameEvents infrastructure with type-safe helpers"
```

---

## Task 2: GameSystem Interface

**Files:**
- Create: `client/web/src/game-system.ts`

- [ ] **Step 1: Create GameSystem interface**

Create `client/web/src/game-system.ts`:

```typescript
export interface GameSystem {
    update(deltaTime: number): void;
    destroy(): void;
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd client/web && pnpm run lint:build`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add client/web/src/game-system.ts
git commit -m "feat: add GameSystem interface"
```

---

## Task 3: Hex Coordinate Utilities

**Files:**
- Create: `client/web/src/world/hex-utils.ts`

**Note:** This is a deliberate deviation from spec. Spec indicated adding utilities to `world/types.ts`, but creating separate `hex-utils.ts` provides better separation of concerns (pure math utilities vs type definitions). This is an architectural improvement that doesn't affect functionality.

- [ ] **Step 1: Create hex utility functions**

Create `client/web/src/world/hex-utils.ts`:

```typescript
import { ChunkId } from './types';

const CHUNK_WORLD_SIZE = 1000;

/**
 * Convert world coordinates to the ChunkId containing that position.
 * Uses pointy-top hex grid (matching Rust implementation).
 */
export function worldPositionToChunkId(worldX: number, worldY: number): ChunkId {
    // Inverse of hex-to-world: world_coordinate in Rust
    // For pointy-top hexes:
    //   x = hex_size * (sqrt(3) * q + sqrt(3)/2 * r)
    //   y = hex_size * (3/2 * r)

    const sqrt3 = Math.sqrt(3);
    const size = CHUNK_WORLD_SIZE;

    // From y, solve for r
    const r = (2/3) * (worldY / size);

    // From x and r, solve for q
    const q = (worldX / size) / sqrt3 - r / 2;

    // Round to nearest hex using cube coordinates
    return roundToHex(q, r);
}

/**
 * Convert ChunkId to its world position (hex center).
 */
export function chunkIdToWorldPosition(chunkId: ChunkId): [number, number] {
    const sqrt3 = Math.sqrt(3);
    const size = CHUNK_WORLD_SIZE;

    // Pointy-top hex to world
    const x = size * (sqrt3 * chunkId.q + (sqrt3 / 2) * chunkId.r);
    const y = size * (3 / 2) * chunkId.r;

    return [x, y];
}

/**
 * Round fractional axial coordinates to nearest hex.
 * Uses cube coordinate rounding algorithm.
 */
function roundToHex(q: number, r: number): ChunkId {
    // Convert axial to cube
    const s = -q - r;

    // Round all three
    let rq = Math.round(q);
    let rr = Math.round(r);
    let rs = Math.round(s);

    // Find largest rounding error
    const q_diff = Math.abs(rq - q);
    const r_diff = Math.abs(rr - r);
    const s_diff = Math.abs(rs - s);

    // Reset the coordinate with largest error to maintain q + r + s = 0
    if (q_diff > r_diff && q_diff > s_diff) {
        rq = -rr - rs;
    } else if (r_diff > s_diff) {
        rr = -rq - rs;
    }

    // Convert cube back to axial
    return new ChunkId(rq, rr);
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd client/web && pnpm run lint:build`
Expected: No errors

- [ ] **Step 3: Manual test hex utilities in browser console**

Add temporary test code to `game.ts` init() method, after the chunk loading code and before the animation loop:
```typescript
import { worldPositionToChunkId, chunkIdToWorldPosition } from './world/hex-utils';

// In Game.init(), add after world.loadChunk calls:
// Test origin - world position (0,0) should map to ChunkId.ORIGIN (0,0)
console.log('Origin test:', worldPositionToChunkId(0, 0));
// Expected: ChunkId { q: 0, r: 0 }

// Test round-trip - ChunkId should convert to world pos and back unchanged
const testId = new ChunkId(5, 3);
const pos = chunkIdToWorldPosition(testId);
const backId = worldPositionToChunkId(pos[0], pos[1]);
console.log('Round-trip test:', testId, '->', pos, '->', backId);
// Expected: same q=5, r=3 after round-trip
```

Run: `cd client/web && pnpm dev`
Open browser, check console for correct output
Remove test code and import after verification

- [ ] **Step 4: Commit**

```bash
git add client/web/src/world/hex-utils.ts
git commit -m "feat: add hex coordinate conversion utilities"
```

---

## Task 4: RenderContext

**Files:**
- Create: `client/web/src/render-context.ts`

- [ ] **Step 1: Create RenderContext class**

Create `client/web/src/render-context.ts`:

```typescript
import * as THREE from 'three';
import { dispatchViewportResize } from './game-events';

export class RenderContext {
    readonly scene: THREE.Scene;
    readonly renderer: THREE.WebGLRenderer;
    readonly domElement: HTMLElement;
    private readonly resizeObserver: ResizeObserver;
    private _width: number;
    private _height: number;

    get width(): number {
        return this._width;
    }

    get height(): number {
        return this._height;
    }

    constructor(container: HTMLElement, private readonly events: EventTarget) {
        this.domElement = container;

        // Create scene
        this.scene = new THREE.Scene();
        this.scene.background = new THREE.Color(0x1a1a2e);

        // Create renderer
        const width = container.clientWidth;
        const height = container.clientHeight;
        this._width = width;
        this._height = height;

        this.renderer = new THREE.WebGLRenderer({ antialias: true });
        this.renderer.setSize(width, height);
        this.renderer.setPixelRatio(window.devicePixelRatio);
        container.appendChild(this.renderer.domElement);

        // Lighting
        const ambient = new THREE.AmbientLight(0xffffff, 0.6);
        this.scene.add(ambient);

        const directional = new THREE.DirectionalLight(0xffffff, 0.8);
        directional.position.set(1000, -500, 3000);
        this.scene.add(directional);

        // Resize handling
        this.resizeObserver = new ResizeObserver(() => {
            const w = container.clientWidth;
            const h = container.clientHeight;
            this._width = w;
            this._height = h;
            this.renderer.setSize(w, h);
            dispatchViewportResize(this.events, w, h);
        });
        this.resizeObserver.observe(container);
    }

    render(camera: THREE.PerspectiveCamera): void {
        this.renderer.render(this.scene, camera);
    }

    destroy(): void {
        this.resizeObserver.disconnect();
        this.renderer.dispose();
        this.renderer.domElement.remove();
    }
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd client/web && pnpm run lint:build`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add client/web/src/render-context.ts
git commit -m "feat: add RenderContext for rendering infrastructure"
```

---

## Task 5: Camera Class

**Files:**
- Create: `client/web/src/camera/camera.ts`

- [ ] **Step 1: Create Camera class**

Create `client/web/src/camera/camera.ts`:

```typescript
import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { onViewportResize, onWorldReferenceChanged } from '../game-events';
import type { RenderContext } from '../render-context';

export class Camera {
    readonly camera: THREE.PerspectiveCamera;
    readonly worldPosition: [number, number] = [0, 0];
    private readonly controls: OrbitControls;
    private readonly centerDot: THREE.Points;
    private readonly eventCleanups: (() => void)[] = [];

    constructor(renderContext: RenderContext, events: EventTarget) {
        // Create camera
        const aspect = renderContext.width / renderContext.height;
        this.camera = new THREE.PerspectiveCamera(50, aspect, 1, 50000);
        this.camera.up.set(0, 0, 1);
        this.camera.position.set(0, -1200, 2000);
        this.camera.lookAt(0, 0, 0);

        // Create controls
        this.controls = new OrbitControls(this.camera, renderContext.domElement);
        this.controls.target.set(0, 0, 0);
        this.controls.enableDamping = true;
        this.controls.dampingFactor = 0.1;
        this.controls.update();

        // Create center dot
        const dotGeometry = new THREE.BufferGeometry();
        const dotPosition = new Float32Array([0, 0, 0.1]);
        dotGeometry.setAttribute('position', new THREE.BufferAttribute(dotPosition, 3));
        const dotMaterial = new THREE.PointsMaterial({ color: 0xff0000, size: 10 });
        this.centerDot = new THREE.Points(dotGeometry, dotMaterial);
        renderContext.scene.add(this.centerDot);

        // Subscribe to events
        this.eventCleanups.push(
            onViewportResize(events, (event) => {
                this.camera.aspect = event.width / event.height;
                this.camera.updateProjectionMatrix();
            })
        );

        this.eventCleanups.push(
            onWorldReferenceChanged(events, (event) => {
                const delta = event.getDelta();
                this.camera.position.x += delta[0];
                this.camera.position.y += delta[1];
            })
        );
    }

    get showCenterDot(): boolean {
        return this.centerDot.visible;
    }

    set showCenterDot(value: boolean) {
        this.centerDot.visible = value;
    }

    update(): void {
        // Update controls
        this.controls.update();

        // Calculate world position (camera forward ∩ XY plane at z=0)
        const camPos = this.camera.position;
        const forward = new THREE.Vector3(0, 0, -1).applyQuaternion(this.camera.quaternion);

        // Check if camera is parallel to plane
        if (Math.abs(forward.z) < 0.001) {
            // Fallback: use camera XY position
            this.worldPosition[0] = camPos.x;
            this.worldPosition[1] = camPos.y;
        } else {
            // Intersect with z=0 plane: point = camPos + t * forward, where t = -camPos.z / forward.z
            const t = -camPos.z / forward.z;
            this.worldPosition[0] = camPos.x + t * forward.x;
            this.worldPosition[1] = camPos.y + t * forward.y;
        }

        // Update center dot position
        const posAttr = this.centerDot.geometry.attributes.position as THREE.BufferAttribute;
        posAttr.setXYZ(0, this.worldPosition[0], this.worldPosition[1], 0.1);
        posAttr.needsUpdate = true;
    }

    destroy(): void {
        // Remove center dot
        this.centerDot.parent?.remove(this.centerDot);
        this.centerDot.geometry.dispose();
        (this.centerDot.material as THREE.Material).dispose();

        // Dispose controls
        this.controls.dispose();

        // Unsubscribe from events
        for (const cleanup of this.eventCleanups) {
            cleanup();
        }
        this.eventCleanups.length = 0;
    }
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd client/web && pnpm run lint:build`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add client/web/src/camera/camera.ts
git commit -m "feat: add Camera class with world position tracking"
```

---

## Task 6: WorldReferenceSystem

**Files:**
- Create: `client/web/src/systems/world-reference-system.ts`

- [ ] **Step 1: Create WorldReferenceSystem class**

Create `client/web/src/systems/world-reference-system.ts`:

```typescript
import type { GameSystem } from '../game-system';
import type { Camera } from '../camera/camera';
import { ChunkId } from '../world/types';
import { worldPositionToChunkId, chunkIdToWorldPosition } from '../world/hex-utils';
import { dispatchWorldReferenceChanged } from '../game-events';

const CHUNK_WORLD_SIZE = 1000;
const REPOSITION_THRESHOLD = 5 * CHUNK_WORLD_SIZE;

export class WorldReferenceSystem implements GameSystem {
    private referenceCenter: ChunkId = ChunkId.ORIGIN;

    constructor(
        private readonly camera: Camera,
        private readonly events: EventTarget
    ) {}

    update(_deltaTime: number): void {
        // Check camera distance from origin
        const [x, y] = this.camera.worldPosition;
        const distance = Math.sqrt(x * x + y * y);

        if (distance > REPOSITION_THRESHOLD) {
            // Calculate new reference center
            const newCenter = worldPositionToChunkId(x, y);

            // Calculate old and new positions
            const oldPosition = chunkIdToWorldPosition(this.referenceCenter);
            const newPosition = chunkIdToWorldPosition(newCenter);

            // Dispatch event
            dispatchWorldReferenceChanged(
                this.events,
                this.referenceCenter,
                newCenter,
                oldPosition,
                newPosition
            );

            // Update reference
            this.referenceCenter = newCenter;
        }
    }

    destroy(): void {
        // No cleanup needed
    }
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd client/web && pnpm run lint:build`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add client/web/src/systems/world-reference-system.ts
git commit -m "feat: add WorldReferenceSystem for camera-based repositioning"
```

---

## Task 7: Update World for Events

**Files:**
- Modify: `client/web/src/world/world.ts`

- [ ] **Step 1: Add events parameter and event handling**

Update `client/web/src/world/world.ts`:

```typescript
import { WasmWorld } from '#wasm';
import * as THREE from 'three';
import { Chunk } from './chunk';
import { ChunkId } from './types';
import { onWorldReferenceChanged } from '../game-events';

export class World {
    readonly group = new THREE.Group();
    private readonly wasm: WasmWorld;
    private readonly chunks = new Map<string, Chunk>();
    private _referenceCenter = ChunkId.ORIGIN;
    private readonly eventCleanups: (() => void)[] = [];

    get referenceCenter(): ChunkId {
        return this._referenceCenter;
    }

    constructor(events: EventTarget) {
        this.wasm = new WasmWorld();

        // Subscribe to world reference changed
        const cleanup = onWorldReferenceChanged(events, (event) => {
            const delta = event.getDelta();
            for (const chunk of this.chunks.values()) {
                chunk.group.position.x += delta[0];
                chunk.group.position.y += delta[1];
            }
            this._referenceCenter = event.newCenter;
        });
        this.eventCleanups.push(cleanup);
    }

    loadChunk(id: ChunkId): Chunk {
        const key = id.key();
        const existing = this.chunks.get(key);
        if (existing) return existing;

        this.wasm.init_chunk(id.q, id.r);

        const chunk = new Chunk(this.wasm, id);
        chunk.buildMesh(this._referenceCenter);
        this.group.add(chunk.group);
        this.chunks.set(key, chunk);
        return chunk;
    }

    unloadChunk(id: ChunkId): void {
        const key = id.key();
        const chunk = this.chunks.get(key);
        if (!chunk) return;
        this.group.remove(chunk.group);
        chunk.disposeMesh();
        this.chunks.delete(key);
        this.wasm.remove_chunk(id.q, id.r);
    }

    dispose(): void {
        // Cleanup event listeners
        for (const cleanup of this.eventCleanups) {
            cleanup();
        }
        this.eventCleanups.length = 0;

        // Dispose chunks
        for (const chunk of this.chunks.values()) {
            this.group.remove(chunk.group);
            chunk.disposeMesh();
        }
        this.chunks.clear();
        this.wasm.free();
    }
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd client/web && pnpm run lint:build`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add client/web/src/world/world.ts
git commit -m "feat: update World to handle world reference changed events"
```

---

## Task 8: Refactor Game

**Files:**
- Modify: `client/web/src/game.ts`

- [ ] **Step 1: Refactor Game to use new architecture**

Update `client/web/src/game.ts`:

```typescript
import init from '#wasm';
import wasmUrl from '#wasm-bin';
import * as THREE from 'three';
import { ChunkId } from './world/types';
import { World } from './world/world';
import { RenderContext } from './render-context';
import { Camera } from './camera/camera';
import { createGameEvents } from './game-events';
import { WorldReferenceSystem } from './systems/world-reference-system';
import type { GameSystem } from './game-system';

class Game {
    private readonly events: EventTarget;
    private readonly renderContext: RenderContext;
    private readonly camera: Camera;
    private readonly world: World;
    private readonly systems: GameSystem[] = [];
    private animationId = 0;
    private lastTime = 0;

    constructor(private readonly container: HTMLElement) {
        this.events = createGameEvents();
        this.renderContext = new RenderContext(container, this.events);
        this.camera = new Camera(this.renderContext, this.events);
        this.world = new World(this.events);

        // Register systems
        this.systems.push(new WorldReferenceSystem(this.camera, this.events));

        // Add world to scene
        this.renderContext.scene.add(this.world.group);
    }

    init(): void {
        this.world.loadChunk(ChunkId.ORIGIN);
        for (const neighbor of ChunkId.ORIGIN.neighbors()) {
            this.world.loadChunk(neighbor);
        }

        // Debug: circle with radius 1000 at origin (XY plane)
        const circleGeom = new THREE.RingGeometry(998, 1000, 64);
        const circleMat = new THREE.MeshBasicMaterial({ color: 0xff0000, side: THREE.DoubleSide });
        const circle = new THREE.Mesh(circleGeom, circleMat);
        circle.position.z = 0.1;
        this.renderContext.scene.add(circle);

        this.lastTime = performance.now();
        this.animationId = requestAnimationFrame((t) => this.animate(t));
    }

    private animate(currentTime: number): void {
        this.animationId = requestAnimationFrame((t) => this.animate(t));

        const deltaTime = (currentTime - this.lastTime) / 1000;
        this.lastTime = currentTime;

        // Update camera
        this.camera.update();

        // Update systems
        for (const system of this.systems) {
            system.update(deltaTime);
        }

        // Render
        this.renderContext.render(this.camera.camera);
    }

    destroy(): void {
        cancelAnimationFrame(this.animationId);
        this.camera.destroy();
        for (const system of this.systems) {
            system.destroy();
        }
        this.world.dispose();
        this.renderContext.destroy();
    }
}

export async function createGame(container: HTMLElement): Promise<Game> {
    await init(wasmUrl);
    const game = new Game(container);
    game.init();
    return game;
}
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd client/web && pnpm run lint:build`
Expected: No errors

- [ ] **Step 3: Commit**

```bash
git add client/web/src/game.ts
git commit -m "refactor: update Game to use RenderContext, Camera, and systems"
```

---

## Task 9: Integration Testing

**Files:**
- Test: Manual browser testing
- Modify: `client/web/src/game.ts` (temporarily expose game for testing)

- [ ] **Step 1: Expose game on window for testing**

Add to `game.ts` createGame function, before return:
```typescript
// Temporary: expose for testing
(window as any).game = game;
(window as any).camera = game['camera']; // Access private field
```

- [ ] **Step 2: Build and run the application**

Run: `cd client/web && pnpm dev`
Open browser at local dev URL

- [ ] **Step 3: Test basic camera functionality**

- Verify camera controls work (left mouse drag to orbit, right mouse to pan, scroll to zoom)
- Verify red center dot is visible on the ground
- Verify chunks render correctly
- Verify window resize updates camera aspect ratio

- [ ] **Step 4: Test world position tracking**

Open browser console:
```javascript
console.log('Camera world position:', camera.worldPosition);
// Should show [x, y] coordinates that update as you move the camera
```
Move camera around using mouse controls, verify worldPosition updates in real-time

- [ ] **Step 5: Test center dot toggle**

Open browser console:
```javascript
camera.showCenterDot = false;
// Verify dot disappears

camera.showCenterDot = true;
// Verify dot reappears
```

- [ ] **Step 6: Test world repositioning**

Verify repositioning works without visual glitches.

First, capture current state in console:
```javascript
console.log('Before move - worldPosition:', camera.worldPosition);
console.log('Before move - camera position:', camera.camera.position.toArray());
```

Move far quickly using console to set camera position beyond threshold:
```javascript
// Move camera to position that's ~8485 meters from origin (>5000 threshold)
camera.camera.position.set(6000, -6000, 2000);
camera.camera.lookAt(0, 0, 0);
```

Wait a few animation frames (1-2 seconds), then verify in console:
```javascript
console.log('After reposition - worldPosition:', camera.worldPosition);
console.log('After reposition - camera position:', camera.camera.position.toArray());
console.log('Center dot position:', camera['centerDot'].geometry.attributes.position.array);
```

Expected results:
- `worldPosition` should be close to (6000, -6000) before reposition trigger
- Camera position should have changed after reposition (delta applied)
- Visual scene should show NO pop or jump
- Chunks maintain their visual positions
- Center dot stays visually at the same ground intersection point
- Distance from origin should now be < 5000 meters

If repositioning doesn't occur, increase distance further or wait longer for update cycle.

- [ ] **Step 7: Verify no console errors**

Check browser console for errors or warnings
Expected: Clean console, no errors (besides temporary test exposure warnings if any)

- [ ] **Step 8: Remove temporary test exposure**

Remove the window exposure lines added in Step 1 from `game.ts`

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "test: verify camera system integration"
```

---

## Task 10: Debug Circle Removal (Optional)

**Files:**
- Modify: `client/web/src/game.ts`

- [ ] **Step 1: Remove debug circle (optional cleanup)**

If the red debug circle at origin is no longer needed, remove these lines from `game.ts` init():

```typescript
// Debug: circle with radius 1000 at origin (XY plane)
const circleGeom = new THREE.RingGeometry(998, 1000, 64);
const circleMat = new THREE.MeshBasicMaterial({ color: 0xff0000, side: THREE.DoubleSide });
const circle = new THREE.Mesh(circleGeom, circleMat);
circle.position.z = 0.1;
this.renderContext.scene.add(circle);
```

- [ ] **Step 2: Verify still works**

Run: `cd client/web && pnpm dev`
Verify application still works without the circle

- [ ] **Step 3: Commit**

```bash
git add client/web/src/game.ts
git commit -m "chore: remove debug circle from Game"
```

---

## Notes

**Testing Strategy:**
- TypeScript compilation at each step
- Manual browser testing for visual verification
- Console testing for properties and behavior
- Integration test covers all interactions

**DRY Compliance:**
- Event helpers eliminate repeated CustomEvent boilerplate
- RenderContext centralizes scene/renderer setup
- Camera encapsulates all camera-related logic
- Systems pattern enables reusable orchestration

**YAGNI Compliance:**
- No unused features or abstractions
- Minimal interfaces (GameSystem has only required methods)
- Direct implementations without over-engineering

**Potential Issues:**
- Hex math accuracy: May need fine-tuning if repositioning doesn't align perfectly. Current manual test (Task 3 Step 3) verifies basic round-trip but doesn't test edge cases (negative coords, 60-degree angles). If repositioning shows visual misalignment, expand testing to verify hex math matches Rust implementation.
- Camera parallel to plane: Fallback may need adjustment based on UX testing
- Performance: Monitor event dispatch frequency if frame rate drops
- Repositioning thrashing: If camera stays near threshold boundary (exactly at 5000m), could trigger repeated repositions. Monitor during testing in Task 9.

**Future Extensions:**
- Expose game/camera on window during dev for easier console debugging
- Add automated tests for hex utilities with known coordinate conversions
- Add UI controls for camera.showCenterDot toggle
- Add ChunkLoadingSystem to dynamically load/unload chunks based on camera position
