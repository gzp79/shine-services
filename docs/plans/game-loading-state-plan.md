# Plan: Game stages (loading → playing) + stage-view preview experiment

Goals
1. The Game scene starts in a **loading** stage that runs async preload steps, shows a Three.js-rendered
   progress visual plus status text, then switches to **playing**. Dummy steps for now — no fetch, no assets.
2. **Stages are self-contained**: each owns its own input handling, systems, camera/scene, debug-panel
   folder and event subscriptions. Nothing stage-specific lives on `Game`.
3. **Switching is one call with typed data**: `ctx.switchTo('playing', { world })`. The compiler checks
   the payload matches the target stage; ownership of disposable payload data transfers with it.
4. A **preview experiment** shows stage visuals in isolation with lil-gui controls (progress slider etc.).
   Adding a stage or a preview later is a fixed, small recipe.

Naming: "stage" (not "state") — avoids collision with `InputState` and reads as game flow
(loading, menu, playing, level transition, game over).

## Current state (relevant facts)

- `SceneRuntime.run()` constructs the scene synchronously, calls `scene.init?.()`, starts the rAF loop.
- `runtime.spawn(promise)` turns a rejection into a fatal error → host `showFatal`.
- **Gotcha:** on hash navigation `runtime.run()` disposes the old scene but the runtime stays alive. A late
  rejection from the old game's async work would `reportFatal` and kill the *new* scene → all stage async
  work must be abortable and an abort must resolve silently.
- `Game` today owns everything: events, render context, debug panel, RTS camera, world cursor, world,
  assets, text sprites, `InputState` + `InputManager` (game-specific RTS schemas), systems.
- `InputManager(handler, container)` attaches DOM listeners for its schemas; `enabled` setter gates them;
  `dispose()` detaches. So "one InputManager per stage" is already a clean lifetime unit.
- `GameWorld(textSprites, events, debugPanel)` subscribes to the shared `events` bus.
- `EventSubscriptions` (managed listeners, `dispose()` cleans all) and `DebugPanel.scope/removeScope` exist.
- `RenderContext.render(camera, dt)` always renders `this.scene`; metrics update there.
- Scene list is duplicated in `scene-registry.ts` and the hard-coded `<select>` in `index.html`.

## Ownership model

| Owner | Lifetime | Holds |
|---|---|---|
| `Game` (session) | whole scene | `GameServices`: container, renderContext, debugPanel, events bus, assets, textSprites; the `StageMachine`; host input flag |
| Stage | enter → leave | its own scene/camera, `InputManager`/`InputState` (or a lighter input helper), systems, debug folder, event subscriptions, view |
| Payload | `switchTo` → target's factory | data produced by one stage for the next (e.g. the built `GameWorld`); ownership moves to the receiving stage |

Rule: a stage may **read/use** `GameServices` but never dispose them; it disposes everything it created or
received.

## 1. Stage machinery (`src/game/stages/`)

### Types (`stage.ts`)
```ts
// Single registry of stages and the data each one is entered with. Adding a stage = one line here.
export interface StageInputs {
    loading: LoadingInput;     // { reason: 'boot' | 'restart' }
    playing: PlayingInput;     // { world: GameWorld }
}
export type StageId = keyof StageInputs;

// switchTo('x') allowed without a payload when the stage's input type is void.
type SwitchArgs<K extends StageId> = StageInputs[K] extends void ? [] : [input: StageInputs[K]];

export interface StageContext {
    readonly services: GameServices;
    readonly signal: AbortSignal;                       // aborted when the stage is left / game disposed
    readonly debug: GUI;                                // stage-owned debug folder, removed on leave
    readonly subscriptions: EventSubscriptions;         // on services.events, disposed on leave
    spawn(work: Promise<unknown>): void;                // errors → fatal, unless signal aborted
    switchTo<K extends StageId>(id: K, ...args: SwitchArgs<K>): void;
}

export interface Stage {
    enter?(): void;                                     // after construction, ctx ready; kick off async here
    update(dt: number): void;                           // run own systems
    render(dt: number): void;                           // draw own scene
    setInputEnabled(enabled: boolean): void;            // host flag; stage gates its own input with it
    exit?(): Promise<void> | void;                      // optional exit animation; still updated/rendered meanwhile
    dispose(): void;                                    // release own + received resources
}

export interface StageDef<K extends StageId> {
    create(ctx: StageContext, input: StageInputs[K]): Stage;
    discard?(input: StageInputs[K]): void;              // payload cleanup if the stage is never entered
}
export type StageDefs = { [K in StageId]: StageDef<K> };
```

