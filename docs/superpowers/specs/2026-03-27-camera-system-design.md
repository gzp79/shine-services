# Camera System Design

## Overview

Add a Camera class to the client web that manages the THREE.js camera with orbit controls, calculates the center world position (camera forward ray intersection with XY plane), displays an optional debug center dot sprite, and triggers world repositioning when the camera moves too far from the origin. This design introduces a general GameSystem pattern and event-driven architecture for coordinating game components.

## Architecture Principles

### Component Categories

**Resources**: Own game state and objects (Camera, World, RenderContext)
**Systems**: Orchestrate resources, implement cross-cutting logic (WorldReferenceSystem)
**Events**: Decouple communication between resources and systems (GameEvents)

### Communication Pattern

Hybrid event-driven approach:
- Direct references for performance-critical reads (WorldReferenceSystem reads `camera.worldPosition` each frame)
- Events for state changes and notifications (resize, world reference changes)
- Clear dependencies via constructor injection (no singletons except what Game creates)

## Core Components

### 1. GameEvents (`game-events.ts`)

Factory function and type-safe event helpers for game-wide event bus.

**Event Types:**
```typescript
type WorldReferenceChangedEvent = {
  oldCenter: ChunkId;
  newCenter: ChunkId;
  oldPosition: [number, number]; // world position of old reference center
  newPosition: [number, number]; // world position of new reference center
  getDelta(): [number, number];  // helper: returns oldPosition - newPosition
};

type ViewportResizeEvent = {
  width: number;
  height: number;
};
```

**API:**
```typescript
export function createGameEvents(): EventTarget;

// Type-safe dispatch helpers
export function dispatchWorldReferenceChanged(
  target: EventTarget,
  oldCenter: ChunkId,
  newCenter: ChunkId,
  oldPosition: [number, number],
  newPosition: [number, number]
): void;

export function dispatchViewportResize(
  target: EventTarget,
  width: number,
  height: number
): void;

// Type-safe listener helpers (return cleanup function)
export function onWorldReferenceChanged(
  target: EventTarget,
  handler: (event: WorldReferenceChangedEvent) => void
): () => void;

export function onViewportResize(
  target: EventTarget,
  handler: (event: ViewportResizeEvent) => void
): () => void;
```

**Implementation:**
- `dispatchWorldReferenceChanged` creates event detail object with `getDelta()` method
- `getDelta()` returns `[oldPosition[0] - newPosition[0], oldPosition[1] - newPosition[1]]`
- Listener helpers wrap `addEventListener` with typed CustomEvent extraction, return cleanup function that removes listener

### 2. GameSystem Interface (`game-system.ts`)

Standard interface for all game systems.

```typescript
export interface GameSystem {
  update(deltaTime: number): void;
  destroy(): void;
}
```

**Usage Pattern:**
- Game maintains `private readonly systems: GameSystem[] = []`
- Game calls `system.update(deltaTime)` for each system in animate loop
- Game calls `system.destroy()` for each system in Game.destroy()

### 3. RenderContext (`render-context.ts`)

Owns rendering resources and handles viewport lifecycle.

**API:**
```typescript
export class RenderContext {
  readonly scene: THREE.Scene;
  readonly renderer: THREE.WebGLRenderer;
  readonly domElement: HTMLElement;

  get width(): number;
  get height(): number;

  constructor(container: HTMLElement, events: EventTarget);
  render(camera: THREE.PerspectiveCamera): void;
  destroy(): void;
}
```

**Responsibilities:**
- Creates THREE.Scene with background color `0x1a1a2e`
- Creates THREE.WebGLRenderer with antialias, sets pixel ratio, appends canvas to container
- Sets up lighting: `AmbientLight(0xffffff, 0.6)` + `DirectionalLight(0xffffff, 0.8)` at position `(1000, -500, 3000)`
- Creates ResizeObserver on container, updates renderer size and dispatches `ViewportResizeEvent` on resize
- Provides `render(camera)` method that wraps `renderer.render(scene, camera)`

