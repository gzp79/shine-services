# InputController Documentation

## Overview

The `InputController` is a unified gesture recognition system that converts raw pointer events (mouse, touch, pen) and keyboard input into semantic input events for 3D viewer applications. Pen input is treated as mouse input. It uses a **sub-controller architecture** to cleanly separate touch-based and desktop-based interaction models.

**Location:** `client/web/src/engine/input-controller.ts`

**Architecture:** Main orchestrator + two sub-controllers
- **TouchController:** Single/multi-finger gestures (tap, pan, pinch)
- **DesktopController:** Mouse buttons, WASD keys, wheel (tap, pan, rotate, WASD movement, zoom)
- **Orchestrator:** Routes input to active sub-controller, prevents mixing

**Key Features:**
- ✅ Sub-controller architecture (touch vs desktop separation)
- ✅ First-input-wins controller selection (no accidental input mixing)
- ✅ Selected/active state model (selected persists, active tracks input)
- ✅ WASD keyboard movement concurrent with mouse rotate + wheel zoom
- ✅ Dual-button mouse support (left=pan/interact, right=rotate)
- ✅ Movement-based disambiguation (pan vs pinch)
- ✅ Time-based disambiguation (tap vs long press)
- ✅ Mutual exclusion per input source (no overlapping pointer gestures)
- ✅ Raw data output (consumers calculate zoom/rotation from start+current positions)
- ✅ Focus loss and pointer capture handling (clean gesture termination)

---

## Architecture

### Sub-Controller Design

```
┌─────────────────────────────────────────────────────────┐
│               InputController (Orchestrator)            │
├─────────────────────────────────────────────────────────┤
│  State:                                                 │
│  • selected: 'touch' | 'desktop' (never null)           │
│  • active: 'touch' | 'desktop' | null                   │
│                                                         │
│  Responsibilities:                                      │
│  - Detect input type (touch vs mouse/keyboard)          │
│  - Activate controller on first input                   │
│  - Update selected = active, emit event if changed      │
│  - Route events ONLY to active controller               │
│  - Inactive controller receives zero events             │
└──────────┬──────────────────────────┬───────────────────┘
           │                          │
   ┌───────▼─────────┐       ┌────────▼─────────────┐
   │ TouchController │       │ DesktopController    │
   ├─────────────────┤       ├──────────────────────┤
   │ Inputs:         │       │ Inputs:              │
   │ • Touch events  │       │ • Mouse buttons      │
   │                 │       │ • Mouse wheel        │
   │ Gestures:       │       │ • WASD keys          │
   │ • Tap           │       │                      │
   │ • Long press    │       │ Gestures:            │
   │ • Pan (1-finger)│       │ • Tap (left click)   │
   │ • Pinch (2-fngr)│       │ • Long press (left)  │
   │                 │       │ • Pan (left drag)    │
   │ Rules:          │       │ • Rotate (right drag)│
   │ • Mutually      │       │ • WASD movement      │
   │   exclusive     │       │ • Wheel zoom         │
   │                 │       │                      │
   │                 │       │ Rules:               │
   │                 │       │ • Pointer gestures   │
   │                 │       │   mutually exclusive │
   │                 │       │ • WASD + Rotate +    │
   │                 │       │   Zoom concurrent    │
   └─────────────────┘       └──────────────────────┘
```

### Controller Selection

**Orchestrator State:**
```typescript
selected: 'touch' | 'desktop'  // Last used controller (never null, default: 'desktop')
active: 'touch' | 'desktop' | null  // Currently tracking inputs (null when idle)
```

**State Machine:**
```
Initial: selected = 'desktop', active = null
  ↓
Touch pointer down
  ↓
active = 'touch'
  ↓
selected ≠ active? → selected = 'touch', emit INPUT_CONTROLLER_CHANGED
  ↓
TouchController processes events (DesktopController completely disabled)
  ↓
All touch pointers up
  ↓
active = null (selected stays 'touch')
  ↓
Mouse/keyboard input
  ↓
active = 'desktop'
  ↓
selected ≠ active? → selected = 'desktop', emit INPUT_CONTROLLER_CHANGED
  ↓
DesktopController processes events (TouchController completely disabled)
```