Input stays inside the stage: the machine only forwards the host's `setInputEnabled` flag (on change and
right after `enter`). Each stage decides what it listens to:
- playing: full `InputManager` + `InputState` + RTS systems (moved from `Game`).
- loading: no game input; optionally a tiny keyboard hook via `ctx.subscriptions` (e.g. `Esc` → nothing yet).
- future menu: its own small input (pointer/keys) through the same pattern.
The project rule still holds per stage: DOM → stage input state → stage update; no DOM handler drives logic directly.

### `StageMachine` (`stage-machine.ts`)
- `constructor(defs: StageDefs, services: GameServices, runtime: SceneRuntime, veil: TransitionVeil)`.
- `start(id, ...args)`, `frame(dt)`, `setInputEnabled(enabled)`, `dispose()`, `get currentId`.
- `frame(dt)`: apply a queued switch if any, then `current.update(dt)` + `current.render(dt)`.
- Switch sequence (one in flight; a newer `switchTo` replaces the queued target and `discard`s its payload):
  1. abort old stage's `AbortController`; old stage's input disabled
  2. `await old.exit?.()` (old keeps updating/rendering)
  3. `veil.cover()` → `old.dispose()`, remove its debug folder, dispose its subscriptions
  4. build new ctx (fresh controller, folder, subscriptions) → `def.create(ctx, input)` → `enter()` → forward host input flag
  5. `veil.reveal()`
- `spawn` wraps `runtime.spawn`; drops rejections once the owning stage's signal is aborted.
- `dispose()` mid-transition: aborts, disposes current, `discard`s an un-entered payload.
- Each stage gets its own debug folder named after its id → stage-specific debug UI appears/disappears
  automatically.

### `TransitionVeil` (`transition-veil.ts`)
DOM div (bg `#1a1a2e`, `pointer-events:none`, inline styles) in the container; `cover(ms)` / `reveal(ms)`
promises resolved on `transitionend` with a timeout fallback. Per-switch durations can be overridden later
(`switchTo` options) — not needed now.

### Tests (`stage-machine.test.ts`, no renderer — fake stages)
Lifecycle order; payload passed through intact; `discard` called for superseded / never-entered payloads;
queued switch applied between frames; rejection after leave is silent, while active → fatal handler;
re-enter builds a fresh instance; host input flag forwarded on enter; dispose mid-transition.
Typing checked by a `// @ts-expect-error` case (wrong payload for `playing`, missing payload).

## 2. Game (`src/game/game.ts`) becomes a thin shell
```ts
constructor(context) {
    // services: container focus setup, renderContext, debugPanel, events, assets, textSprites
    this.stages = new StageMachine({ loading: loadingStage, playing: playingStage }, this.services, runtime, veil);
}
init()                  { this.stages.start('loading', { reason: 'boot' }); }
frame(dt)               { this.stages.frame(dt); }
setInputEnabled(e)      { this.stages.setInputEnabled(e); }
dispose()               { this.stages.dispose(); /* then services */ }
```
Debug panel root gets a "Stages" folder: current stage name + "Restart loading" button
(`ctx.switchTo('loading', { reason: 'restart' })` from any stage — exposed via machine).

## 3. Playing stage (`src/game/playing/playing-stage.ts`)
Receives `PlayingInput { world: GameWorld }` and takes ownership of it. Creates and owns what `Game` owns today
apart from shared services: `RtsCamera`, `WorldCursor`, `InputState`, `InputManager`, the six systems (same
order), "Show World Cursor" control in `ctx.debug`, adds `world.group` to `renderContext.scene`.
`render` = `renderContext.render(camera.camera, dt)`. `dispose` = current `Game.dispose` order minus services,
plus removing `world.group` and `world.dispose()`.

## 4. Loading feature (`src/game/loading/`)

