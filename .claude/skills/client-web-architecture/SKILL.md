---
name: client-web-architecture
description: Client web architecture patterns. Use when adding features, refactoring, or deciding code placement. Covers engine vs domain separation, event co-location, and module organization.
---

# Client Web Architecture

## Structure

```
client/web/src/
├── engine/       # Infrastructure (reusable)
│   ├── events.ts          # EventDispatcher, EventSubscriptions
│   ├── game-system.ts     # System interface
│   ├── render-context.ts  # THREE.js rendering
│   ├── debug-panel.ts     # lil-gui debug panel
│   └── game.ts            # Orchestrator
├── camera/       # Domain: Camera + controls
├── systems/      # Domain: Game systems (orchestrators)
└── world/        # Domain: Chunks, hex math, WASM
```

**Rule**: Infrastructure in `engine/`, game logic outside.

---

## Event Pattern: Co-location

**Events live with the module that dispatches them.**

```typescript
// systems/world-reference-system.ts
export const WORLD_REFERENCE_CHANGED = 'worldreferencechanged';

export type WorldReferenceChangedEvent = {
    oldChunkId: ChunkId;
    newChunkId: ChunkId;
    getDeltaPosition(): [number, number];
};

export class WorldReferenceSystem implements GameSystem {
    private readonly dispatcher: EventDispatcher;

    constructor(..., events: EventTarget, ...) {
        this.dispatcher = new EventDispatcher(events);
    }

    update() {
        if (shouldReposition) {
            this.dispatcher.dispatch<WorldReferenceChangedEvent>(
                WORLD_REFERENCE_CHANGED,
                { oldChunkId, newChunkId, ... }
            );
        }
    }
}
```

**Subscribing (arrow function handlers):**

```typescript
// world/world.ts
export class World {
    private readonly subscriptions: EventSubscriptions;

    constructor(events: EventTarget) {
        this.subscriptions = new EventSubscriptions(events);
        this.subscriptions.on<WorldReferenceChangedEvent>(
            WORLD_REFERENCE_CHANGED,
            this.handleWorldReferenceChanged
        );
    }

    destroy() {
        this.subscriptions.destroy();
    }

    private handleWorldReferenceChanged = (event: WorldReferenceChangedEvent): void => {
        // Handle event
    };
}
```

**Why arrow functions:** Handlers passed directly to `on()` must bind `this`. Arrow function properties capture `this` automatically.

**EventSubscriptions** - One subscription per event name (replaces on re-registration):
- `on<T>(eventName, handler)` - Subscribe to custom events
- `listenWindow<K>(eventName, handler)` - Subscribe to window events
- `remove(eventName)` - Remove subscription
- `destroy()` - Cleanup all

**EventDispatcher** - Dispatches events:
- `dispatch<T>(eventName, detail)` - Type-safe dispatch

**Event naming**: `'lowercase'` (e.g., `'worldreferencechanged'`, `'viewportresize'`)

**Event constants**: Export from dispatcher module
```typescript
export const WORLD_REFERENCE_CHANGED = 'worldreferencechanged';
```

---

## Class Member Ordering

```typescript
export class Example {
    // 1. Properties (state)
    readonly publicProp: string;
    private privateProp: number;

    // 2. Constructor (initialization)
    constructor(events: EventTarget) {
        this.subscriptions.on(EVENT, this.handleEvent);
    }

    // 3. Public functions (API)
    public doSomething(): void { }

    // 4. Handlers (arrow properties for callbacks)
    private handleEvent = (event: EventType): void => {
        // Event handling
    };

    // 5. Private functions (implementation)
    private helperMethod(): void { }
}
```

---

## Module Types

### System (orchestrator)
- Implements `GameSystem` interface (`update(deltaTime)`, `destroy()`)
- Coordinates between resources (Camera, World)
- Lives in `systems/`
- Dispatches events when coordination triggers state changes
- Example: `WorldReferenceSystem` monitors camera, dispatches repositioning events

### Resource (state holder)
- Manages game state and THREE.js objects
- Lives in own directory: `camera/`, `world/`
- Subscribes to relevant events
- May dispatch events (define in same file)
- Example: `Camera`, `World`, `Chunk`