**Constructor Flow:**
1. Store container and events references
2. Create scene with background color
3. Create renderer, configure, append to container
4. Add lights to scene
5. Create ResizeObserver that:
   - Reads `container.clientWidth/clientHeight`
   - Calls `renderer.setSize(w, h)`
   - Dispatches `ViewportResizeEvent` via `dispatchViewportResize(events, w, h)`
6. Trigger initial resize

**Destroy:**
- Disconnect ResizeObserver
- Dispose renderer
- Remove canvas from DOM

### 4. Camera (`camera/camera.ts`)

Manages camera, controls, and world position calculation.

**API:**
```typescript
export class Camera {
  readonly camera: THREE.PerspectiveCamera;
  readonly worldPosition: [number, number]; // updated each frame
  showCenterDot: boolean; // property to toggle visibility

  constructor(renderContext: RenderContext, events: EventTarget);
  update(): void;
  destroy(): void;
}
```

**Responsibilities:**
- Owns THREE.PerspectiveCamera and OrbitControls
- Calculates world position (camera forward ray ∩ XY plane at z=0) each frame
- Displays optional debug center dot at world position
- Responds to viewport resize and world reference changes

**Constructor:**
- Creates `THREE.PerspectiveCamera(50, aspect, 1, 50000)` with `camera.up.set(0, 0, 1)` (Z-up)
- Initial position: `camera.position.set(0, -1200, 2000)`, `camera.lookAt(0, 0, 0)`
- Creates `OrbitControls(camera, renderContext.domElement)` with:
  - `target.set(0, 0, 0)`
  - `enableDamping = true`
  - `dampingFactor = 0.1`
- Creates center dot: `THREE.Points` with single vertex at origin, `PointsMaterial({ color: 0xff0000, size: 10 })`
- Adds center dot to `renderContext.scene`
- Subscribes to `onWorldReferenceChanged` and `onViewportResize` events (stores cleanup functions)
- Initializes `worldPosition` to `[0, 0]`, `showCenterDot` to `true`

**update() Method:**
1. Call `controls.update()` for damping
2. Calculate world position:
   - Get camera position and forward direction
   - Find intersection with XY plane (z=0): `point = cameraPos + t * forward` where `t = -cameraPos.z / forward.z`
   - If forward.z is ~0 (camera parallel to plane), use fallback (e.g., `[camera.position.x, camera.position.y]`)
   - Store result in `worldPosition` as `[point.x, point.y]`
3. Update center dot position: `centerDot.geometry.attributes.position.setXYZ(0, worldPosition[0], worldPosition[1], 0.1)`
4. Mark geometry for update: `centerDot.geometry.attributes.position.needsUpdate = true`

**showCenterDot Property:**
- Getter returns `centerDot.visible`
- Setter updates `centerDot.visible`

**Event Handlers:**
- `onViewportResize`: Updates `camera.aspect = width / height`, calls `camera.updateProjectionMatrix()`
- `onWorldReferenceChanged`: Applies `event.getDelta()` to camera position:
  ```typescript
  const delta = event.getDelta();
  this.camera.position.x += delta[0];
  this.camera.position.y += delta[1];
  ```

**destroy():**
- Remove center dot from scene
- Dispose center dot geometry and material
- Dispose OrbitControls
- Call cleanup functions for event listeners

### 5. WorldReferenceSystem (`systems/world-reference-system.ts`)

GameSystem that monitors camera position and triggers world repositioning.

**API:**
```typescript
export class WorldReferenceSystem implements GameSystem {
  constructor(camera: Camera, events: EventTarget);
  update(deltaTime: number): void;
  destroy(): void;
}
```

**Constants:**
```typescript
const CHUNK_WORLD_SIZE = 1000; // from existing code
const REPOSITION_THRESHOLD = 5 * CHUNK_WORLD_SIZE; // 5000 meters
```

**Responsibilities:**
- Monitors `camera.worldPosition` each frame
- Detects when distance from origin exceeds `REPOSITION_THRESHOLD`
- Calculates new reference center (ChunkId containing camera position)
- Emits `WorldReferenceChangedEvent` with old/new positions