### Preloader — pure logic (`preloader.ts`)
```ts
export interface PreloadProgress { fraction: number; label: string; detail?: string; step: number; stepCount: number; }
export interface PreloadStep<R> { label: string; weight?: number; run(ctx: PreloadStepContext, result: R): Promise<void>; }
export interface PreloadStepContext { readonly signal: AbortSignal; report(fraction: number, detail?: string): void; }

export async function runPreload<R>(steps: PreloadStep<R>[], result: R, onProgress, signal): Promise<'done' | 'aborted'>;
export function delay(ms: number, signal: AbortSignal): Promise<void>;
```
- Steps fill a shared, mutable `result` accumulator (`Partial<PlayingInput>`-like) — that's how loaded data
  reaches the next stage.
- Sequential; weighted, clamped, monotonic fraction; always 0 at step start, 1 at end.
- Abort → `'aborted'`, no further callbacks; other errors propagate (→ fatal).
- Tests with fake timers.

### Steps (`steps.ts`) — ~2.5–3 s total
| Label | Weight | Behaviour |
|---|---|---|
| Waking the engine | 1 | dummy, 300 ms, 3 ticks |
| Shaping the hex lattice | 3 | dummy, 900 ms, 12 ticks, detail "cell n/12" |
| Mixing terrain palettes | 2 | dummy, 600 ms, uneven ticks |
| Warming up shaders | 2 | dummy, 500 ms, stall then jump |
| Building the world | 1 | **real**: `new GameWorld(services.textSprites, services.events, debugPanel)` → `result.world` |

The last step is the one real piece: it demonstrates data transfer (world built in loading, owned by playing).
Dummy steps take a `timeScale` option (used by the preview). On abort, the loading stage disposes a
partially-built `result.world`.

### LoadingView — visuals only (`loading-view.ts`)
```ts
export interface LoadingViewParams { tileCount; colorFrom; colorTo; dimColor; rotationSpeed; bobAmount; easeRate; comet; reducedMotion; }
export class LoadingView {
    readonly params: LoadingViewParams;
    constructor(container: HTMLElement, renderContext: RenderContext, params?: Partial<LoadingViewParams>);
    setProgress(p: { fraction: number; label: string; detail?: string }, opts?: { snap?: boolean }): void;
    rebuild(): void;
    playExit(): Promise<void>;
    reset(): void;
    update(dt: number): void;
    render(dt: number): void;
    dispose(): void;
}
```
- Own `THREE.Scene` + slightly tilted `PerspectiveCamera`; rendered via `renderContext.render(camera, dt, scene)`
  (new optional scene arg) so metrics + resize are reused.
- "Hex ring": `tileCount` flat hex prisms in one `InstancedMesh`, per-instance colour; lit tiles along
  `colorFrom → colorTo` with pop-in (ease-out-back) + lift; pulsing frontier tile; orbiting comet sprite with
  progress-length trail; slow rotation + bob. Displayed fraction eases toward target, never backwards; `snap`
  bypasses easing. `reducedMotion` (default from `prefers-reduced-motion`) disables rotation/bob/comet.
- Exit: flash + expand + fade (~400 ms).
- Text as DOM overlay (crisp, accessible): "SHINE" title, step label with cross-fade, detail line,
  percentage (`tabular-nums`); `role="progressbar"`, `aria-valuenow/valuetext`, `aria-live="polite"`; inline styles.

### LoadingStage (`loading-stage.ts`)
- `create(ctx, { reason })`: build `LoadingView`; `enter()` → `ctx.spawn(this.run())`.
- `run()`: `runPreload(steps, result, view.setProgress, ctx.signal)`; on `'done'` →
  `ctx.switchTo('playing', { world: result.world! })` (ownership handed over; loading no longer disposes it).
- `exit()` → `view.playExit()`; `update/render` → view; `setInputEnabled` → no-op (no game input);
  `dispose` → view + any un-handed-over result data.

## 5. Preview experiment (`src/experiments/stage-preview/`)
Previews **views**, not full stages (no game logic, no async coupling).

