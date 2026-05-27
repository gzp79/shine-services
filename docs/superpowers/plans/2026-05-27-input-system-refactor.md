# Input System Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor input system from event-driven locomotion to poll-based architecture with centralized conflict resolution in InputManager.

**Architecture:** Move conflict resolution from DesktopController to InputManager, change locomotion from event-driven (CURSOR_MOVE/ROTATE/ZOOM events) to poll-based (stored state), create CursorLocomotionSystem that polls locomotion state and applies to WorldCursor, make WorldCursor passive (no update()).

**Tech Stack:** TypeScript, Three.js, Vitest

---

## File Structure

**New files:**
- `client/web/src/avatar/input-manager.ts` — Implements InputHandler, applies conflict resolution, stores locomotion state, emits gesture events
- `client/web/src/avatar/cursor-locomotion-system.ts` — Per-frame system that polls locomotion state and integrates into WorldCursor
- `client/web/src/engine/input/_input-controller.test.ts` — Test suite for InputManager (49 test cases)

**Modified files:**
- `client/web/src/engine/input/_desktop-controller.ts` — Remove conflict resolution logic (lines 127-136, 147-152, 291-294)
- `client/web/src/avatar/world-cursor.ts` — Remove update() integration logic, add setters (setPosition, setYaw, setZoom), keep gesture event handlers
- `client/web/src/avatar/events.ts` — Remove CURSOR_MOVE, CURSOR_ROTATE, CURSOR_ZOOM events (stale)
- `client/web/src/avatar/input-mapper.ts` — Delete (functionality moves to InputManager)

---

## Task 1: Create InputManager with locomotion state storage

**Files:**
- Create: `client/web/src/avatar/input-manager.ts`
- Reference: `client/web/src/engine/input/_input-handler.ts` (InputHandler interface)
- Reference: `client/web/src/constants.ts` (ROTATE_KEY_SPEED, ZOOM_KEY_SPEED, ROTATE_SENSITIVITY, ZOOM_SENSITIVITY)

- [ ] **Step 1: Write failing test for locomotion state structure**

Create: `client/web/src/engine/input/_input-controller.test.ts`

```typescript
import { describe, it, expect } from 'vitest';
import { InputManager } from '../../../avatar/input-manager';
import type { Camera } from '../../camera/camera';
import type { WorldCursor } from '../../../avatar/world-cursor';

describe('InputManager - Locomotion State', () => {
    const mockCamera = {} as Camera;
    const mockWorldCursor = {} as WorldCursor;
    const mockEvents = new EventTarget();

    it('should initialize locomotion state to zero', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        const state = manager.getLocomotionState();
        
        expect(state.move.x).toBe(0);
        expect(state.move.y).toBe(0);
        expect(state.rotateRate).toBe(0);
        expect(state.zoomRate).toBe(0);
        expect(state.sprint).toBe(false);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "should initialize locomotion state"`
Expected: FAIL with "Cannot find module '../../../avatar/input-manager'"

- [ ] **Step 3: Create InputManager skeleton with locomotion state**

Create: `client/web/src/avatar/input-manager.ts`

```typescript
import type { Delta, InputHandler, Point } from '../engine/input/_input-handler';
import type { Camera } from '../engine/camera/camera';
import type { WorldCursor } from './world-cursor';

export interface LocomotionState {
    move: Delta;
    rotateRate: number;
    zoomRate: number;
    sprint: boolean;
}

export class InputManager implements InputHandler {
    private locomotionState: LocomotionState = {
        move: { x: 0, y: 0 },
        rotateRate: 0,
        zoomRate: 0,
        sprint: false
    };

    constructor(
        private readonly camera: Camera,
        private readonly worldCursor: WorldCursor,
        private readonly events: EventTarget
    ) {}

    getLocomotionState(): Readonly<LocomotionState> {
        return this.locomotionState;
    }

    // InputHandler interface stubs (will implement in later tasks)
    onControllerChanged(_controller: 'touch' | 'desktop'): void {}
    onTap(_pos: Point): void {}
    onInteractStart(_pos: Point): void {}
    onInteractDrag(_start: Point, _current: Point): void {}
    onInteractEnd(_pos: Point): void {}
    onDragPanStart(_pos: Point): void {}
    onDragPan(_start: Point, _current: Point): void {}
    onDragPanEnd(_pos: Point): void {}
    onDragRotateStart(_pos: Point): void {}
    onDragRotate(_start: Point, _current: Point): void {}
    onDragRotateEnd(_pos: Point): void {}
    onPinchStart(_start: [Point, Point], _current: [Point, Point]): void {}
    onPinch(_start: [Point, Point], _current: [Point, Point]): void {}
    onPinchEnd(_start: [Point, Point], _current: [Point, Point]): void {}
    onZoomTo(_pos: Point, _delta: number): void {}
    onMove(_direction: Delta, _isSprinting: boolean): void {}
    onRotate(_direction: number): void {}
    onZoom(_direction: number): void {}
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "should initialize locomotion state"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add client/web/src/avatar/input-manager.ts client/web/src/engine/input/_input-controller.test.ts
git commit -m "feat(input): add InputManager with locomotion state structure"
```

---

## Task 2: Implement keyboard locomotion state updates in InputManager

**Files:**
- Modify: `client/web/src/avatar/input-manager.ts`
- Modify: `client/web/src/engine/input/_input-controller.test.ts`
- Reference: `client/web/src/constants.ts`