**Rules:**
- **Selected controller:** Last used controller, persists when idle, determines UI state (e.g., show WASD hints)
- **Active controller:** Currently tracking inputs, null when idle, determines event routing
- **Automatic selection:** When input starts, `active = controller`, if `selected ≠ active` then `selected = active` and emit event
- **No mixing:** Only active controller processes events, other controller completely disabled
- **Selection persistence:** `selected` persists through idle periods (doesn't reset to null)

**Hybrid device handling (Surface, iPad + Mouse):**
- Touch-first user: Touch becomes active → mouse completely ignored until touch ends
- Mouse-first user: Desktop becomes active → touch completely ignored until mouse/keys released
- Palm rejection by design: touch inputs ignored when desktop active

**Controller Change Event:**
```typescript
type InputControllerChangedEvent = {
    controller: 'touch' | 'desktop'  // Newly selected controller
};
```

**Usage:**
- Listen to `INPUT_CONTROLLER_CHANGED` to show/hide UI hints
- Desktop: show WASD controls, mouse button instructions
- Touch: show touch gesture hints

**Initial selection:**
- Default: `'desktop'` (keyboard/mouse assumed on first load)

---

## Constants

```typescript
const TAP_THRESHOLD_MS = 500;   // Tap vs interact (equals LONG_PRESS_MS, no dead zone)
const LONG_PRESS_MS = 500;      // Interact trigger time
const MOVE_THRESHOLD_PX = 6;    // Drag trigger distance (Euclidean)
const PINCH_TIMING_MS = 300;    // Max time between pointers for pinch
```

---

## Global Gesture Rules

**Termination (all gestures):**
- Normal: Pointer up → `*_END` event fires
- Focus loss: `window.blur` → `*_END` fires immediately, all state cleared
- Pointer capture lost: `pointercancel` → `*_END` fires immediately for affected pointer

**Dead pointer:** When WASD/wheel cancels a tap/interact timer, that pointer becomes unusable until released and re-pressed (cannot start any gesture). Dead pointers still count as active inputs for controller deactivation.

---

## Event System

All events dispatch through the game's `EventTarget` using typed event pattern.

**Event subscription:**
- Use `subscriptions.on<EventType>(EVENT_NAME, handler)` pattern
- Controller changes: `INPUT_CONTROLLER_CHANGED`
- Gestures: `INPUT_TAP`, `INPUT_PAN`, `INPUT_ROTATE`, etc.

**Event handling:**
- Calculate deltas from `start` and `current` positions
- Use `INPUT_CONTROLLER_CHANGED` to update UI hints (show/hide WASD controls)

**Coordinate space:**
- All position coordinates (`pos: Point`) use **screen space** (viewport-relative)
- Values come from `clientX`/`clientY` (CSS pixels, not physical pixels)
- Origin: top-left of viewport (not canvas element)

**Event propagation:**
- **All pointer events:** `preventDefault()` called to prevent browser defaults
  - Prevents text selection during drag
  - Prevents context menu during gestures
  - Prevents touch scroll/zoom
  - Core principle: InputController owns all pointer interaction
- **Result:** No browser defaults interfere with gesture recognition

---

## Input Modes

### Controller Activation

| Input Type | Triggers | Active Controller |
|------------|----------|-------------------|
| Touch pointer down | `pointerType === 'touch'` | TouchController |
| Mouse/Pen pointer down | `pointerType === 'mouse'` or `'pen'` | DesktopController |
| Keyboard (WASD) | Key down | DesktopController |
| Wheel scroll | Wheel event | DesktopController |
| Middle mouse button | Button 1 pressed | **Ignored** (filtered out) |

**Activation behavior:**
- First input activates controller: `active = controller`
- If `selected ≠ active`: update `selected = active` and emit `INPUT_CONTROLLER_CHANGED`
- Active controller processes all subsequent events
- Inactive controller receives **zero events** (completely disabled)
- Controller deactivates when all inputs released: `active = null`
  - **TouchController:** All touch pointers released
  - **DesktopController:** All pointers + all WASD keys released
- `selected` persists (no event on deactivation)

**Keyboard activation:**
- **Only WASD keys** (W, A, S, D) activate DesktopController and generate INPUT_WASD_MOVE events
- **All other keyboard input** (Escape, Tab, Arrow keys, Space, etc.) is completely ignored by InputController
- Application layer must handle other keys independently (InputController never sees them)

**Mouse button filtering:**
- **Left button (0):** Pan, tap, interact
- **Right button (2):** Rotate
- **Middle button (1):** Completely ignored (filtered out, no activation, no events)
- **Other buttons (3+):** Completely ignored

---

## Supported Gestures

### By Sub-Controller

#### TouchController Gestures
- ✅ Tap
- ✅ Long press + drag (Interact)
- ✅ Pan (single-finger drag)
- ✅ Pinch (two-finger)

#### DesktopController Gestures
- ✅ Tap (left-click)
- ✅ Long press + drag (Interact, left button)
- ✅ Pan (left-drag)
- ✅ Rotate (right-drag)
- ✅ WASD movement (keyboard)
- ✅ Wheel zoom

---

## Gesture Specifications

**Controller Support:**

| Gesture | Touch | Desktop |
|---------|-------|---------|
| Tap | ✅ Any touch | ✅ Left button |
| Long press + Drag (Interact) | ✅ Any touch | ✅ Left button |
| Pan | ✅ Single finger | ✅ Left drag |
| Rotate | ❌ | ✅ Right drag |
| Pinch | ✅ Two fingers | ❌ |
| Zoom | ❌ | ✅ Wheel |
| WASD | ❌ | ✅ Keyboard |

### 1. Tap

**Trigger:** Pointer down + up < TAP_THRESHOLD_MS (500ms), no movement (≤ MOVE_THRESHOLD_PX)

**Event:** `INPUT_TAP`
```typescript
type InputTapEvent = { 
    pos: Point
};
```

**Flow:**
```
(Touch or Left-click) Down → Up (< TAP_THRESHOLD_MS, no pan/interact) → TAP
```

**Notes:**
- Threshold equals long press timing (TAP_THRESHOLD_MS) to eliminate dead zone
- Follows mobile OS conventions (iOS/Android use ~450-500ms)
- **Left button only:** Tap only fires from left mouse button (button 0)
- **Right-click behavior:** Right button quick press produces no INPUT_TAP event (preventDefault still called)

**Cancellation:**
- Pointer movement > MOVE_THRESHOLD_PX → becomes pan/rotate
- WASD/wheel (desktop) → timer cancelled, pointer becomes dead
- Other button pressed (desktop) → ignored (first button owns gesture)
- Focus loss / pointercancel → timer cancelled, no event

---

### 2. Long Press + Drag (Interact)

**Trigger:** Hold ≥ LONG_PRESS_MS (500ms) → drag → release

**Events:**
- `INPUT_INTERACT_START` → `{ pos: Point }`
- `INPUT_INTERACT_DRAG` → `{ start: Point, current: Point }` (continuous)
- `INPUT_INTERACT_END` → `{ pos: Point }`

**Flow:**
```
(Touch or Left-click) Down → Wait LONG_PRESS_MS (no movement) → INTERACT_START
                                                              ↓
                                                          Move → INTERACT_DRAG (loop)
                                                              ↓
                                                            Up → INTERACT_END
```

**Cancellation (before INTERACT_START):**
- Pointer movement > MOVE_THRESHOLD_PX → becomes pan
- 2nd pointer added (touch) → timer cancelled, no interact
- WASD/wheel (desktop) → timer cancelled, pointer becomes dead
- Other button pressed (desktop) → ignored (first button owns gesture)
- Focus loss / pointercancel → timer cancelled, no event

**Protection (after INTERACT_START):**
Camera-moving inputs blocked until `INTERACT_END` (stable camera for menu/selection):
- ❌ WASD, wheel zoom, pan, rotate all blocked

---

### 3. Pan (Drag)

**Trigger:** Single finger/mouse drag > MOVE_THRESHOLD_PX (6px)

**Events:**
- `INPUT_PAN_START` → `{ pos: Point }` (position where pointer first pressed)
- `INPUT_PAN` → `{ start: Point, current: Point }` (continuous; start = pointer down position)
- `INPUT_PAN_END` → `{ pos: Point }`

**Flow:**
```
(Touch or Left-click) Down → Move > MOVE_THRESHOLD_PX → PAN_START
                                                      ↓
                                                  Move → PAN (loop)
                                                      ↓
                                                    Up → PAN_END
```

**Cancellation:**
- Long press timer cancelled when pan starts
- 2nd touch pointer added → see disambiguation rules
- Right-click during pan (desktop) → ignored, pan continues
- WASD during pan (desktop) → blocked (conflicting camera movement)

**Concurrency:**
- ✅ Wheel zoom allowed (different camera properties)

---


### 4. Rotate (Drag)

**Trigger:** Right mouse button drag > MOVE_THRESHOLD_PX (6px, Euclidean distance from start position)

**Event:** `INPUT_ROTATE`
```typescript
type InputRotateEvent = {
    start: Point,      // Position where rotate started (screen space)
    current: Point     // Current pointer position (screen space)
};
```

**Movement threshold:** `sqrt((x-x0)² + (y-y0)²) > MOVE_THRESHOLD_PX` (same calculation as pan)

**Events:**
- `INPUT_ROTATE_START` → `{ pos: Point }` (position where pointer first pressed)
- `INPUT_ROTATE` → `{ start: Point, current: Point }` (continuous; start = pointer down position)
- `INPUT_ROTATE_END` → `{ pos: Point }`

**Flow:**
```
(Right) Down → Move > MOVE_THRESHOLD_PX → ROTATE_START
                                       ↓
                                   Move → ROTATE (loop)
                                       ↓
                                     Up → ROTATE_END
```

**Concurrency:**
- ✅ WASD + wheel allowed (FPS-style)
- ❌ Left-click → ignored (pointer gestures exclusive)

---

### 5. Pinch (Two-Finger)

**Trigger:** 2 pointers down within PINCH_TIMING_MS (300ms), if 1st finger moved < MOVE_THRESHOLD_PX (6px)

**Events:**
- `INPUT_PINCH_START` → `{ start: { pos1: Point, pos2: Point }, current: { pos1: Point, pos2: Point } }` (current === start at START time)
- `INPUT_PINCH` → `{ start: { pos1: Point, pos2: Point }, current: { pos1: Point, pos2: Point } }` (continuous)
- `INPUT_PINCH_END` → `{ start: { pos1: Point, pos2: Point }, current: { pos1: Point, pos2: Point } }`

**Flow:**
```
Finger1 Down → Finger2 Down (< PINCH_TIMING_MS) → PINCH_START
                                                        ↓
                                                    Move → PINCH (loop)
                                                        ↓
                                                 Either Up → PINCH_END
```

**Post-pinch:** When one finger lifts, `PINCH_END` fires and remaining finger becomes inactive (must lift and re-touch to start new gesture).

---

### 6. Zoom (Mouse Wheel)

**Trigger:** Mouse wheel scroll

**Event:** `INPUT_ZOOM`
```typescript
type InputZoomEvent = { 
    pos: Point,         // Mouse position at time of scroll
    delta: number       // Scroll amount (positive = away, negative = toward)
};
```

**Flow:**
```
Wheel → ZOOM (immediate)
```

**Concurrency:**
- ✅ Rotate allowed (zoom while rotating)
- ✅ WASD allowed (zoom while moving)
- ✅ Pan allowed (zoom while panning - different camera properties)

**Notes:**
- `delta > 0` = scroll down/away, `delta < 0` = scroll up/toward
- Position allows zoom-to-cursor behavior

**Blocked by:**
- Interact active (after INTERACT_START) → wheel events ignored

---

### 7. WASD Movement (Keyboard)

**Trigger:** WASD key state change

**Event:** `INPUT_WASD_MOVE`
```typescript
type InputWASDMoveEvent = {
    direction: { x: number, y: number },  // Normalized unit vector (-1, 0, 1)
    keys: { w: boolean, a: boolean, s: boolean, d: boolean }
};
```

**Flow:**
```
Key Down (W/A/S/D) → WASD_MOVE (fires if direction changed)
    ↓
Key Up → WASD_MOVE (fires if direction changed)
```

**Change detection:**
- Events only fire when the direction vector changes
- Holding keys down does NOT continuously emit events
- Reduces event spam while maintaining responsiveness
- Example: W down → event fired. W held → no events. W+D pressed → event fired (diagonal). W released → event fired (D only).

**Direction calculation:**
```
W pressed → direction.y = -1  (forward)
S pressed → direction.y = +1  (backward)
A pressed → direction.x = -1  (left)
D pressed → direction.x = +1  (right)

Multiple keys: vector sum normalized
Example: W+D = { x: 0.707, y: -0.707 } (forward-right diagonal)
```

**Concurrency:**
- ✅ Rotate allowed (FPS-style: keyboard move + mouse look)
- ✅ Wheel zoom allowed
- ❌ Pan blocked (conflicting camera movement)
- ❌ Tap blocked (target moves under cursor during movement)
- ❌ Interact blocked (requires stationary pointer and camera)

**Consumer usage:**
- Multiply `direction` by speed and delta time for camera velocity
- Direction is already normalized (no need to normalize again)

**Blocked by:**
- Interact active (after INTERACT_START) → WASD keys ignored

**Termination:** 
- All keys released → fires INPUT_WASD_MOVE with direction={0,0}
- Focus lost (`window.blur`) → fires INPUT_WASD_MOVE with direction={0,0}, all keys cleared

---

## Disambiguation Rules

### Pan vs Pinch (Two-Finger Timing)

**Solution:** When 2nd finger arrives, check if 1st moved > MOVE_THRESHOLD_PX. If yes → ignore 2nd, continue pan. If no → start pinch. No cancel event (avoids jarring discontinuity).

---

### Tap vs Long Press

**Solution:** Up < TAP_THRESHOLD_MS → TAP. Still down at LONG_PRESS_MS → INTERACT_START. Movement → PAN. Single threshold (TAP_THRESHOLD_MS === LONG_PRESS_MS) eliminates dead zone, matches mobile OS conventions (iOS/Android ~450-500ms).

---

### Pan vs Interact Drag

**Solution:** Immediate drag → PAN. Hold LONG_PRESS_MS → drag → INTERACT_DRAG. Mutually exclusive (only one active at a time).

---

### Pan vs Rotate (Dual Mouse Buttons)

**Solution:** First button down owns gesture until release. Other button ignored (prevents disorienting simultaneous pan+rotate). Exception: WASD + Rotate allowed (different input sources, FPS-style).

---

### WASD vs Pointer Gestures

**Solution:** Partial concurrency. Allowed: WASD + Rotate + Wheel (FPS-style). Blocked: WASD + Pan/Tap/Interact (conflicting movement or target instability). Tap/interact timers cancelled if WASD/wheel starts.

---

## Concurrent Event Table

### TouchController (All Mutually Exclusive)

| Scenario | Resolution |
|----------|------------|
| **Pan → 2nd finger (fast)** | `firstMoved < MOVE_THRESHOLD_PX` → start pinch (pan not committed) |
| **Pan → 2nd finger (slow)** | `firstMoved > MOVE_THRESHOLD_PX` → ignore 2nd, continue pan |
| **Interact timer → Move** | `totalMoved > MOVE_THRESHOLD_PX` → cancel timer, become pan |
| **Pinch → 3rd pointer** | Ignore 3rd, continue pinch |
| **Interact → 2nd pointer** | Cancel timer, no gesture |
| **Pinch → One finger up** | PINCH_END, remaining finger inactive |

### DesktopController (Pointer Exclusive, Free Inputs Concurrent)

| Scenario | Resolution |
|----------|------------|
| **Pan → Right button** | Right ignored, pan continues |
| **Rotate → Left button** | Left ignored, rotate continues |
| **Tap/Interact timer → Other button** | Other button ignored (first owns) |
| **Pan → Wheel** | Both fire (zoom while panning) |
| **Rotate → Wheel** | Both fire (zoom while rotating) |
| **Rotate → WASD** | Both fire (FPS-style) |
| **Pan ↔ WASD** | Blocked (conflicting movement) |
| **WASD → Tap/Interact** | Blocked (target/camera unstable) |
| **Timer → WASD/Wheel** | Timer cancelled, pointer becomes dead |
| **Interact active → WASD/Wheel** | Blocked (camera must be stable) |
| **Pan active → Right held, left up** | Pan ends, right ignored until released |

### Cross-Controller (Orchestrator Handles)

| Scenario | Resolution |
|----------|------------|
| **Touch active → Mouse/WASD** | Not routed to DesktopController (completely ignored) |
| **Desktop active → Touch** | Not routed to TouchController (completely ignored) |

**Note:** Orchestrator prevents mixing by routing events only to active controller. Inactive controller receives zero events.

**Multiple pointers of same type:**
- Each pointer tracked by unique `pointerId`
- First pointer down of active controller owns the gesture
- Subsequent pointers of same type ignored until first gesture ends
- Exception: Touch pinch explicitly supports 2 pointers (3rd ignored)

---

## Testing

### Unit Testing Strategy

**Key scenarios to test:**

#### TouchController Tests:
1. **Tap** (< TAP_THRESHOLD_MS, no movement)
2. **Long press** (≥ LONG_PRESS_MS hold)
3. **Pan** (single-finger drag > MOVE_THRESHOLD_PX)
4. **Pinch (fast two-finger)** (< PINCH_TIMING_MS between pointers)
5. **Pinch rejection (slow two-finger)** (pan started, 2nd finger ignored)
6. **Post-pinch inactive** (one finger up → remaining finger inactive until lift)
7. **Timer cancellation** (movement before LONG_PRESS_MS)
8. **Dead zone eliminated** (300ms tap fires TAP, not swallowed)
9. **Focus loss cleanup** (pan active → blur → PAN_END fires, active=null)
10. **Pointer capture loss** (pan active → pointercancel → PAN_END fires)

#### DesktopController Tests:
11. **Tap** (left-click < TAP_THRESHOLD_MS)
12. **Long press** (left-click ≥ LONG_PRESS_MS hold)
13. **Pan** (left drag > MOVE_THRESHOLD_PX)
14. **Rotate** (right drag > MOVE_THRESHOLD_PX)
15. **Wheel zoom** (scroll events, blocked during interact)
16. **WASD movement** (key state changes, direction vectors, change detection)
17. **Dual-button** (left+right pressed → first wins)
18. **Button switching** (pan → right-click → right ignored)
19. **FPS-style** (WASD + rotate + wheel simultaneously)
20. **WASD blocks pan** (WASD active → left-drag ignored)
21. **Pan blocks WASD** (left-drag active → WASD ignored)
22. **Interact blocks WASD** (INTERACT_START fired → WASD ignored)
23. **Interact blocks wheel** (INTERACT_START fired → wheel ignored)
24. **Wheel cancels tap timer** (tap timer active → wheel → timer cancelled)
25. **Focus loss cleanup** (WASD active → blur → INPUT_WASD_MOVE direction={0,0})
26. **Pointer capture loss** (rotate active → pointercancel → ROTATE_END fires)

#### Controller Switching Tests:
27. **Touch-first** (touch down → active='touch' → mouse completely ignored)
28. **Mouse-first** (mouse down → active='desktop' → touch completely ignored)
29. **Clean transition** (touch up → active=null → mouse down → active='desktop')
30. **Keyboard activates desktop** (WASD → active='desktop' → touch ignored)
31. **Selection persistence** (touch up → active=null, selected='touch' → verify UI state)
32. **Controller change events** (verify INPUT_CONTROLLER_CHANGED fires only on selection change, not on activation)
33. **No duplicate events** (touch → touch again without desktop in between → no event)
34. **Non-WASD keys ignored** (Escape, Tab, Arrow keys → no controller activation)
35. **Multiple pointers** (mouse1 pan active → mouse2 click → ignored)