```ts
// previews.ts
export interface PreviewHandle { update(dt: number): void; render(dt: number): void; dispose(): void; }
export interface PreviewEntry {
    id: string;
    title: string;
    create(ctx: { container: HTMLElement; renderContext: RenderContext; gui: GUI }): PreviewHandle;
}
export const previews: PreviewEntry[] = [loadingPreview];
```
`StagePreview extends Experiment` (`addOrbitCamera: false`): lil-gui "View" dropdown over `previews`;
switching disposes the handle + its folder and creates the next; overrides `frame(dt)` → handle.

`loadingPreview` controls:
- Mode `manual`: `progress` slider 0..1, `label`/`detail` text, `ease` toggle (snap vs eased).
- Mode `simulate`: runs `runPreload` with dummy steps (own AbortController), `loop`, `speed` (→ `timeScale`).
- Buttons: Play exit, Reset, Replay. "Look" folder bound live to `view.params` (`tileCount` → `rebuild()`).
- "Copy params" → copies current `params` JSON for pasting back as defaults.

Register in `scene-registry.ts` (`stage-preview`, "Stage Preview") and add the `<option>` in `index.html`.

## Recipe: adding a future stage
1. `src/game/<name>/<name>-view.ts` (if it has visuals): `params`, setters, `update/render/dispose`.
2. `src/game/<name>/<name>-stage.ts`: implements `Stage`; owns its input/systems/scene; async via `ctx.spawn`;
   leaves via `ctx.switchTo(target, payload)`.
3. `StageInputs`: add `<name>: <Name>Input` (or `void`); add the `StageDef` to `Game`'s defs — the compiler
   then flags every `switchTo` and def that is missing/mistyped.
4. `previews.ts`: add a `PreviewEntry` for the view.

## Files

| File | Change |
|---|---|
| `src/game/stages/stage.ts` | new — `StageInputs`, `Stage`, `StageContext`, `StageDef`, `GameServices` |
| `src/game/stages/stage-machine.ts` + `.test.ts` | new |
| `src/game/stages/transition-veil.ts` | new |
| `src/game/loading/preloader.ts` + `.test.ts` | new |
| `src/game/loading/steps.ts` | new — dummy steps + world build step |
| `src/game/loading/loading-view.ts` | new |
| `src/game/loading/loading-stage.ts` | new |
| `src/game/playing/playing-stage.ts` | new — camera, cursor, input, systems moved from `Game` |
| `src/game/game.ts` | thin shell: services + stage machine |
| `src/engine/compositor/render-context.ts` | optional `scene` arg on `render()` |
| `src/experiments/stage-preview/index.ts`, `previews.ts` | new |
| `src/scene-registry.ts`, `index.html` | register `stage-preview` |
| `.claude/skills/client-web-architecture/SKILL.md` | document stages (ownership, input per stage, payloads, recipe) + preview; fix stale `engine/game.ts` paths (now `src/game/`) and the Game table, which moves into the playing stage |

## Order of work
1. `preloader.ts` + tests (TDD).
2. `stages/` types + machine + veil; machine tests (TDD), including type-level payload checks.
3. `RenderContext.render` scene arg; `steps.ts`.
4. Move game content into `PlayingStage`; `Game` as shell; temporarily start straight in `playing`
   (world built in shell) to confirm nothing regressed.
5. `loading-view.ts` + preview experiment — tune the look.
6. `loading-stage.ts`; boot into `loading`; drop the temporary world build.
7. Verify: `pnpm test`, `pnpm lint`, `pnpm dev`:
   - `#` (game): loading sequence → veil → world; input dead during loading, live after; RTS controls unchanged.
   - "Restart loading" from playing: old world disposed, new one built, no duplicate listeners
     (move/zoom speed unchanged after several restarts).
   - Navigate to an experiment mid-load and back: no fatal, no leaked DOM (veil, text, debug folders).
   - Temporarily throw from a dummy step → fatal page.
   - `#stage-preview`: slider, simulate+loop, speed, play exit/reset, live params, tileCount rebuild.
   - Playwright screenshots of loading mid-progress and the preview.

## Open questions
- Minimum loading display time once steps are real.
- Host-facing progress (`SceneHandle.onProgress`) — not now.
- Generate `index.html` scene options from `listScenes()` — offer separately.
