---
name: client-web-architecture
description: >
  Architecture guide for the client/web TypeScript game client in shine-services.
  Use whenever working on any file under client/web/src — systems, resources,
  input, rendering, events, world/chunk, avatar/cursor, or experiments. Invoke
  before writing or reviewing any code in that tree.
---

# Client Web Architecture

`client/web/src/` — TypeScript WebGPU game loop around a central `Game` store.

## Game — central store (`engine/game.ts`)

Three groups; no formal type boundary — just construction order.

### Engine core
Infra everything else depends on. Passed as constructor args.

| Field | Type | Role |
|---|---|---|
| `events` | `EventTarget` | typed event bus |
| `renderContext` | `RenderContext` | WebGPU renderer + Three.js scene |
| `debugPanel` | `DebugPanel` | lil-gui debug UI |
| `performanceMetrics` | `PerformanceMetrics` | frame/GPU stats |
| `inputState` | `InputState` | per-frame input accumulator |
| `inputManager` | `InputManager` | DOM → schemas → inputState |

### Resources
Domain data stores. Implement `GameResource` (`engine/game-resource.ts`):
```ts
interface GameResource { name: string; dispose(): void; }
```
Registered in `this.resources[]` for lifecycle. Read/write own state, may fire events, no direct cross-resource calls.

| Field | Type | Depends on |
|---|---|---|
| `camera` | `Camera` | events |
| `worldCursor` | `WorldCursor` | renderContext.scene, events |
| `world` | `World` | events, debugPanel |

### Systems
Per-frame workers. Implement `GameSystem` (`engine/game-system.ts`):
```ts
interface GameSystem { name: string; update(dt: number): void; dispose(): void; }
```
Run in order; read resources, write resources or emit events. Never call each other directly.

| System | Reads | Writes/emits |
|---|---|---|
| `CameraViewportSystem` | renderContext | camera aspect |
| `CursorDriveSystem` | inputState, camera | worldCursor |
| `CameraFollowCursorSystem` | worldCursor, events | camera view |
| `WorldReferenceSystem` | worldCursor, world | world rebase + events |
| `SelectionSystem` | inputState, camera, world | world selection |
| `ClearInputStateSystem` | inputState | clears pending input (runs last) |

## Events (`engine/events.ts`)

`EventDispatcher.dispatch<T>(name, detail)` — typed dispatch.
`EventSubscriptions.on<T>(name, handler)` — managed listeners, auto-cleanup via `dispose()`.

Rule: no DOM event controls game logic directly. DOM → `InputState` → systems.

## Input pipeline

```
DOM → Raw detectors (raw/*.ts)
    → Schemas (schemas/*.ts) — gesture composition + conflict rules
    → InputManager — single active schema at a time (no-steal while busy)
    → InputState — per-frame accumulator; cleared by ClearInputStateSystem
```

`InputState` fields: `pointerPos`, `moveSpeed`, `rotateSpeed`, `zoomSpeed`,
`pendingMoveTo`, `pendingRotateBy`, `pendingZoomBy`, `pendingInteracts[]`, `pendingSchemaChange`.

## Experiments (`experiments/`)

Isolated prototypes — not in `Game`, not shipped. Extend `Experiment` base class
(`experiments/experiment.ts`): own Three.js scene + camera + renderer + OrbitControls.
Minimal pattern: extend, build geometry in ctor, call `this.start()`, lil-gui params, `regenerate()` on change.
See `experiments/hex-mesh/` or `experiments/cdt/` as reference.

## Adding things

- **Resource**: impl `GameResource`, register in `game.ts` `this.resources[]`
- **System**: impl `GameSystem`, register in `game.ts` `this.systems[]` in correct order
- **Event**: add const name, dispatch with `EventDispatcher`, subscribe with `EventSubscriptions`
- **Experiment**: extend `Experiment` in new `experiments/<name>/` — never wired into `Game`

## Ref docs

| Doc | Load when |
|---|---|
| `docs/client/web/input-system.html` | working on anything in `engine/input/` |