**Constructor:**
- Stores references to `camera` and `events`
- Tracks current `referenceCenter: ChunkId` (initially `ChunkId.ORIGIN`)

**update(deltaTime) Method:**
1. Read `camera.worldPosition`
2. Calculate distance from origin: `Math.sqrt(x*x + y*y)`
3. If distance > `REPOSITION_THRESHOLD`:
   - Calculate new ChunkId: `worldPositionToChunkId(worldPosition[0], worldPosition[1])`
   - Calculate old position: `chunkIdToWorldPosition(referenceCenter)`
   - Calculate new position: `chunkIdToWorldPosition(newCenter)`
   - Dispatch `WorldReferenceChangedEvent` with old/new centers and positions
   - Update internal `referenceCenter` to new value

**Utility Functions Needed:**
```typescript
// Convert world coordinates to ChunkId (inverse of chunk world offset)
function worldPositionToChunkId(worldX: number, worldY: number): ChunkId;

// Convert ChunkId to world position (hex center)
function chunkIdToWorldPosition(chunkId: ChunkId): [number, number];
```

**Hex Grid Math:**
- Uses axial hex coordinate system (existing in ChunkId)
- World to hex: inverse of `AxialCoord.world_coordinate()` from Rust (pointy-top hex)
- Hex to world: apply hex-to-cartesian conversion with `CHUNK_WORLD_SIZE`

**destroy():**
- No cleanup needed (no resources owned)

### 6. World Updates (`world/world.ts`)

**Constructor Update:**
```typescript
export class World {
  // ... existing fields
  private readonly eventCleanup: (() => void)[] = [];

  constructor(events: EventTarget); // new parameter
}
```

**Changes:**
- Constructor accepts `events: EventTarget`
- Subscribes to `onWorldReferenceChanged`, stores cleanup function:
  ```typescript
  const cleanup = onWorldReferenceChanged(events, (event) => {
    const delta = event.getDelta();
    for (const chunk of this.chunks.values()) {
      chunk.group.position.x += delta[0];
      chunk.group.position.y += delta[1];
    }
    this._referenceCenter = event.newCenter;
  });
  this.eventCleanup.push(cleanup);
  ```

**dispose() Update:**
- Call all cleanup functions before disposing chunks:
  ```typescript
  for (const cleanup of this.eventCleanup) {
    cleanup();
  }
  this.eventCleanup.length = 0;
  ```

### 7. Game Updates (`game.ts`)

**New Structure:**
```typescript
class Game {
    private readonly events: EventTarget;
    private readonly renderContext: RenderContext;
    private readonly camera: Camera;
    private readonly world: World;
    private readonly systems: GameSystem[] = [];
    private animationId = 0;
    private lastTime = 0;

    constructor(private readonly container: HTMLElement);
    init(): void;
    private animate(currentTime: number): void;
    destroy(): void;
}
```

**Constructor:**
1. Create `events = createGameEvents()`
2. Create `renderContext = new RenderContext(container, events)`
3. Create `camera = new Camera(renderContext, events)`
4. Create `world = new World(events)`
5. Create and register systems: `systems.push(new WorldReferenceSystem(camera, events))`

**init() Method:**
- Load chunks (unchanged logic)
- Initialize lastTime: `lastTime = performance.now()`
- Start animation loop: `animationId = requestAnimationFrame((t) => this.animate(t))`

**animate(currentTime) Method:**
1. Calculate `deltaTime = (currentTime - lastTime) / 1000` (seconds)
2. Update `lastTime = currentTime`
3. Call `camera.update()`
4. For each system: call `system.update(deltaTime)`
5. Call `renderContext.render(camera.camera)`
6. Request next frame: `animationId = requestAnimationFrame((t) => this.animate(t))`

**destroy() Method:**
1. Cancel animation frame: `cancelAnimationFrame(animationId)`
2. Call `camera.destroy()`
3. For each system: call `system.destroy()`
4. Call `world.dispose()`
5. Call `renderContext.destroy()`