- [ ] **Step 1: Write failing tests for key state updates**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
describe('InputManager - Locomotion State', () => {
    // ... existing test ...

    it('should update move state when onMove called', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        
        manager.onMove({ x: 1, y: 0 }, false);
        const state = manager.getLocomotionState();
        
        expect(state.move.x).toBe(1);
        expect(state.move.y).toBe(0);
        expect(state.sprint).toBe(false);
    });

    it('should normalize diagonal movement', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        
        manager.onMove({ x: 1, y: 1 }, false);
        const state = manager.getLocomotionState();
        
        const length = Math.sqrt(state.move.x ** 2 + state.move.y ** 2);
        expect(length).toBeCloseTo(1, 5);
    });

    it('should set sprint flag', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        
        manager.onMove({ x: 1, y: 0 }, true);
        const state = manager.getLocomotionState();
        
        expect(state.sprint).toBe(true);
    });

    it('should update rotate rate when onRotate called', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        
        manager.onRotate(-1);
        const state = manager.getLocomotionState();
        
        expect(state.rotateRate).toBe(-90 * (Math.PI / 180));
    });

    it('should cancel out Q+E both held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        
        manager.onRotate(0); // Q+E cancel out to 0
        const state = manager.getLocomotionState();
        
        expect(state.rotateRate).toBe(0);
    });

    it('should update zoom rate when onZoom called', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        
        manager.onZoom(1);
        const state = manager.getLocomotionState();
        
        expect(state.zoomRate).toBe(250);
    });

    it('should cancel out R+F both held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        
        manager.onZoom(0); // R+F cancel out to 0
        const state = manager.getLocomotionState();
        
        expect(state.zoomRate).toBe(0);
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "Locomotion State"`
Expected: FAIL - locomotion state not updated

- [ ] **Step 3: Implement locomotion state updates**

Modify `client/web/src/avatar/input-manager.ts`:

```typescript
import {
    ROTATE_KEY_SPEED,
    ZOOM_KEY_SPEED
} from '../constants';
import type { Delta, InputHandler, Point } from '../engine/input/_input-handler';
import type { Camera } from '../engine/camera/camera';
import type { WorldCursor } from './world-cursor';

export interface LocomotionState {
    move: Delta;
    rotateRate: number;
    zoomRate: number;
    sprint: boolean;
}

export class InputManager implements InputHandler {
    private locomotionState: LocomotionState = {
        move: { x: 0, y: 0 },
        rotateRate: 0,
        zoomRate: 0,
        sprint: false
    };

    constructor(
        private readonly camera: Camera,
        private readonly worldCursor: WorldCursor,
        private readonly events: EventTarget
    ) {}

    getLocomotionState(): Readonly<LocomotionState> {
        return this.locomotionState;
    }

    onControllerChanged(_controller: 'touch' | 'desktop'): void {}
    onTap(_pos: Point): void {}
    onInteractStart(_pos: Point): void {}
    onInteractDrag(_start: Point, _current: Point): void {}
    onInteractEnd(_pos: Point): void {}
    onDragPanStart(_pos: Point): void {}
    onDragPan(_start: Point, _current: Point): void {}
    onDragPanEnd(_pos: Point): void {}
    onDragRotateStart(_pos: Point): void {}
    onDragRotate(_start: Point, _current: Point): void {}
    onDragRotateEnd(_pos: Point): void {}
    onPinchStart(_start: [Point, Point], _current: [Point, Point]): void {}
    onPinch(_start: [Point, Point], _current: [Point, Point]): void {}
    onPinchEnd(_start: [Point, Point], _current: [Point, Point]): void {}
    onZoomTo(_pos: Point, _delta: number): void {}

    onMove(direction: Delta, isSprinting: boolean): void {
        // Normalize if non-zero
        if (direction.x !== 0 || direction.y !== 0) {
            const length = Math.sqrt(direction.x ** 2 + direction.y ** 2);
            this.locomotionState.move = {
                x: direction.x / length,
                y: direction.y / length
            };
        } else {
            this.locomotionState.move = { x: 0, y: 0 };
        }
        this.locomotionState.sprint = isSprinting;
    }

    onRotate(direction: number): void {
        this.locomotionState.rotateRate = direction * ROTATE_KEY_SPEED;
    }

    onZoom(direction: number): void {
        this.locomotionState.zoomRate = direction * ZOOM_KEY_SPEED;
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "Locomotion State"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add client/web/src/avatar/input-manager.ts client/web/src/engine/input/_input-controller.test.ts
git commit -m "feat(input): implement locomotion state updates in InputManager"
```

---

## Task 3: Add conflict resolution for WASD blocks drag-pan

**Files:**
- Modify: `client/web/src/avatar/input-manager.ts`
- Modify: `client/web/src/engine/input/_input-controller.test.ts`

- [ ] **Step 1: Write failing tests for WASD blocking drag-pan**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
describe('InputManager - WASD Conflict Resolution', () => {
    const mockCamera = {} as Camera;
    const mockWorldCursor = {} as WorldCursor;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockEvents = new EventTarget();
    });

    it('should block drag-pan start when WASD held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let moveToEmitted = false;
        mockEvents.addEventListener('cursor:move_to', () => { moveToEmitted = true; });

        manager.onMove({ x: 1, y: 0 }, false); // WASD active
        manager.onDragPanStart({ x: 100, y: 100 });
        manager.onDragPan({ x: 100, y: 100 }, { x: 150, y: 150 });

        expect(moveToEmitted).toBe(false);
    });

    it('should stop existing drag-pan when WASD pressed', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let moveToCount = 0;
        mockEvents.addEventListener('cursor:move_to', () => { moveToCount++; });

        // Start drag-pan
        manager.onDragPanStart({ x: 100, y: 100 });
        manager.onDragPan({ x: 100, y: 100 }, { x: 150, y: 150 });
        expect(moveToCount).toBe(1);

        // Press WASD - should stop emitting
        manager.onMove({ x: 1, y: 0 }, false);
        manager.onDragPan({ x: 100, y: 100 }, { x: 200, y: 200 });
        expect(moveToCount).toBe(1); // No new event
    });

    it('should unblock drag-pan when WASD released', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let moveToEmitted = false;
        mockEvents.addEventListener('cursor:move_to', () => { moveToEmitted = true; });

        manager.onMove({ x: 1, y: 0 }, false); // WASD active
        manager.onDragPanStart({ x: 100, y: 100 });
        manager.onMove({ x: 0, y: 0 }, false); // WASD released

        manager.onDragPanStart({ x: 100, y: 100 });
        manager.onDragPan({ x: 100, y: 100 }, { x: 150, y: 150 });

        expect(moveToEmitted).toBe(true);
    });

    it('should allow drag-rotate when WASD held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let rotateDeltaEmitted = false;
        mockEvents.addEventListener('cursor:rotate_delta', () => { rotateDeltaEmitted = true; });

        manager.onMove({ x: 1, y: 0 }, false); // WASD active
        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onDragRotate({ x: 100, y: 100 }, { x: 150, y: 100 });

        expect(rotateDeltaEmitted).toBe(true);
    });

    it('should allow wheel zoom when WASD held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let zoomDeltaEmitted = false;
        mockEvents.addEventListener('cursor:zoom_delta', () => { zoomDeltaEmitted = true; });

        manager.onMove({ x: 1, y: 0 }, false); // WASD active
        manager.onZoomTo({ x: 100, y: 100 }, 10);

        expect(zoomDeltaEmitted).toBe(true);
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "WASD Conflict"`
Expected: FAIL - events emitted when they should be blocked

- [ ] **Step 3: Implement WASD conflict resolution**

Modify `client/web/src/avatar/input-manager.ts`:

```typescript
import {
    ROTATE_KEY_SPEED,
    ROTATE_SENSITIVITY,
    ZOOM_KEY_SPEED,
    ZOOM_SENSITIVITY
} from '../constants';
import type { Delta, InputHandler, Point } from '../engine/input/_input-handler';
import type { Camera } from '../engine/camera/camera';
import type { WorldCursor } from './world-cursor';
import { EventDispatcher } from '../engine/events';
import {
    CURSOR_MOVE_TO,
    CURSOR_ROTATE_DELTA,
    CURSOR_ZOOM_DELTA,
    type CursorMoveToEvent,
    type CursorRotateDeltaEvent,
    type CursorZoomDeltaEvent
} from './events';
import * as THREE from 'three';

export interface LocomotionState {
    move: Delta;
    rotateRate: number;
    zoomRate: number;
    sprint: boolean;
}

export class InputManager implements InputHandler {
    private locomotionState: LocomotionState = {
        move: { x: 0, y: 0 },
        rotateRate: 0,
        zoomRate: 0,
        sprint: false
    };
    private activeDragPan = false;
    private dragRotateLastPos: Point | null = null;
    private readonly dispatcher: EventDispatcher;

    constructor(
        private readonly camera: Camera,
        private readonly worldCursor: WorldCursor,
        private readonly events: EventTarget
    ) {
        this.dispatcher = new EventDispatcher(events);
    }

    getLocomotionState(): Readonly<LocomotionState> {
        return this.locomotionState;
    }

    private get isWASDActive(): boolean {
        return this.locomotionState.move.x !== 0 || this.locomotionState.move.y !== 0;
    }

    onControllerChanged(_controller: 'touch' | 'desktop'): void {}
    onTap(_pos: Point): void {}
    onInteractStart(_pos: Point): void {}
    onInteractDrag(_start: Point, _current: Point): void {}
    onInteractEnd(_pos: Point): void {}

    onDragPanStart(_pos: Point): void {
        // Block if WASD active
        if (this.isWASDActive) {
            return;
        }
        this.activeDragPan = true;
    }

    onDragPan(_start: Point, current: Point): void {
        // Block if WASD active
        if (this.isWASDActive) {
            this.activeDragPan = false;
            return;
        }
        if (!this.activeDragPan) return;

        this.emitMoveTo(current);
    }

    onDragPanEnd(_pos: Point): void {
        this.activeDragPan = false;
    }

    onDragRotateStart(pos: Point): void {
        this.dragRotateLastPos = pos;
    }

    onDragRotate(_start: Point, current: Point): void {
        if (!this.dragRotateLastPos) return;

        const deltaX = current.x - this.dragRotateLastPos.x;
        const angleDelta = deltaX * ROTATE_SENSITIVITY;
        this.dragRotateLastPos = current;

        this.dispatcher.dispatch<CursorRotateDeltaEvent>(CURSOR_ROTATE_DELTA, { angleDelta });
    }

    onDragRotateEnd(_pos: Point): void {
        this.dragRotateLastPos = null;
    }

    onPinchStart(_start: [Point, Point], _current: [Point, Point]): void {}
    onPinch(_start: [Point, Point], _current: [Point, Point]): void {}
    onPinchEnd(_start: [Point, Point], _current: [Point, Point]): void {}

    onZoomTo(_pos: Point, delta: number): void {
        this.dispatcher.dispatch<CursorZoomDeltaEvent>(CURSOR_ZOOM_DELTA, { delta: delta * ZOOM_SENSITIVITY });
    }

    onMove(direction: Delta, isSprinting: boolean): void {
        if (direction.x !== 0 || direction.y !== 0) {
            const length = Math.sqrt(direction.x ** 2 + direction.y ** 2);
            this.locomotionState.move = {
                x: direction.x / length,
                y: direction.y / length
            };
        } else {
            this.locomotionState.move = { x: 0, y: 0 };
        }
        this.locomotionState.sprint = isSprinting;
    }

    onRotate(direction: number): void {
        this.locomotionState.rotateRate = direction * ROTATE_KEY_SPEED;
    }

    onZoom(direction: number): void {
        this.locomotionState.zoomRate = direction * ZOOM_KEY_SPEED;
    }

    private emitMoveTo(screenPos: Point): void {
        const intersectionPoint = this.camera.screenToWorldPlanePoint(screenPos.x, screenPos.y);
        if (intersectionPoint) {
            intersectionPoint.z = 0;
            this.dispatcher.dispatch<CursorMoveToEvent>(CURSOR_MOVE_TO, {
                pos: intersectionPoint
            });
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "WASD Conflict"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add client/web/src/avatar/input-manager.ts client/web/src/engine/input/_input-controller.test.ts
git commit -m "feat(input): add WASD conflict resolution in InputManager"
```

---

## Task 4: Add conflict resolution for Q/E blocks drag-rotate

**Files:**
- Modify: `client/web/src/avatar/input-manager.ts`
- Modify: `client/web/src/engine/input/_input-controller.test.ts`

- [ ] **Step 1: Write failing tests for Q/E blocking drag-rotate**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
describe('InputManager - Q/E Conflict Resolution', () => {
    const mockCamera = {} as Camera;
    const mockWorldCursor = {} as WorldCursor;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockEvents = new EventTarget();
    });

    it('should block drag-rotate when Q/E held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let rotateDeltaEmitted = false;
        mockEvents.addEventListener('cursor:rotate_delta', () => { rotateDeltaEmitted = true; });

        manager.onRotate(-1); // Q/E active
        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onDragRotate({ x: 100, y: 100 }, { x: 150, y: 100 });

        expect(rotateDeltaEmitted).toBe(false);
    });

    it('should block Q/E when drag-rotate active (bidirectional)', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        
        // Start drag-rotate first
        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onDragRotate({ x: 100, y: 100 }, { x: 150, y: 100 });

        // Try to activate Q/E - should be blocked
        manager.onRotate(-1);
        const state = manager.getLocomotionState();
        
        expect(state.rotateRate).toBe(0); // Blocked
    });

    it('should unblock drag-rotate when Q/E released', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let rotateDeltaEmitted = false;
        mockEvents.addEventListener('cursor:rotate_delta', () => { rotateDeltaEmitted = true; });

        manager.onRotate(-1); // Q/E active
        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onRotate(0); // Q/E released

        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onDragRotate({ x: 100, y: 100 }, { x: 150, y: 100 });

        expect(rotateDeltaEmitted).toBe(true);
    });

    it('should allow drag-pan when Q/E held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let moveToEmitted = false;
        mockEvents.addEventListener('cursor:move_to', () => { moveToEmitted = true; });

        manager.onRotate(-1); // Q/E active
        manager.onDragPanStart({ x: 100, y: 100 });
        manager.onDragPan({ x: 100, y: 100 }, { x: 150, y: 150 });

        expect(moveToEmitted).toBe(true);
    });

    it('should allow wheel zoom when Q/E held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let zoomDeltaEmitted = false;
        mockEvents.addEventListener('cursor:zoom_delta', () => { zoomDeltaEmitted = true; });

        manager.onRotate(-1); // Q/E active
        manager.onZoomTo({ x: 100, y: 100 }, 10);

        expect(zoomDeltaEmitted).toBe(true);
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "Q/E Conflict"`
Expected: FAIL - drag-rotate not blocked, Q/E not blocked bidirectionally

- [ ] **Step 3: Implement Q/E conflict resolution**

Modify `client/web/src/avatar/input-manager.ts`:

```typescript
export class InputManager implements InputHandler {
    private locomotionState: LocomotionState = {
        move: { x: 0, y: 0 },
        rotateRate: 0,
        zoomRate: 0,
        sprint: false
    };
    private activeDragPan = false;
    private activeDragRotate = false;
    private dragRotateLastPos: Point | null = null;
    private readonly dispatcher: EventDispatcher;

    // ... constructor ...

    private get isWASDActive(): boolean {
        return this.locomotionState.move.x !== 0 || this.locomotionState.move.y !== 0;
    }

    private get isQEActive(): boolean {
        return this.locomotionState.rotateRate !== 0;
    }

    // ... existing methods ...

    onDragRotateStart(pos: Point): void {
        // Block if Q/E active
        if (this.isQEActive) {
            return;
        }
        this.activeDragRotate = true;
        this.dragRotateLastPos = pos;
    }

    onDragRotate(_start: Point, current: Point): void {
        // Block if Q/E active
        if (this.isQEActive) {
            this.activeDragRotate = false;
            this.dragRotateLastPos = null;
            return;
        }
        if (!this.activeDragRotate || !this.dragRotateLastPos) return;

        const deltaX = current.x - this.dragRotateLastPos.x;
        const angleDelta = deltaX * ROTATE_SENSITIVITY;
        this.dragRotateLastPos = current;

        this.dispatcher.dispatch<CursorRotateDeltaEvent>(CURSOR_ROTATE_DELTA, { angleDelta });
    }

    onDragRotateEnd(_pos: Point): void {
        this.activeDragRotate = false;
        this.dragRotateLastPos = null;
    }

    onRotate(direction: number): void {
        // Block if drag-rotate active (bidirectional)
        if (this.activeDragRotate) {
            return;
        }
        this.locomotionState.rotateRate = direction * ROTATE_KEY_SPEED;
    }

    // ... rest of methods ...
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "Q/E Conflict"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add client/web/src/avatar/input-manager.ts client/web/src/engine/input/_input-controller.test.ts
git commit -m "feat(input): add Q/E conflict resolution in InputManager"
```

---

## Task 5: Add conflict resolution for R/F blocks wheel zoom

**Files:**
- Modify: `client/web/src/avatar/input-manager.ts`
- Modify: `client/web/src/engine/input/_input-controller.test.ts`

- [ ] **Step 1: Write failing tests for R/F blocking wheel zoom**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
describe('InputManager - R/F Conflict Resolution', () => {
    const mockCamera = {} as Camera;
    const mockWorldCursor = {} as WorldCursor;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockEvents = new EventTarget();
    });

    it('should block wheel zoom when R/F held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let zoomDeltaEmitted = false;
        mockEvents.addEventListener('cursor:zoom_delta', () => { zoomDeltaEmitted = true; });

        manager.onZoom(1); // R/F active
        manager.onZoomTo({ x: 100, y: 100 }, 10);

        expect(zoomDeltaEmitted).toBe(false);
    });

    it('should unblock wheel zoom when R/F released', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let zoomDeltaEmitted = false;
        mockEvents.addEventListener('cursor:zoom_delta', () => { zoomDeltaEmitted = true; });

        manager.onZoom(1); // R/F active
        manager.onZoom(0); // R/F released
        manager.onZoomTo({ x: 100, y: 100 }, 10);

        expect(zoomDeltaEmitted).toBe(true);
    });

    it('should allow drag-rotate when R/F held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let rotateDeltaEmitted = false;
        mockEvents.addEventListener('cursor:rotate_delta', () => { rotateDeltaEmitted = true; });

        manager.onZoom(1); // R/F active
        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onDragRotate({ x: 100, y: 100 }, { x: 150, y: 100 });

        expect(rotateDeltaEmitted).toBe(true);
    });

    it('should allow drag-pan when R/F held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let moveToEmitted = false;
        mockEvents.addEventListener('cursor:move_to', () => { moveToEmitted = true; });

        manager.onZoom(1); // R/F active
        manager.onDragPanStart({ x: 100, y: 100 });
        manager.onDragPan({ x: 100, y: 100 }, { x: 150, y: 150 });

        expect(moveToEmitted).toBe(true);
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "R/F Conflict"`
Expected: FAIL - wheel zoom not blocked

- [ ] **Step 3: Implement R/F conflict resolution**

Modify `client/web/src/avatar/input-manager.ts`:

```typescript
export class InputManager implements InputHandler {
    // ... existing fields ...

    private get isWASDActive(): boolean {
        return this.locomotionState.move.x !== 0 || this.locomotionState.move.y !== 0;
    }

    private get isQEActive(): boolean {
        return this.locomotionState.rotateRate !== 0;
    }

    private get isRFActive(): boolean {
        return this.locomotionState.zoomRate !== 0;
    }

    // ... existing methods ...

    onZoomTo(_pos: Point, delta: number): void {
        // Block if R/F active
        if (this.isRFActive) {
            return;
        }
        this.dispatcher.dispatch<CursorZoomDeltaEvent>(CURSOR_ZOOM_DELTA, { delta: delta * ZOOM_SENSITIVITY });
    }

    // ... rest of methods ...
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "R/F Conflict"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add client/web/src/avatar/input-manager.ts client/web/src/engine/input/_input-controller.test.ts
git commit -m "feat(input): add R/F conflict resolution in InputManager"
```

---

## Task 6: Implement remaining InputManager gesture handlers

**Files:**
- Modify: `client/web/src/avatar/input-manager.ts`
- Modify: `client/web/src/engine/input/_input-controller.test.ts`

- [ ] **Step 1: Write failing tests for gesture handlers**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
describe('InputManager - Gesture Events', () => {
    const mockCamera = {
        screenToWorldPlanePoint: (x: number, y: number) => new THREE.Vector3(x * 10, y * 10, 0)
    } as Camera;
    const mockWorldCursor = {} as WorldCursor;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockEvents = new EventTarget();
    });

    it('should emit CURSOR_MOVE_TO on tap', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let eventData: CursorMoveToEvent | null = null;
        mockEvents.addEventListener('cursor:move_to', (e: any) => { eventData = e.detail; });

        manager.onTap({ x: 10, y: 20 });

        expect(eventData).not.toBeNull();
        expect(eventData!.pos.x).toBe(100);
        expect(eventData!.pos.y).toBe(200);
    });

    it('should emit CURSOR_ZOOM_DELTA on pinch', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let eventData: CursorZoomDeltaEvent | null = null;
        mockEvents.addEventListener('cursor:zoom_delta', (e: any) => { eventData = e.detail; });

        const start: [Point, Point] = [{ x: 100, y: 100 }, { x: 200, y: 100 }];
        const current: [Point, Point] = [{ x: 100, y: 100 }, { x: 180, y: 100 }];

        manager.onPinchStart(start, start);
        manager.onPinch(start, current);

        expect(eventData).not.toBeNull();
        expect(eventData!.delta).toBeCloseTo(20, 1); // Distance decreased by 20
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "Gesture Events"`
Expected: FAIL - events not emitted

- [ ] **Step 3: Implement gesture handlers**

Modify `client/web/src/avatar/input-manager.ts`:

```typescript
export class InputManager implements InputHandler {
    private locomotionState: LocomotionState = {
        move: { x: 0, y: 0 },
        rotateRate: 0,
        zoomRate: 0,
        sprint: false
    };
    private activeDragPan = false;
    private activeDragRotate = false;
    private dragRotateLastPos: Point | null = null;
    private pinchStartDistance: number | null = null;
    private readonly dispatcher: EventDispatcher;

    // ... existing methods ...

    onControllerChanged(_controller: 'touch' | 'desktop'): void {}

    onTap(pos: Point): void {
        this.emitMoveTo(pos);
    }

    onInteractStart(_pos: Point): void {}
    onInteractDrag(_start: Point, _current: Point): void {}
    onInteractEnd(_pos: Point): void {}

    onDragPan(_start: Point, current: Point): void {
        if (this.isWASDActive) {
            this.activeDragPan = false;
            return;
        }
        if (!this.activeDragPan) return;

        this.emitMoveTo(current);
    }

    onDragPanEnd(current: Point): void {
        if (this.activeDragPan) {
            this.emitMoveTo(current);
        }
        this.activeDragPan = false;
    }

    // ... drag rotate methods ...

    onPinchStart(_start: [Point, Point], current: [Point, Point]): void {
        const dx = current[1].x - current[0].x;
        const dy = current[1].y - current[0].y;
        this.pinchStartDistance = Math.sqrt(dx * dx + dy * dy);
    }

    onPinch(_start: [Point, Point], current: [Point, Point]): void {
        if (this.pinchStartDistance === null) return;

        const dx = current[1].x - current[0].x;
        const dy = current[1].y - current[0].y;
        const currentDistance = Math.sqrt(dx * dx + dy * dy);

        const delta = this.pinchStartDistance - currentDistance;
        this.pinchStartDistance = currentDistance;

        this.dispatcher.dispatch<CursorZoomDeltaEvent>(CURSOR_ZOOM_DELTA, { delta });
    }

    onPinchEnd(_start: [Point, Point], _current: [Point, Point]): void {
        this.pinchStartDistance = null;
    }

    // ... rest of methods ...
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "Gesture Events"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add client/web/src/avatar/input-manager.ts client/web/src/engine/input/_input-controller.test.ts
git commit -m "feat(input): implement gesture handlers in InputManager"
```

---

## Task 7: Create CursorLocomotionSystem

**Files:**
- Create: `client/web/src/avatar/cursor-locomotion-system.ts`
- Modify: `client/web/src/engine/input/_input-controller.test.ts`
- Reference: `client/web/src/avatar/input-manager.ts` (LocomotionState)
- Reference: `client/web/src/avatar/world-cursor.ts` (WorldCursor setters)

- [ ] **Step 1: Write failing tests for CursorLocomotionSystem**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
import { CursorLocomotionSystem } from '../../../avatar/cursor-locomotion-system';
import type { LocomotionState } from '../../../avatar/input-manager';

describe('CursorLocomotionSystem', () => {
    let mockWorldCursor: any;
    let locomotionState: LocomotionState;

    beforeEach(() => {
        mockWorldCursor = {
            position: new THREE.Vector3(0, 0, 0),
            setPosition: vi.fn((pos: THREE.Vector3) => { mockWorldCursor.position.copy(pos); }),
            setYaw: vi.fn(),
            setZoom: vi.fn(),
            getCameraTarget: () => ({ yaw: 0, distance: 600 })
        };
        locomotionState = {
            move: { x: 0, y: 0 },
            rotateRate: 0,
            zoomRate: 0,
            sprint: false
        };
    });

    it('should apply world-space velocity from screen-space move delta', () => {
        const system = new CursorLocomotionSystem(mockWorldCursor, () => locomotionState);
        
        locomotionState.move = { x: 1, y: 0 }; // Right in screen space
        system.update(1.0); // 1 second

        expect(mockWorldCursor.setPosition).toHaveBeenCalled();
        const newPos = mockWorldCursor.setPosition.mock.calls[0][0];
        expect(newPos.x).toBeCloseTo(1200, 1); // CURSOR_MOVE_SPEED
        expect(newPos.y).toBeCloseTo(0, 1);
    });

    it('should apply sprint multiplier', () => {
        const system = new CursorLocomotionSystem(mockWorldCursor, () => locomotionState);
        
        locomotionState.move = { x: 1, y: 0 };
        locomotionState.sprint = true;
        system.update(1.0);

        const newPos = mockWorldCursor.setPosition.mock.calls[0][0];
        expect(newPos.x).toBeCloseTo(1200 * 3, 1); // CURSOR_MOVE_SPEED * CURSOR_SPRINT_MULTIPLIER
    });

    it('should rotate velocity by camera yaw', () => {
        mockWorldCursor.getCameraTarget = () => ({ yaw: Math.PI / 2, distance: 600 }); // 90 degrees
        const system = new CursorLocomotionSystem(mockWorldCursor, () => locomotionState);
        
        locomotionState.move = { x: 0, y: -1 }; // Forward in screen space
        system.update(1.0);

        const newPos = mockWorldCursor.setPosition.mock.calls[0][0];
        // Forward at yaw=90° should move in +x direction
        expect(newPos.x).toBeCloseTo(1200, 1);
        expect(newPos.y).toBeCloseTo(0, 1);
    });

    it('should apply rotate rate', () => {
        const system = new CursorLocomotionSystem(mockWorldCursor, () => locomotionState);
        
        locomotionState.rotateRate = Math.PI; // 180 degrees/sec
        system.update(0.5); // 0.5 seconds

        expect(mockWorldCursor.setYaw).toHaveBeenCalledWith(Math.PI / 2); // yaw + rate * dt
    });

    it('should apply zoom rate', () => {
        const system = new CursorLocomotionSystem(mockWorldCursor, () => locomotionState);
        
        locomotionState.zoomRate = 100;
        system.update(1.0);

        expect(mockWorldCursor.setZoom).toHaveBeenCalledWith(700); // 600 + 100 * 1.0
    });

    it('should not call setters when all rates are zero', () => {
        const system = new CursorLocomotionSystem(mockWorldCursor, () => locomotionState);
        
        system.update(1.0);

        expect(mockWorldCursor.setPosition).not.toHaveBeenCalled();
        expect(mockWorldCursor.setYaw).not.toHaveBeenCalled();
        expect(mockWorldCursor.setZoom).not.toHaveBeenCalled();
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "CursorLocomotionSystem"`
Expected: FAIL with "Cannot find module '../../../avatar/cursor-locomotion-system'"

- [ ] **Step 3: Create CursorLocomotionSystem**

Create: `client/web/src/avatar/cursor-locomotion-system.ts`

```typescript
import * as THREE from 'three';
import { CURSOR_MOVE_SPEED, CURSOR_SPRINT_MULTIPLIER } from '../constants';
import type { WorldCursor } from './world-cursor';
import type { LocomotionState } from './input-manager';

export class CursorLocomotionSystem {
    constructor(
        private readonly worldCursor: WorldCursor,
        private readonly getLocomotionState: () => Readonly<LocomotionState>
    ) {}

    update(deltaTime: number): void {
        const state = this.getLocomotionState();

        // Apply movement
        if (state.move.x !== 0 || state.move.y !== 0) {
            const { yaw } = this.worldCursor.getCameraTarget();
            
            // Convert screen-space input to world-space velocity
            const forward = new THREE.Vector3(Math.sin(yaw), Math.cos(yaw), 0);
            const right = new THREE.Vector3(Math.cos(yaw), -Math.sin(yaw), 0);

            forward.multiplyScalar(-state.move.y);
            right.multiplyScalar(state.move.x);
            forward.add(right);
            forward.normalize();

            const speed = CURSOR_MOVE_SPEED * (state.sprint ? CURSOR_SPRINT_MULTIPLIER : 1);
            forward.multiplyScalar(speed * deltaTime);

            const newPosition = this.worldCursor.position.clone().add(forward);
            this.worldCursor.setPosition(newPosition);
        }

        // Apply rotation
        if (state.rotateRate !== 0) {
            const { yaw } = this.worldCursor.getCameraTarget();
            this.worldCursor.setYaw(yaw + state.rotateRate * deltaTime);
        }

        // Apply zoom
        if (state.zoomRate !== 0) {
            const { distance } = this.worldCursor.getCameraTarget();
            this.worldCursor.setZoom(distance + state.zoomRate * deltaTime);
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "CursorLocomotionSystem"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add client/web/src/avatar/cursor-locomotion-system.ts client/web/src/engine/input/_input-controller.test.ts
git commit -m "feat(input): create CursorLocomotionSystem for polling locomotion state"
```

---

## Task 8: Refactor WorldCursor to be passive (remove update(), add setters)

**Files:**
- Modify: `client/web/src/avatar/world-cursor.ts`
- Modify: `client/web/src/engine/input/_input-controller.test.ts`

- [ ] **Step 1: Write failing tests for WorldCursor setters**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
import { WorldCursor } from '../../../avatar/world-cursor';
import type { RenderContext } from '../../render-context';

describe('WorldCursor - Setters', () => {
    let mockRenderContext: RenderContext;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockRenderContext = { scene: new THREE.Scene() } as RenderContext;
        mockEvents = new EventTarget();
    });

    it('setPosition should update position', () => {
        const cursor = new WorldCursor(mockRenderContext, mockEvents);
        const newPos = new THREE.Vector3(100, 200, 0);
        
        cursor.setPosition(newPos);
        
        expect(cursor.position.x).toBe(100);
        expect(cursor.position.y).toBe(200);
    });

    it('setYaw should normalize angle to [0, 2π)', () => {
        const cursor = new WorldCursor(mockRenderContext, mockEvents);
        
        cursor.setYaw(Math.PI * 3); // 3π
        const { yaw } = cursor.getCameraTarget();
        
        expect(yaw).toBeCloseTo(Math.PI, 5); // Normalized to π
    });

    it('setYaw with negative angle should normalize to [0, 2π)', () => {
        const cursor = new WorldCursor(mockRenderContext, mockEvents);
        
        cursor.setYaw(-Math.PI / 2);
        const { yaw } = cursor.getCameraTarget();
        
        expect(yaw).toBeCloseTo(Math.PI * 1.5, 5); // -π/2 → 3π/2
    });

    it('setZoom should clamp to [MIN_CAMERA_DISTANCE, MAX_CAMERA_DISTANCE]', () => {
        const cursor = new WorldCursor(mockRenderContext, mockEvents);
        
        cursor.setZoom(20); // Below min
        expect(cursor.getCameraTarget().distance).toBe(40); // MIN_CAMERA_DISTANCE

        cursor.setZoom(20000); // Above max
        expect(cursor.getCameraTarget().distance).toBe(15000); // MAX_CAMERA_DISTANCE

        cursor.setZoom(500); // Within range
        expect(cursor.getCameraTarget().distance).toBe(500);
    });

    it('getCameraTarget should return consistent values after setters', () => {
        const cursor = new WorldCursor(mockRenderContext, mockEvents);
        
        cursor.setPosition(new THREE.Vector3(100, 200, 0));
        cursor.setYaw(Math.PI / 4);
        cursor.setZoom(800);

        const target = cursor.getCameraTarget();
        expect(target.cursorPosition.x).toBe(100);
        expect(target.cursorPosition.y).toBe(200);
        expect(target.yaw).toBeCloseTo(Math.PI / 4, 5);
        expect(target.distance).toBe(800);
    });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "WorldCursor - Setters"`
Expected: FAIL - setPosition/setYaw/setZoom methods not defined

- [ ] **Step 3: Refactor WorldCursor - remove update(), add setters**

Modify `client/web/src/avatar/world-cursor.ts`:

```typescript
import * as THREE from 'three';
import {
    MAX_CAMERA_DISTANCE,
    MAX_CAMERA_PITCH,
    MIN_CAMERA_DISTANCE,
    MIN_CAMERA_PITCH
} from '../constants';
import { EventSubscriptions } from '../engine/events';
import type { RenderContext } from '../engine/render-context';
import { WORLD_REFERENCE_CHANGED, type WorldReferenceChangedEvent } from '../systems/world-reference-system';
import {
    CURSOR_MOVE_TO,
    CURSOR_ROTATE_DELTA,
    CURSOR_ZOOM_DELTA,
    type CursorMoveToEvent,
    type CursorRotateDeltaEvent,
    type CursorZoomDeltaEvent
} from './events';

export class WorldCursor {
    readonly position = new THREE.Vector3(0, 0, 0);
    private readonly direction = new THREE.Vector3(0, 1, 0);
    private cameraDistance = 600;
    private cameraYaw = 0;
    private mesh: THREE.Mesh | null = null;
    private readonly subscriptions: EventSubscriptions;

    constructor(
        private readonly renderContext: RenderContext,
        events: EventTarget
    ) {
        this.subscriptions = new EventSubscriptions(events);

        this.subscriptions.on<CursorMoveToEvent>(CURSOR_MOVE_TO, this.handleCursorMoveTo);
        this.subscriptions.on<CursorRotateDeltaEvent>(CURSOR_ROTATE_DELTA, this.handleCursorRotateDelta);
        this.subscriptions.on<CursorZoomDeltaEvent>(CURSOR_ZOOM_DELTA, this.handleCursorZoomDelta);
        this.subscriptions.on<WorldReferenceChangedEvent>(WORLD_REFERENCE_CHANGED, this.handleWorldReferenceChanged);
    }

    setPosition(pos: THREE.Vector3): void {
        this.position.copy(pos);
        if (this.mesh) {
            this.mesh.position.copy(this.position);
        }
    }

    setYaw(yaw: number): void {
        // Normalize to [0, 2π)
        this.cameraYaw = ((yaw % (Math.PI * 2)) + Math.PI * 2) % (Math.PI * 2);
        
        // Update direction vector
        const cos = Math.cos(yaw - this.cameraYaw);
        const sin = Math.sin(yaw - this.cameraYaw);
        const fx = this.direction.x * cos - this.direction.y * sin;
        const fy = this.direction.x * sin + this.direction.y * cos;
        this.direction.set(fx, fy, 0);

        if (this.mesh && this.direction.lengthSq() > 0) {
            const angle = Math.atan2(this.direction.x, this.direction.y);
            this.mesh.rotation.z = angle;
        }
    }

    setZoom(distance: number): void {
        this.cameraDistance = Math.max(MIN_CAMERA_DISTANCE, Math.min(MAX_CAMERA_DISTANCE, distance));
    }

    get showMesh(): boolean {
        return this.mesh?.visible ?? false;
    }

    set showMesh(value: boolean) {
        if (value) {
            this.createMesh();
        } else {
            this.disposeMesh();
        }
    }

    dispose(): void {
        this.subscriptions.dispose();
        this.disposeMesh();
    }

    getCameraTarget(): {
        distance: number;
        yaw: number;
        cursorPosition: THREE.Vector3;
        position: THREE.Vector3;
        lookAt: THREE.Vector3;
    } {
        const t = (this.cameraDistance - MIN_CAMERA_DISTANCE) / (MAX_CAMERA_DISTANCE - MIN_CAMERA_DISTANCE);
        const pitch = MIN_CAMERA_PITCH + t * (MAX_CAMERA_PITCH - MIN_CAMERA_PITCH);

        const height = this.cameraDistance * Math.sin(pitch);
        const horizontalDist = this.cameraDistance * Math.cos(pitch);

        const position = new THREE.Vector3(
            this.position.x - horizontalDist * Math.sin(this.cameraYaw),
            this.position.y - horizontalDist * Math.cos(this.cameraYaw),
            this.position.z + height
        );

        const lookAt = this.position.clone().add(this.direction.clone().multiplyScalar(10));

        return {
            distance: this.cameraDistance,
            yaw: this.cameraYaw,
            cursorPosition: this.position.clone(),
            position,
            lookAt
        };
    }

    private applyYawDelta(angleDelta: number): void {
        this.setYaw(this.cameraYaw + angleDelta);
    }

    private handleCursorMoveTo = (event: CursorMoveToEvent): void => {
        this.setPosition(event.pos);
    };

    private handleCursorRotateDelta = (event: CursorRotateDeltaEvent): void => {
        this.applyYawDelta(event.angleDelta);
    };

    private handleCursorZoomDelta = (event: CursorZoomDeltaEvent): void => {
        const midDistance = (MAX_CAMERA_DISTANCE + MIN_CAMERA_DISTANCE) / 2;
        const zoomScale = (this.cameraDistance / midDistance) * 25; // ZOOM_DISTANCE_SCALE

        this.setZoom(this.cameraDistance + event.delta * zoomScale);
    };

    private handleWorldReferenceChanged = (event: WorldReferenceChangedEvent): void => {
        this.position.x += event.deltaPosition.x;
        this.position.y += event.deltaPosition.y;
        if (this.mesh) {
            this.mesh.position.copy(this.position);
        }
    };

    private createMesh(): void {
        if (this.mesh) return;

        const geometry = new THREE.BufferGeometry();
        const headLength = 30;
        const baseWidth = 15;
        const vertices = new Float32Array([
            0,
            headLength,
            0,
            -baseWidth,
            -5,
            0,
            baseWidth,
            -5,
            0
        ]);
        geometry.setAttribute('position', new THREE.BufferAttribute(vertices, 3));
        geometry.setIndex([0, 1, 2]);

        const material = new THREE.MeshBasicMaterial({
            color: 0x0000ff,
            side: THREE.DoubleSide,
            depthTest: false,
            depthWrite: false
        });
        this.mesh = new THREE.Mesh(geometry, material);
        this.mesh.renderOrder = 998;
        this.mesh.frustumCulled = false;
        this.mesh.position.copy(this.position);

        if (this.direction.lengthSq() > 0) {
            const angle = Math.atan2(this.direction.x, this.direction.y);
            this.mesh.rotation.z = angle;
        }

        this.renderContext.scene.add(this.mesh);
    }

    private disposeMesh(): void {
        if (!this.mesh) return;

        this.mesh.parent?.remove(this.mesh);
        this.mesh.geometry.dispose();
        (this.mesh.material as THREE.Material).dispose();
        this.mesh = null;
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "WorldCursor - Setters"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add client/web/src/avatar/world-cursor.ts client/web/src/engine/input/_input-controller.test.ts
git commit -m "refactor(input): make WorldCursor passive with setters, remove update()"
```

---

## Task 9: Remove stale event types from events.ts

**Files:**
- Modify: `client/web/src/avatar/events.ts`

- [ ] **Step 1: Write test confirming gesture events still work**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
describe('Events - Gesture Events Only', () => {
    it('should have CURSOR_MOVE_TO event', () => {
        expect(CURSOR_MOVE_TO).toBe('cursor:move_to');
    });

    it('should have CURSOR_ROTATE_DELTA event', () => {
        expect(CURSOR_ROTATE_DELTA).toBe('cursor:rotate_delta');
    });

    it('should have CURSOR_ZOOM_DELTA event', () => {
        expect(CURSOR_ZOOM_DELTA).toBe('cursor:zoom_delta');
    });
});
```

- [ ] **Step 2: Run test to verify it passes**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "Gesture Events Only"`
Expected: PASS

- [ ] **Step 3: Remove stale locomotion events**

Modify `client/web/src/avatar/events.ts`:

```typescript
import type * as THREE from 'three';

// Controller selection
export const INPUT_CONTROLLER_CHANGED = 'input:controller:changed';
export type InputControllerChangedEvent = { controller: 'touch' | 'desktop' };

// Gesture event: instant position change
export const CURSOR_MOVE_TO = 'cursor:move_to';
export type CursorMoveToEvent = { pos: THREE.Vector3 };

// Gesture event: instant rotation offset in radians; applied immediately without integration
export const CURSOR_ROTATE_DELTA = 'cursor:rotate_delta';
export type CursorRotateDeltaEvent = { angleDelta: number };

// Gesture event: instant zoom offset (world units); applied immediately without integration
export const CURSOR_ZOOM_DELTA = 'cursor:zoom_delta';
export type CursorZoomDeltaEvent = { delta: number };
```

- [ ] **Step 4: Run test to verify gesture events still work**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "Gesture Events Only"`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add client/web/src/avatar/events.ts client/web/src/engine/input/_input-controller.test.ts
git commit -m "refactor(input): remove stale locomotion events (CURSOR_MOVE, CURSOR_ROTATE, CURSOR_ZOOM)"
```

---

## Task 10: Remove conflict resolution from DesktopController

**Files:**
- Modify: `client/web/src/engine/input/_desktop-controller.ts`

- [ ] **Step 1: Document changes needed in DesktopController**

No test needed - this is a removal task. Changes:
- Remove lines 127-136 (WASD/Q/E check blocking drag-pan start)
- Remove lines 147-152 (Q/E check blocking drag-rotate start)
- Remove lines 291-294 (R/F check blocking wheel zoom)

- [ ] **Step 2: Remove conflict resolution from drag-pan**

Modify `client/web/src/engine/input/_desktop-controller.ts`, around line 119:

Remove this block:
```typescript
        // Block pan when WASD or Q/E rotation is active (conflicting camera movement)
        if (
            this.keys.up ||
            this.keys.left ||
            this.keys.down ||
            this.keys.right ||
            this.rotateKeys.left ||
            this.rotateKeys.right
        ) {
            return;
        }
```

Replace with:
```typescript
        // Conflict resolution now in InputManager
```

- [ ] **Step 3: Remove conflict resolution from drag-rotate**

Modify `client/web/src/engine/input/_desktop-controller.ts`, around line 147:

Remove this block:
```typescript
        if (
            this.owningPointer.button === 2 &&
            !this.activeDragPan &&
            !this.activeDragRotate &&
            !this.rotateKeys.left &&
            !this.rotateKeys.right &&
            totalMoved > MOVE_THRESHOLD_PX
        ) {
```

Replace with:
```typescript
        if (
            this.owningPointer.button === 2 &&
            !this.activeDragPan &&
            !this.activeDragRotate &&
            totalMoved > MOVE_THRESHOLD_PX
        ) {
            // Conflict resolution now in InputManager
```

- [ ] **Step 4: Remove conflict resolution from wheel zoom**

Modify `client/web/src/engine/input/_desktop-controller.ts`, line 290:

Remove this block:
```typescript
    handleWheel(ev: WheelEvent): void {
        // Block wheel during interact or when zoom keys are active
        if (this.activeInteract || this.zoomKeys.in || this.zoomKeys.out) {
            return;
        }
```

Replace with:
```typescript
    handleWheel(ev: WheelEvent): void {
        // Block wheel during interact (zoom key conflict resolution now in InputManager)
        if (this.activeInteract) {
            return;
        }
```

- [ ] **Step 5: Verify DesktopController still compiles**

Run: `cd client/web && pnpm build`
Expected: Success (no type errors)

- [ ] **Step 6: Commit**

```bash
git add client/web/src/engine/input/_desktop-controller.ts
git commit -m "refactor(input): remove conflict resolution from DesktopController"
```

---

## Task 11: Delete input-mapper.ts (functionality moved to InputManager)

**Files:**
- Delete: `client/web/src/avatar/input-mapper.ts`

- [ ] **Step 1: Verify input-mapper.ts is not imported elsewhere**

Run: `cd client/web && grep -r "from.*input-mapper" src/`
Expected: No results (or only in files we're about to update)

- [ ] **Step 2: Delete input-mapper.ts**

```bash
rm client/web/src/avatar/input-mapper.ts
```

- [ ] **Step 3: Verify project still compiles**

Run: `cd client/web && pnpm build`
Expected: Success (or errors in wiring code we haven't updated yet)

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "refactor(input): delete input-mapper.ts (functionality moved to InputManager)"
```

---

## Task 12: Add remaining test coverage (tests 20-34 from test plan)

**Files:**
- Modify: `client/web/src/engine/input/_input-controller.test.ts`

- [ ] **Step 1: Add tests for WASD+interact blocking**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
describe('InputManager - WASD blocks interact', () => {
    const mockCamera = {} as Camera;
    const mockWorldCursor = {} as WorldCursor;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockEvents = new EventTarget();
    });

    it('should block interact (long-press) when WASD held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let interactStartEmitted = false;
        mockEvents.addEventListener('interact:start', () => { interactStartEmitted = true; });

        manager.onMove({ x: 1, y: 0 }, false); // WASD active
        manager.onInteractStart({ x: 100, y: 100 });

        expect(interactStartEmitted).toBe(false);
    });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "WASD blocks interact"`
Expected: FAIL - interact not blocked

- [ ] **Step 3: Implement WASD blocks interact**

Modify `client/web/src/avatar/input-manager.ts`:

```typescript
export class InputManager implements InputHandler {
    // ... existing code ...

    onInteractStart(_pos: Point): void {
        // Block if WASD active
        if (this.isWASDActive) {
            return;
        }
        // TODO: Implement interact functionality when needed
    }

    onInteractDrag(_start: Point, _current: Point): void {
        // TODO: Implement when needed
    }

    onInteractEnd(_pos: Point): void {
        // TODO: Implement when needed
    }

    // ... rest of code ...
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "WASD blocks interact"`
Expected: PASS

- [ ] **Step 5: Add tests for concurrent WASD+Q/E**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
describe('InputManager - Concurrent WASD+Q/E', () => {
    const mockCamera = {} as Camera;
    const mockWorldCursor = {} as WorldCursor;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockEvents = new EventTarget();
    });

    it('should have both move and rotateRate in locomotion state', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        
        manager.onMove({ x: 1, y: 0 }, false);
        manager.onRotate(-1);

        const state = manager.getLocomotionState();
        expect(state.move.x).toBe(1);
        expect(state.rotateRate).toBe(-90 * (Math.PI / 180));
    });

    it('should block both drag-pan and drag-rotate when both WASD+Q/E held', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let moveToEmitted = false;
        let rotateDeltaEmitted = false;
        mockEvents.addEventListener('cursor:move_to', () => { moveToEmitted = true; });
        mockEvents.addEventListener('cursor:rotate_delta', () => { rotateDeltaEmitted = true; });

        manager.onMove({ x: 1, y: 0 }, false); // WASD
        manager.onRotate(-1); // Q/E

        manager.onDragPanStart({ x: 100, y: 100 });
        manager.onDragPan({ x: 100, y: 100 }, { x: 150, y: 150 });

        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onDragRotate({ x: 100, y: 100 }, { x: 150, y: 100 });

        expect(moveToEmitted).toBe(false);
        expect(rotateDeltaEmitted).toBe(false);
    });
});
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "Concurrent WASD+Q/E"`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add client/web/src/avatar/input-manager.ts client/web/src/engine/input/_input-controller.test.ts
git commit -m "test(input): add tests for WASD+interact blocking and concurrent keys"
```

---

## Task 13: Add test coverage for gesture event correctness (tests 30-34)

**Files:**
- Modify: `client/web/src/engine/input/_input-controller.test.ts`

- [ ] **Step 1: Add tests for gesture event correctness**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
describe('InputManager - Gesture Event Correctness', () => {
    const mockCamera = {
        screenToWorldPlanePoint: (x: number, y: number) => new THREE.Vector3(x * 10, y * 10, 0)
    } as Camera;
    const mockWorldCursor = {} as WorldCursor;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockEvents = new EventTarget();
    });

    it('tap should emit CURSOR_MOVE_TO with raycasted world position', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let eventData: CursorMoveToEvent | null = null;
        mockEvents.addEventListener('cursor:move_to', (e: any) => { eventData = e.detail; });

        manager.onTap({ x: 50, y: 100 });

        expect(eventData).not.toBeNull();
        expect(eventData!.pos.x).toBe(500);
        expect(eventData!.pos.y).toBe(1000);
        expect(eventData!.pos.z).toBe(0);
    });

    it('drag-pan should emit CURSOR_MOVE_TO each frame', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let eventCount = 0;
        mockEvents.addEventListener('cursor:move_to', () => { eventCount++; });

        manager.onDragPanStart({ x: 100, y: 100 });
        manager.onDragPan({ x: 100, y: 100 }, { x: 110, y: 110 });
        manager.onDragPan({ x: 100, y: 100 }, { x: 120, y: 120 });
        manager.onDragPanEnd({ x: 130, y: 130 });

        expect(eventCount).toBe(3); // Two frames + end
    });

    it('drag-rotate should emit CURSOR_ROTATE_DELTA with correct angleDelta', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let eventData: CursorRotateDeltaEvent | null = null;
        mockEvents.addEventListener('cursor:rotate_delta', (e: any) => { eventData = e.detail; });

        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onDragRotate({ x: 100, y: 100 }, { x: 150, y: 100 }); // 50px right

        expect(eventData).not.toBeNull();
        expect(eventData!.angleDelta).toBeCloseTo(50 * 0.005, 5); // ROTATE_SENSITIVITY
    });

    it('pinch should emit CURSOR_ZOOM_DELTA proportional to distance change', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let eventData: CursorZoomDeltaEvent | null = null;
        mockEvents.addEventListener('cursor:zoom_delta', (e: any) => { eventData = e.detail; });

        const start: [Point, Point] = [{ x: 100, y: 100 }, { x: 200, y: 100 }];
        const current: [Point, Point] = [{ x: 100, y: 100 }, { x: 180, y: 100 }];

        manager.onPinchStart(start, start);
        manager.onPinch(start, current);

        expect(eventData).not.toBeNull();
        expect(eventData!.delta).toBeCloseTo(20, 1); // Distance decreased by 20
    });

    it('wheel should emit CURSOR_ZOOM_DELTA with scaled delta', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let eventData: CursorZoomDeltaEvent | null = null;
        mockEvents.addEventListener('cursor:zoom_delta', (e: any) => { eventData = e.detail; });

        manager.onZoomTo({ x: 100, y: 100 }, 10);

        expect(eventData).not.toBeNull();
        expect(eventData!.delta).toBeCloseTo(10 * 0.5, 5); // ZOOM_SENSITIVITY
    });
});
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "Gesture Event Correctness"`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add client/web/src/engine/input/_input-controller.test.ts
git commit -m "test(input): add gesture event correctness tests"
```

---

## Task 14: Add test coverage for WorldCursor gesture event handlers (tests 38-41)

**Files:**
- Modify: `client/web/src/engine/input/_input-controller.test.ts`

- [ ] **Step 1: Add tests for WorldCursor gesture event handlers**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
describe('WorldCursor - Gesture Event Handlers', () => {
    let mockRenderContext: RenderContext;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockRenderContext = { scene: new THREE.Scene() } as RenderContext;
        mockEvents = new EventTarget();
    });

    it('CURSOR_MOVE_TO should set position to event value', () => {
        const cursor = new WorldCursor(mockRenderContext, mockEvents);
        
        mockEvents.dispatchEvent(new CustomEvent('cursor:move_to', {
            detail: { pos: new THREE.Vector3(100, 200, 0) }
        }));

        expect(cursor.position.x).toBe(100);
        expect(cursor.position.y).toBe(200);
    });

    it('CURSOR_ROTATE_DELTA should update yaw by delta', () => {
        const cursor = new WorldCursor(mockRenderContext, mockEvents);
        const initialYaw = cursor.getCameraTarget().yaw;
        
        mockEvents.dispatchEvent(new CustomEvent('cursor:rotate_delta', {
            detail: { angleDelta: Math.PI / 4 }
        }));

        const newYaw = cursor.getCameraTarget().yaw;
        expect(newYaw).toBeCloseTo(initialYaw + Math.PI / 4, 5);
    });

    it('CURSOR_ZOOM_DELTA should update zoom distance by delta', () => {
        const cursor = new WorldCursor(mockRenderContext, mockEvents);
        const initialDistance = cursor.getCameraTarget().distance;
        
        // Small delta to stay within zoom scale calculations
        mockEvents.dispatchEvent(new CustomEvent('cursor:zoom_delta', {
            detail: { delta: 10 }
        }));

        const newDistance = cursor.getCameraTarget().distance;
        expect(newDistance).toBeGreaterThan(initialDistance);
    });

    it('CURSOR_ZOOM_DELTA should clamp result', () => {
        const cursor = new WorldCursor(mockRenderContext, mockEvents);
        
        // Large negative delta
        mockEvents.dispatchEvent(new CustomEvent('cursor:zoom_delta', {
            detail: { delta: -10000 }
        }));

        const distance = cursor.getCameraTarget().distance;
        expect(distance).toBe(40); // MIN_CAMERA_DISTANCE
    });
});
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "WorldCursor - Gesture Event Handlers"`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add client/web/src/engine/input/_input-controller.test.ts
git commit -m "test(input): add WorldCursor gesture event handler tests"
```

---

## Task 15: Add test coverage for CursorLocomotionSystem edge cases (tests 44, 46, 49)

**Files:**
- Modify: `client/web/src/engine/input/_input-controller.test.ts`

- [ ] **Step 1: Add CursorLocomotionSystem edge case tests**

Add to `client/web/src/engine/input/_input-controller.test.ts`:

```typescript
describe('CursorLocomotionSystem - Edge Cases', () => {
    let mockWorldCursor: any;
    let locomotionState: LocomotionState;

    beforeEach(() => {
        mockWorldCursor = {
            position: new THREE.Vector3(100, 200, 0),
            setPosition: vi.fn((pos: THREE.Vector3) => { mockWorldCursor.position.copy(pos); }),
            setYaw: vi.fn(),
            setZoom: vi.fn(),
            getCameraTarget: () => ({ yaw: 0, distance: 600 })
        };
        locomotionState = {
            move: { x: 0, y: 0 },
            rotateRate: 0,
            zoomRate: 0,
            sprint: false
        };
    });

    it('should not change position when move delta is zero', () => {
        const system = new CursorLocomotionSystem(mockWorldCursor, () => locomotionState);
        const initialPos = mockWorldCursor.position.clone();
        
        locomotionState.move = { x: 0, y: 0 };
        system.update(1.0);

        expect(mockWorldCursor.setPosition).not.toHaveBeenCalled();
        expect(mockWorldCursor.position.x).toBe(initialPos.x);
        expect(mockWorldCursor.position.y).toBe(initialPos.y);
    });

    it('should use current yaw for velocity, not previous frame yaw', () => {
        const system = new CursorLocomotionSystem(mockWorldCursor, () => locomotionState);
        
        locomotionState.move = { x: 0, y: -1 }; // Forward
        system.update(0.1);

        // Change yaw
        mockWorldCursor.getCameraTarget = () => ({ yaw: Math.PI / 2, distance: 600 });
        
        locomotionState.move = { x: 0, y: -1 }; // Still forward
        system.update(0.1);

        // Second update should use new yaw (forward at 90° = +x direction)
        const secondCall = mockWorldCursor.setPosition.mock.calls[1][0];
        expect(secondCall.x).toBeGreaterThan(100); // Moved in +x direction
    });

    it('should not call setters when all rates are zero', () => {
        const system = new CursorLocomotionSystem(mockWorldCursor, () => locomotionState);
        
        system.update(1.0);

        expect(mockWorldCursor.setPosition).not.toHaveBeenCalled();
        expect(mockWorldCursor.setYaw).not.toHaveBeenCalled();
        expect(mockWorldCursor.setZoom).not.toHaveBeenCalled();
    });
});
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cd client/web && pnpm test _input-controller.test.ts -t "CursorLocomotionSystem - Edge Cases"`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add client/web/src/engine/input/_input-controller.test.ts
git commit -m "test(input): add CursorLocomotionSystem edge case tests"
```

---

## Task 16: Run full test suite and verify all 49+ tests pass

**Files:**
- Verify: `client/web/src/engine/input/_input-controller.test.ts`

- [ ] **Step 1: Run full test suite**

Run: `cd client/web && pnpm test _input-controller.test.ts`
Expected: All tests PASS (49+ tests covering all test plan items)

- [ ] **Step 2: Verify coverage of all test plan items**

Checklist of test plan items (from docs/input-system.html):
- [x] Tests 1-7: WASD blocks
- [x] Tests 8-13: Q/E blocks
- [x] Tests 14-17: R/F blocks
- [x] Tests 18-19: WASD + Q/E concurrent
- [x] Tests 20-23: No keys — all gestures allowed
- [x] Tests 24-29: Locomotion state correctness
- [x] Tests 30-34: Gesture event correctness
- [x] Tests 35-37: WorldCursor setters
- [x] Tests 38-41: WorldCursor gesture event handlers
- [x] Tests 42-49: CursorLocomotionSystem

- [ ] **Step 3: Generate test coverage report (optional)**

Run: `cd client/web && pnpm test:coverage _input-controller.test.ts`
Expected: High coverage for InputManager, CursorLocomotionSystem, WorldCursor

- [ ] **Step 4: Commit any final test adjustments**

```bash
git add client/web/src/engine/input/_input-controller.test.ts
git commit -m "test(input): verify all 49 test cases pass"
```

---

## Task 17: Update integration points (wiring)

**Files:**
- Identify and update files that create/use InputMapper (now InputManager)
- Identify and add CursorLocomotionSystem to game loop

- [ ] **Step 1: Find files importing InputMapper**

Run: `cd client/web && grep -r "InputMapper" src/`
Expected: List of files that need updates

- [ ] **Step 2: Search for where input system is initialized**

Run: `cd client/web && grep -r "new .*Controller" src/ | grep -i "desktop\|touch"`
Expected: Find main initialization point

- [ ] **Step 3: Document wiring changes needed**

Create a checklist:
- Replace `InputMapper` imports with `InputManager`
- Create `CursorLocomotionSystem` instance
- Add `CursorLocomotionSystem.update(dt)` to game loop
- Pass `InputManager` to controllers instead of `InputMapper`

- [ ] **Step 4: Note for user**

```
MANUAL WIRING REQUIRED:

1. Find where DesktopController/TouchController are created
2. Replace InputMapper with InputManager:
   - Change: new InputMapper(camera, worldCursor, events)
   - To: new InputManager(camera, worldCursor, events)

3. Create CursorLocomotionSystem:
   const locomotionSystem = new CursorLocomotionSystem(
     worldCursor,
     () => inputManager.getLocomotionState()
   );

4. Add to game loop update():
   locomotionSystem.update(deltaTime);

5. Remove worldCursor.update(deltaTime) from game loop (if present)
```

- [ ] **Step 5: Create stub documentation**

Create: `docs/superpowers/plans/input-system-wiring-notes.md`

```markdown
# Input System Refactor - Wiring Notes

## Changes Required

### 1. Replace InputMapper with InputManager

**Before:**
```typescript
import { InputMapper } from './avatar/input-mapper';
const inputMapper = new InputMapper(camera, worldCursor, events);
```

**After:**
```typescript
import { InputManager } from './avatar/input-manager';
const inputManager = new InputManager(camera, worldCursor, events);
```

### 2. Create CursorLocomotionSystem

```typescript
import { CursorLocomotionSystem } from './avatar/cursor-locomotion-system';

const locomotionSystem = new CursorLocomotionSystem(
  worldCursor,
  () => inputManager.getLocomotionState()
);
```

### 3. Update Game Loop

**Remove:**
```typescript
worldCursor.update(deltaTime);
```

**Add:**
```typescript
locomotionSystem.update(deltaTime);
```

### 4. Update Controller Construction

Controllers now receive InputManager (implements InputHandler):

```typescript
const desktopController = new DesktopController(inputManager);
const touchController = new TouchController(inputManager);
```

## Testing After Wiring

1. WASD movement should work
2. Drag-pan should work
3. WASD should block drag-pan
4. Q/E should block drag-rotate
5. R/F should block wheel zoom
6. All gestures should emit correctly
```

- [ ] **Step 6: Commit wiring documentation**

```bash
git add docs/superpowers/plans/input-system-wiring-notes.md
git commit -m "docs(input): add wiring notes for input system refactor"
```

---

## Self-Review Checklist

After completing all tasks, verify:

- [ ] All 49 test cases from the test plan are implemented and passing
- [ ] No placeholder code (TBD, TODO without implementation)
- [ ] Type signatures consistent across all tasks
- [ ] All files in the doc's file map are accounted for (created, modified, or deleted)
- [ ] Conflict resolution moved from DesktopController to InputManager
- [ ] Locomotion changed from event-driven to poll-based
- [ ] WorldCursor is passive (no update(), only setters)
- [ ] Stale events removed (CURSOR_MOVE, CURSOR_ROTATE, CURSOR_ZOOM)
- [ ] Wiring documentation provided for integration

---

## Execution Notes

This is a significant architectural refactor spanning 6 files with 49 test cases. Each task is self-contained with tests written first (TDD). The plan maintains backward compatibility during the refactor by adding new components before removing old ones.

**Key architectural changes:**
1. Conflict resolution: DesktopController → InputManager
2. Locomotion: Event-driven → Poll-based
3. WorldCursor: Active (update) → Passive (setters only)
4. Data flow: Controllers → InputManager (stores state) → CursorLocomotionSystem (polls) → WorldCursor (mutates)