### Infrastructure
- Generic, reusable code
- Lives in `engine/`
- No game-specific logic
- Example: `EventDispatcher`, `RenderContext`, `GameSystem` interface

---

## Decision Framework

**Adding new code?**
- Could it be reused in a different game? → `engine/`
- Game-specific orchestration? → `systems/` (GameSystem)
- Game-specific state? → New resource directory
- Dispatches events? → Define them in same file

**Adding a system:**
1. Create `systems/my-system.ts`
2. Implement `GameSystem` interface
3. If dispatches events, define event type + constant in same file
4. Create `EventDispatcher` instance for dispatching
5. Register in `engine/game.ts` constructor

**Adding a resource:**
1. Create `resource-name/resource-name.ts`
2. Create `EventSubscriptions` instance for listening
3. If dispatches events, define event type + constant + create `EventDispatcher`
4. Cleanup in `destroy()` method

---

## DebugPanel Pattern

```typescript
export class MySystem implements GameSystem {
    private readonly SCOPE = 'My System';

    constructor(..., debugPanel: DebugPanel) {
        // Values updated each frame
        debugPanel.set(this.SCOPE, 'Key', 'value');

        // Toggles (controls)
        debugPanel.addToggle('Controls', 'Toggle Name', object, 'property');
    }

    destroy() {
        debugPanel.removeScope(this.SCOPE);
    }
}
```

**DebugPanel API:**
- `set(scope, key, value)` - Update value
- `addToggle(scope, name, object, property)` - Add boolean toggle
- `removeScope(scope)` - Cleanup scope

---

## Common Patterns

**System pattern:**
```typescript
export class MySystem implements GameSystem {
    private readonly dispatcher: EventDispatcher;
    private readonly SCOPE = 'My System';

    constructor(
        resource1: Resource1,
        resource2: Resource2,
        events: EventTarget,
        debugPanel: DebugPanel
    ) {
        this.dispatcher = new EventDispatcher(events);
    }

    update(deltaTime: number) {
        // Read from resources, dispatch events
        this.debugPanel.set(this.SCOPE, 'Key', 'value');
    }

    destroy() {
        this.debugPanel.removeScope(this.SCOPE);
    }
}
```

**Resource pattern:**
```typescript
export class MyResource {
    private readonly subscriptions: EventSubscriptions;
    private readonly dispatcher?: EventDispatcher; // If dispatches events

    constructor(events: EventTarget) {
        this.subscriptions = new EventSubscriptions(events);
        this.subscriptions.on<SomeEvent>(SOME_EVENT, this.handleSomeEvent);

        // If dispatches events:
        this.dispatcher = new EventDispatcher(events);
    }

    destroy() {
        this.subscriptions.destroy();
    }

    private handleSomeEvent = (event: SomeEvent): void => {
        // Handle event
    };
}
```

**Window event subscription:**
```typescript
constructor(renderContext: RenderContext, events: EventTarget) {
    this.subscriptions = new EventSubscriptions(events);

    // Window events
    this.subscriptions.listenWindow('keydown', (e) => {
        if (e.key === 'Tab') {
            e.preventDefault();
            // Handle
        }
    });
}
```

---

## Anti-Patterns

❌ Centralized events file with all event types
❌ Game logic in `engine/`
❌ Missing `destroy()` cleanup
❌ Direct method calls between resources (use events)
❌ Multiple EventTarget instances (use single bus from Game)

---

## Quick Reference

| What | Where | Type |
|------|-------|------|
| Event infrastructure | `engine/events.ts` | Infrastructure |
| GameSystem interface | `engine/game-system.ts` | Infrastructure |
| THREE.js rendering | `engine/render-context.ts` | Infrastructure |
| Debug panel | `engine/debug-panel.ts` | Infrastructure |
| Game orchestrator | `engine/game.ts` | Infrastructure |
| Camera + controls | `camera/` | Domain resource |
| World/chunks | `world/` | Domain resource |
| Orchestrators | `systems/` | Domain system |
| Event types | With triggering module | Co-located |