**Removed:**
- `scene`, `camera`, `renderer`, `controls`, `resizeObserver` fields (now in RenderContext and Camera)
- Manual lighting setup (now in RenderContext)
- Manual resize handling (now in RenderContext)
- Direct `controls.update()` call (now in Camera.update())

## Data Flow

### Initialization Flow

```
Game constructor
  → events = createGameEvents()
  → renderContext = new RenderContext(container, events)
      → create scene, renderer, lights
      → attach ResizeObserver
  → camera = new Camera(renderContext, events)
      → create camera, controls, center dot
      → subscribe to resize and world reference changed events
  → world = new World(events)
      → subscribe to world reference changed event
  → systems.push(new WorldReferenceSystem(camera, events))
```

### Frame Update Flow

```
Game.animate(currentTime)
  → calculate deltaTime
  → camera.update()
      → controls.update() (orbit controls damping)
      → calculate worldPosition (camera forward ∩ XY plane)
      → update center dot position

  → worldReferenceSystem.update(deltaTime)
      → read camera.worldPosition
      → if distance > REPOSITION_THRESHOLD:
          → calculate newCenter ChunkId
          → calculate oldPosition, newPosition
          → dispatchWorldReferenceChanged(events, oldCenter, newCenter, oldPosition, newPosition)
              → World event handler:
                  → apply delta to all chunk.group.position
                  → update _referenceCenter
              → Camera event handler:
                  → apply delta to camera.position

  → renderContext.render(camera.camera)
      → renderer.render(scene, camera)
```

### Resize Flow

```
ResizeObserver callback (in RenderContext)
  → read container.clientWidth/clientHeight
  → renderer.setSize(w, h)
  → dispatchViewportResize(events, w, h)
      → Camera event handler:
          → camera.aspect = width / height
          → camera.updateProjectionMatrix()
```

## Files Changed

### New Files

**TypeScript (client/web/src):**
- `game-events.ts` - Event bus factory and type-safe helpers
- `game-system.ts` - GameSystem interface
- `render-context.ts` - RenderContext class
- `camera/camera.ts` - Camera class
- `systems/world-reference-system.ts` - WorldReferenceSystem class

### Modified Files

**TypeScript (client/web/src):**
- `game.ts` - Refactor to use new architecture (RenderContext, Camera, systems)
- `world/world.ts` - Add events parameter, subscribe to world reference changed event
- `world/types.ts` - Add utility functions: `worldPositionToChunkId()`, `chunkIdToWorldPosition()`

### No Changes
- `world/chunk.ts` - Unchanged
- `world/mesh-builder.ts` - Unchanged

## Testing Considerations

### Manual Testing
1. Load game, verify camera controls work (orbit, zoom, pan)
2. Verify center dot appears at ground intersection point
3. Move camera far from origin (>5000 meters), verify world repositions seamlessly (no visual pop)
4. Resize browser window, verify camera aspect ratio updates
5. Toggle `camera.showCenterDot = false` in console, verify dot disappears

### Edge Cases
- Camera parallel to XY plane (forward.z ≈ 0): fallback to camera XY position
- Multiple rapid repositions: system should handle correctly (delta accumulates)
- Reposition exactly at threshold boundary: guard against thrashing

### Debug Features
- Center dot provides visual feedback for world position calculation
- Console access to `camera.showCenterDot` for toggling
- `camera.worldPosition` readable for debugging

## Future Extensions

### Potential New Systems
- **ChunkLoadingSystem**: Monitors camera position, loads/unloads chunks based on distance
- **InputSystem**: Centralizes keyboard/mouse input, dispatches input events
- **DebugOverlaySystem**: Renders performance stats, chunk boundaries, coordinate grids

### Potential New Events
- `cameramoved`: Dispatched each frame with camera position (for minimap updates)
- `chunkloaded` / `chunkunloaded`: World events for UI updates
- `inputaction`: Input system events for game logic to consume

### Architectural Notes
- Systems can own resources but should favor coordination over ownership
- Events should carry sufficient data to avoid listeners needing back-references
- Keep event payloads lean - avoid copying large data structures
- Event names use lowercase, no separators (e.g., `worldreferencechanged` not `world-reference-changed`)
