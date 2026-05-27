// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LONG_PRESS_MS, MOVE_THRESHOLD_PX, PINCH_TIMING_MS, TAP_THRESHOLD_MS } from '../../constants';
import { InputController } from './_input-controller';
import type { InputHandler, Point } from './_input-handler';
import { InputManager } from '../../avatar/input-manager';
import type { Camera } from '../camera/camera';
import { WorldCursor } from '../../avatar/world-cursor';
import type { RenderContext } from '../render-context';
import { MIN_CAMERA_DISTANCE, MAX_CAMERA_DISTANCE } from '../../constants';

// ─── Helpers ────────────────────────────────────────────────────────────────

function makeHandler(): InputHandler {
    return {
        onControllerChanged: vi.fn(),
        onTap: vi.fn(),
        onInteractStart: vi.fn(),
        onInteractDrag: vi.fn(),
        onInteractEnd: vi.fn(),
        onDragPanStart: vi.fn(),
        onDragPan: vi.fn(),
        onDragPanEnd: vi.fn(),
        onDragRotateStart: vi.fn(),
        onDragRotate: vi.fn(),
        onDragRotateEnd: vi.fn(),
        onPinchStart: vi.fn(),
        onPinch: vi.fn(),
        onPinchEnd: vi.fn(),
        onZoomTo: vi.fn(),
        onMove: vi.fn(),
        onRotate: vi.fn(),
        onZoom: vi.fn()
    };
}

function makeSetup() {
    const el = document.createElement('div');
    const handler = makeHandler();
    const ctrl = new InputController(el, handler);
    return { el, handler, ctrl };
}

// Pointer helpers
// Touch finger IDs are offset by 9 so they never collide with mouse pointer id=1.
// Finger 1 → pointerId 10, finger 2 → pointerId 11, etc. (matches real-browser convention)
const TOUCH_ID_OFFSET = 9;

function touchDown(el: HTMLElement, finger: number, x: number, y: number) {
    el.dispatchEvent(
        new PointerEvent('pointerdown', {
            pointerType: 'touch',
            pointerId: finger + TOUCH_ID_OFFSET,
            clientX: x,
            clientY: y,
            button: 0,
            buttons: 1,
            bubbles: true
        })
    );
}

function touchMove(el: HTMLElement, finger: number, x: number, y: number) {
    el.dispatchEvent(
        new PointerEvent('pointermove', {
            pointerType: 'touch',
            pointerId: finger + TOUCH_ID_OFFSET,
            clientX: x,
            clientY: y,
            button: 0,
            buttons: 1,
            bubbles: true
        })
    );
}

function touchUp(el: HTMLElement, finger: number, x: number, y: number) {
    el.dispatchEvent(
        new PointerEvent('pointerup', {
            pointerType: 'touch',
            pointerId: finger + TOUCH_ID_OFFSET,
            clientX: x,
            clientY: y,
            button: 0,
            buttons: 0,
            bubbles: true
        })
    );
}

function touchCancel(el: HTMLElement, finger: number, x: number, y: number) {
    el.dispatchEvent(
        new PointerEvent('pointercancel', {
            pointerType: 'touch',
            pointerId: finger + TOUCH_ID_OFFSET,
            clientX: x,
            clientY: y,
            button: 0,
            buttons: 0,
            bubbles: true
        })
    );
}

function mouseDown(el: HTMLElement, button: number, x: number, y: number, pointerId = 1) {
    const buttons = button === 0 ? 1 : button === 2 ? 2 : 4;
    el.dispatchEvent(
        new PointerEvent('pointerdown', {
            pointerType: 'mouse',
            pointerId,
            clientX: x,
            clientY: y,
            button,
            buttons,
            bubbles: true
        })
    );
}

function mouseMove(el: HTMLElement, button: number, x: number, y: number, pointerId = 1) {
    const buttons = button === 0 ? 1 : button === 2 ? 2 : 4;
    el.dispatchEvent(
        new PointerEvent('pointermove', {
            pointerType: 'mouse',
            pointerId,
            clientX: x,
            clientY: y,
            button,
            buttons,
            bubbles: true
        })
    );
}

function mouseUp(el: HTMLElement, button: number, x: number, y: number, pointerId = 1) {
    el.dispatchEvent(
        new PointerEvent('pointerup', {
            pointerType: 'mouse',
            pointerId,
            clientX: x,
            clientY: y,
            button,
            buttons: 0,
            bubbles: true
        })
    );
}

function mouseCancel(el: HTMLElement, button: number, x: number, y: number, pointerId = 1) {
    el.dispatchEvent(
        new PointerEvent('pointercancel', {
            pointerType: 'mouse',
            pointerId,
            clientX: x,
            clientY: y,
            button,
            buttons: 0,
            bubbles: true
        })
    );
}

function keyDown(key: string) {
    window.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }));
}

function keyUp(key: string) {
    window.dispatchEvent(new KeyboardEvent('keyup', { key, bubbles: true }));
}

function wheel(el: HTMLElement, x: number, y: number, deltaY: number) {
    el.dispatchEvent(
        new WheelEvent('wheel', {
            clientX: x,
            clientY: y,
            deltaY,
            bubbles: true
        })
    );
}

// Drag past MOVE_THRESHOLD_PX
const DRAG = MOVE_THRESHOLD_PX + 5;

// ─── TouchController tests ────────────────────────────────────────────────────

describe('TouchController', () => {
    beforeEach(() => vi.useFakeTimers());
    afterEach(() => vi.useRealTimers());

    it('test 1: tap fires INPUT_TAP (< TAP_THRESHOLD_MS, no movement)', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        vi.advanceTimersByTime(TAP_THRESHOLD_MS - 1);
        touchUp(el, 1, 100, 100);
        expect(handler.onTap).toHaveBeenCalledWith({ x: 100, y: 100 });
        expect(handler.onDragPanStart).not.toHaveBeenCalled();
    });

    it('test 2: long press fires INTERACT_START (>= LONG_PRESS_MS hold)', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        vi.advanceTimersByTime(LONG_PRESS_MS);
        expect(handler.onInteractStart).toHaveBeenCalledWith({ x: 100, y: 100 });
    });

    it('test 3: drag pan fires start/drag/end (single-finger drag > MOVE_THRESHOLD_PX)', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        touchMove(el, 1, 100 + DRAG, 100);
        expect(handler.onDragPanStart).toHaveBeenCalledWith({ x: 100, y: 100 });
        expect(handler.onDragPan).toHaveBeenCalled();
        touchUp(el, 1, 100 + DRAG, 100);
        expect(handler.onDragPanEnd).toHaveBeenCalled();
        expect(handler.onTap).not.toHaveBeenCalled();
    });

    it('test 4: pinch fires start (fast two-finger, < PINCH_TIMING_MS)', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        vi.advanceTimersByTime(PINCH_TIMING_MS - 1);
        touchDown(el, 2, 200, 200);
        expect(handler.onPinchStart).toHaveBeenCalled();
    });

    it('test 5: pinch rejected when 1st finger moved > MOVE_THRESHOLD_PX (pan started, 2nd ignored)', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        touchMove(el, 1, 100 + DRAG, 100); // commits pan
        touchDown(el, 2, 200, 200);
        expect(handler.onPinchStart).not.toHaveBeenCalled();
        expect(handler.onDragPanStart).toHaveBeenCalled();
    });

    it('test 6: post-pinch remaining finger is inactive (must lift to restart)', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        vi.advanceTimersByTime(PINCH_TIMING_MS - 1);
        touchDown(el, 2, 200, 200);
        touchUp(el, 2, 200, 200); // ends pinch
        expect(handler.onPinchEnd).toHaveBeenCalled();
        // Remaining finger 1 is inactive - moving it should not produce drag
        vi.clearAllMocks();
        touchMove(el, 1, 100 + DRAG, 100);
        expect(handler.onDragPanStart).not.toHaveBeenCalled();
        // After lifting and re-touching, can pan again
        touchUp(el, 1, 100, 100);
        touchDown(el, 3, 50, 50);
        touchMove(el, 3, 50 + DRAG, 50);
        expect(handler.onDragPanStart).toHaveBeenCalled();
    });

    it('test 7: movement before LONG_PRESS_MS cancels timer (becomes drag, no interact)', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        vi.advanceTimersByTime(LONG_PRESS_MS - 100);
        touchMove(el, 1, 100 + DRAG, 100);
        vi.advanceTimersByTime(200); // past LONG_PRESS_MS, timer should be cancelled
        expect(handler.onInteractStart).not.toHaveBeenCalled();
        expect(handler.onDragPanStart).toHaveBeenCalled();
    });

    it('test 8: tap at 300ms fires TAP (dead zone eliminated)', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        vi.advanceTimersByTime(300);
        touchUp(el, 1, 100, 100);
        expect(handler.onTap).toHaveBeenCalled();
        expect(handler.onInteractStart).not.toHaveBeenCalled();
    });

    it('test 9: focus loss during drag pan fires DRAG_PAN_END and clears active', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        touchMove(el, 1, 100 + DRAG, 100);
        expect(handler.onDragPanStart).toHaveBeenCalled();
        window.dispatchEvent(new Event('blur'));
        expect(handler.onDragPanEnd).toHaveBeenCalled();
    });

    it('test 10: pointercancel during drag pan fires DRAG_PAN_END', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        touchMove(el, 1, 100 + DRAG, 100);
        touchCancel(el, 1, 100 + DRAG, 100);
        expect(handler.onDragPanEnd).toHaveBeenCalled();
    });
});

// ─── DesktopController tests ──────────────────────────────────────────────────

describe('DesktopController', () => {
    beforeEach(() => vi.useFakeTimers());
    afterEach(() => {
        vi.useRealTimers();
        // Release any held keys
        ['w', 'a', 's', 'd', 'q', 'e', 'r', 'f', 'shift'].forEach((k) => keyUp(k));
    });

    it('test 11: tap fires INPUT_TAP (left-click < TAP_THRESHOLD_MS)', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100);
        vi.advanceTimersByTime(TAP_THRESHOLD_MS - 1);
        mouseUp(el, 0, 100, 100);
        expect(handler.onTap).toHaveBeenCalledWith({ x: 100, y: 100 });
    });

    it('test 12: long press fires INTERACT_START (left-click >= LONG_PRESS_MS)', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100);
        vi.advanceTimersByTime(LONG_PRESS_MS);
        expect(handler.onInteractStart).toHaveBeenCalledWith({ x: 100, y: 100 });
    });

    it('test 13: drag pan fires start/drag/end (left drag > MOVE_THRESHOLD_PX)', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100);
        mouseMove(el, 0, 100 + DRAG, 100);
        expect(handler.onDragPanStart).toHaveBeenCalledWith({ x: 100, y: 100 });
        expect(handler.onDragPan).toHaveBeenCalled();
        mouseUp(el, 0, 100 + DRAG, 100);
        expect(handler.onDragPanEnd).toHaveBeenCalled();
    });

    it('test 14: drag rotate fires start/drag/end (right drag > MOVE_THRESHOLD_PX)', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 2, 100, 100);
        mouseMove(el, 2, 100 + DRAG, 100);
        expect(handler.onDragRotateStart).toHaveBeenCalledWith({ x: 100, y: 100 });
        expect(handler.onDragRotate).toHaveBeenCalled();
        mouseUp(el, 2, 100 + DRAG, 100);
        expect(handler.onDragRotateEnd).toHaveBeenCalled();
    });

    it('test 15: wheel zoom fires onZoomTo (blocked during interact)', () => {
        const { el, handler } = makeSetup();
        wheel(el, 200, 200, 100);
        expect(handler.onZoomTo).toHaveBeenCalledWith({ x: 200, y: 200 }, 100);

        // Blocked during interact
        vi.clearAllMocks();
        mouseDown(el, 0, 100, 100);
        vi.advanceTimersByTime(LONG_PRESS_MS);
        wheel(el, 200, 200, 100);
        expect(handler.onZoomTo).not.toHaveBeenCalled();
    });

    it('test 16: WASD movement fires onMove with correct direction vectors', () => {
        const { handler } = makeSetup();
        keyDown('w');
        expect(handler.onMove).toHaveBeenCalledWith({ x: 0, y: -1 }, false);
        vi.clearAllMocks();
        keyDown('d');
        const diag = 1 / Math.sqrt(2);
        expect(handler.onMove).toHaveBeenCalledWith(
            expect.objectContaining({
                x: expect.closeTo(diag, 5),
                y: expect.closeTo(-diag, 5)
            }),
            false
        );
        keyUp('w');
        keyUp('d');
    });

    it('test 17: Q/E rotation fires onRotate with correct directions', () => {
        const { handler } = makeSetup();
        keyDown('q');
        expect(handler.onRotate).toHaveBeenCalledWith(-1);
        keyDown('e');
        expect(handler.onRotate).toHaveBeenCalledWith(0); // cancel out
        keyUp('q');
        expect(handler.onRotate).toHaveBeenCalledWith(1);
        keyUp('e');
        expect(handler.onRotate).toHaveBeenCalledWith(0);
    });

    it('test 18: R/F zoom fires onZoom with correct directions', () => {
        const { handler } = makeSetup();
        keyDown('r');
        expect(handler.onZoom).toHaveBeenCalledWith(-1);
        keyDown('f');
        expect(handler.onZoom).toHaveBeenCalledWith(0); // cancel out
        keyUp('r');
        expect(handler.onZoom).toHaveBeenCalledWith(1);
        keyUp('f');
        expect(handler.onZoom).toHaveBeenCalledWith(0);
    });

    it('test 19: dual-button — first button wins, second ignored', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100); // left owns
        mouseDown(el, 2, 100, 100); // right ignored
        mouseMove(el, 2, 100 + DRAG, 100);
        expect(handler.onDragRotateStart).not.toHaveBeenCalled();
        expect(handler.onDragPanStart).not.toHaveBeenCalled();
    });

    it('test 20: button switching — drag pan active, right-click ignored', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100);
        mouseMove(el, 0, 100 + DRAG, 100); // drag pan
        vi.clearAllMocks();
        mouseDown(el, 2, 100 + DRAG, 100); // right ignored
        mouseMove(el, 0, 100 + DRAG + 5, 100);
        expect(handler.onDragRotateStart).not.toHaveBeenCalled();
        expect(handler.onDragPan).toHaveBeenCalled(); // drag pan continues
    });

    it('test 21: FPS-style — WASD + drag rotate + wheel simultaneously', () => {
        const { el, handler } = makeSetup();
        keyDown('w');
        mouseDown(el, 2, 100, 100);
        mouseMove(el, 2, 100 + DRAG, 100);
        wheel(el, 100, 100, -50);
        expect(handler.onMove).toHaveBeenCalled();
        expect(handler.onDragRotateStart).toHaveBeenCalled();
        expect(handler.onZoomTo).toHaveBeenCalled();
        keyUp('w');
    });

    it('test 22: WASD + Q/E concurrent — both fire', () => {
        const { handler } = makeSetup();
        keyDown('w');
        keyDown('q');
        expect(handler.onMove).toHaveBeenCalled();
        expect(handler.onRotate).toHaveBeenCalledWith(-1);
        keyUp('w');
        keyUp('q');
    });

    it('test 23: WASD + R/F concurrent — both fire', () => {
        const { handler } = makeSetup();
        keyDown('w');
        keyDown('r');
        expect(handler.onMove).toHaveBeenCalled();
        expect(handler.onZoom).toHaveBeenCalledWith(-1);
        keyUp('w');
        keyUp('r');
    });

    it('test 24: WASD blocks drag pan — DesktopController calls handler, InputManager blocks', () => {
        const { el, handler } = makeSetup();
        keyDown('w');
        mouseDown(el, 0, 100, 100);
        mouseMove(el, 0, 100 + DRAG, 100);
        // DesktopController calls onDragPanStart — blocking is InputManager's job
        expect(handler.onDragPanStart).toHaveBeenCalled();
        keyUp('w');
    });

    it('test 25: drag pan blocks WASD — left-drag active → WASD ignored', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100);
        mouseMove(el, 0, 100 + DRAG, 100); // commit pan
        vi.clearAllMocks();
        keyDown('w');
        expect(handler.onMove).not.toHaveBeenCalled();
        keyUp('w');
    });

    it('test 26: Q/E blocks drag pan — DesktopController calls handler, InputManager blocks', () => {
        const { el, handler } = makeSetup();
        keyDown('q');
        mouseDown(el, 0, 100, 100);
        mouseMove(el, 0, 100 + DRAG, 100);
        // DesktopController calls onDragPanStart — blocking is InputManager's job
        expect(handler.onDragPanStart).toHaveBeenCalled();
        keyUp('q');
    });

    it('test 27: drag pan blocks Q/E — left-drag active → Q/E ignored', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100);
        mouseMove(el, 0, 100 + DRAG, 100);
        vi.clearAllMocks();
        keyDown('q');
        expect(handler.onRotate).not.toHaveBeenCalled();
        keyUp('q');
    });

    it('test 28: Q/E blocked by drag rotate — drag rotate active → Q/E ignored', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 2, 100, 100);
        mouseMove(el, 2, 100 + DRAG, 100); // drag rotate active
        vi.clearAllMocks();
        keyDown('q');
        expect(handler.onRotate).not.toHaveBeenCalled();
        keyUp('q');
    });

    it('test 29: drag rotate blocked by Q/E — DesktopController calls handler, InputManager blocks', () => {
        const { el, handler } = makeSetup();
        keyDown('q');
        mouseDown(el, 2, 100, 100);
        mouseMove(el, 2, 100 + DRAG, 100);
        // DesktopController calls onDragRotateStart — blocking is InputManager's job
        expect(handler.onDragRotateStart).toHaveBeenCalled();
        keyUp('q');
    });

    it('test 30: R/F blocks wheel — DesktopController calls handler, InputManager blocks', () => {
        const { el, handler } = makeSetup();
        keyDown('r');
        wheel(el, 200, 200, 100);
        // DesktopController calls onZoomTo — blocking is InputManager's job
        expect(handler.onZoomTo).toHaveBeenCalled();
        keyUp('r');
    });

    it('test 31: interact blocks WASD — after INTERACT_START → WASD ignored', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100);
        vi.advanceTimersByTime(LONG_PRESS_MS);
        vi.clearAllMocks();
        keyDown('w');
        expect(handler.onMove).not.toHaveBeenCalled();
        keyUp('w');
    });

    it('test 32: interact blocks Q/E', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100);
        vi.advanceTimersByTime(LONG_PRESS_MS);
        vi.clearAllMocks();
        keyDown('q');
        expect(handler.onRotate).not.toHaveBeenCalled();
        keyUp('q');
    });

    it('test 33: interact blocks R/F', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100);
        vi.advanceTimersByTime(LONG_PRESS_MS);
        vi.clearAllMocks();
        keyDown('r');
        expect(handler.onZoom).not.toHaveBeenCalled();
        keyUp('r');
    });

    it('test 34: interact blocks wheel', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100);
        vi.advanceTimersByTime(LONG_PRESS_MS);
        vi.clearAllMocks();
        wheel(el, 200, 200, 100);
        expect(handler.onZoomTo).not.toHaveBeenCalled();
    });

    it('test 35: wheel cancels tap timer — pointer becomes dead', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100); // starts long-press timer
        wheel(el, 200, 200, 50); // cancels timer
        vi.advanceTimersByTime(LONG_PRESS_MS);
        expect(handler.onInteractStart).not.toHaveBeenCalled();
        mouseUp(el, 0, 100, 100); // dead pointer, no tap
        expect(handler.onTap).not.toHaveBeenCalled();
    });

    it('test 36: Q/E cancels tap timer — pointer becomes dead', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100);
        keyDown('q');
        vi.advanceTimersByTime(LONG_PRESS_MS);
        expect(handler.onInteractStart).not.toHaveBeenCalled();
        mouseUp(el, 0, 100, 100);
        expect(handler.onTap).not.toHaveBeenCalled();
        keyUp('q');
    });

    it('test 37: R/F cancels tap timer — pointer becomes dead', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100);
        keyDown('r');
        vi.advanceTimersByTime(LONG_PRESS_MS);
        expect(handler.onInteractStart).not.toHaveBeenCalled();
        mouseUp(el, 0, 100, 100);
        expect(handler.onTap).not.toHaveBeenCalled();
        keyUp('r');
    });

    it('test 38: focus loss clears WASD — fires onMove with direction {0,0}', () => {
        const { handler } = makeSetup();
        keyDown('w');
        vi.clearAllMocks();
        window.dispatchEvent(new Event('blur'));
        expect(handler.onMove).toHaveBeenCalledWith({ x: 0, y: 0 }, false);
    });

    it('test 39: focus loss clears Q/E — fires onRotate with direction 0', () => {
        const { handler } = makeSetup();
        keyDown('q');
        vi.clearAllMocks();
        window.dispatchEvent(new Event('blur'));
        expect(handler.onRotate).toHaveBeenCalledWith(0);
    });

    it('test 40: focus loss clears R/F — fires onZoom with direction 0', () => {
        const { handler } = makeSetup();
        keyDown('r');
        vi.clearAllMocks();
        window.dispatchEvent(new Event('blur'));
        expect(handler.onZoom).toHaveBeenCalledWith(0);
    });

    it('test 41: pointercancel fires DRAG_ROTATE_END', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 2, 100, 100);
        mouseMove(el, 2, 100 + DRAG, 100);
        mouseCancel(el, 2, 100 + DRAG, 100);
        expect(handler.onDragRotateEnd).toHaveBeenCalled();
    });

    it('WASD change detection — holding key does not re-emit', () => {
        const { handler } = makeSetup();
        keyDown('w');
        const firstCallCount = (handler.onMove as ReturnType<typeof vi.fn>).mock.calls.length;
        keyDown('w'); // repeat keydown (same key)
        expect(handler.onMove).toHaveBeenCalledTimes(firstCallCount); // no new call
        keyUp('w');
    });
});

// ─── Controller switching tests (Orchestrator) ───────────────────────────────

describe('Controller switching (Orchestrator)', () => {
    beforeEach(() => vi.useFakeTimers());
    afterEach(() => {
        vi.useRealTimers();
        ['w', 'a', 's', 'd', 'q', 'e', 'r', 'f'].forEach((k) => keyUp(k));
    });

    it('test 42: touch-first — touch active, mouse completely ignored', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100); // activates touch
        vi.clearAllMocks();
        mouseDown(el, 0, 200, 200); // should be ignored
        vi.advanceTimersByTime(TAP_THRESHOLD_MS - 1);
        mouseUp(el, 0, 200, 200);
        expect(handler.onTap).not.toHaveBeenCalled();
        expect(handler.onControllerChanged).not.toHaveBeenCalled();
    });

    it('test 43: mouse-first — desktop active, touch completely ignored', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100); // activates desktop
        vi.clearAllMocks();
        touchDown(el, 1, 200, 200); // should be ignored
        vi.advanceTimersByTime(TAP_THRESHOLD_MS - 1);
        touchUp(el, 1, 200, 200);
        expect(handler.onTap).not.toHaveBeenCalled();
        expect(handler.onControllerChanged).not.toHaveBeenCalled();
    });

    it('test 44: clean transition — touch up then mouse activates desktop', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        touchUp(el, 1, 100, 100); // active=null
        vi.clearAllMocks();
        mouseDown(el, 0, 200, 200);
        vi.advanceTimersByTime(TAP_THRESHOLD_MS - 1);
        mouseUp(el, 0, 200, 200);
        expect(handler.onTap).toHaveBeenCalled();
        expect(handler.onControllerChanged).toHaveBeenCalledWith('desktop');
    });

    it('test 45: WASD activates desktop — subsequent touch ignored', () => {
        const { el, handler } = makeSetup();
        keyDown('w');
        vi.clearAllMocks();
        touchDown(el, 1, 100, 100);
        vi.advanceTimersByTime(TAP_THRESHOLD_MS - 1);
        touchUp(el, 1, 100, 100);
        expect(handler.onTap).not.toHaveBeenCalled();
        keyUp('w');
    });

    it('test 46: Q/E activates desktop', () => {
        const { el, handler } = makeSetup();
        keyDown('q');
        vi.clearAllMocks();
        touchDown(el, 1, 100, 100);
        vi.advanceTimersByTime(TAP_THRESHOLD_MS - 1);
        touchUp(el, 1, 100, 100);
        expect(handler.onTap).not.toHaveBeenCalled();
        keyUp('q');
    });

    it('test 47: R/F activates desktop', () => {
        const { el, handler } = makeSetup();
        keyDown('r');
        vi.clearAllMocks();
        touchDown(el, 1, 100, 100);
        vi.advanceTimersByTime(TAP_THRESHOLD_MS - 1);
        touchUp(el, 1, 100, 100);
        expect(handler.onTap).not.toHaveBeenCalled();
        keyUp('r');
    });

    it('test 48: selection persistence — touch selected stays after touch up', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        touchUp(el, 1, 100, 100);
        // selected='touch', active=null — now touch again, no new event
        vi.clearAllMocks();
        touchDown(el, 2, 50, 50);
        expect(handler.onControllerChanged).not.toHaveBeenCalled();
        touchUp(el, 2, 50, 50);
    });

    it('test 49: INPUT_CONTROLLER_CHANGED fires only on selection change, not on re-activation', () => {
        const { el, handler } = makeSetup();
        // First touch selects touch
        touchDown(el, 1, 100, 100);
        expect(handler.onControllerChanged).toHaveBeenCalledTimes(1);
        touchUp(el, 1, 100, 100);
        vi.clearAllMocks();
        // Second touch — selected already 'touch', no event
        touchDown(el, 2, 50, 50);
        expect(handler.onControllerChanged).not.toHaveBeenCalled();
        touchUp(el, 2, 50, 50);
    });

    it('test 50: no duplicate controller-changed events — touch again without desktop in between', () => {
        const { el, handler } = makeSetup();
        touchDown(el, 1, 100, 100);
        touchUp(el, 1, 100, 100);
        const calls = (handler.onControllerChanged as ReturnType<typeof vi.fn>).mock.calls.length;
        touchDown(el, 2, 50, 50);
        touchUp(el, 2, 50, 50);
        expect(handler.onControllerChanged).toHaveBeenCalledTimes(calls); // no new events
    });

    it('test 51: non-WASD/Q/E/R/F keys ignored — no controller activation', () => {
        const { el, handler } = makeSetup();
        vi.clearAllMocks();
        keyDown('Escape');
        keyDown('Tab');
        keyDown('ArrowUp');
        keyDown(' ');
        expect(handler.onControllerChanged).not.toHaveBeenCalled();
        expect(handler.onMove).not.toHaveBeenCalled();
    });

    it('test 52: multiple mouse pointers — second pointer click ignored during drag pan', () => {
        const { el, handler } = makeSetup();
        mouseDown(el, 0, 100, 100, 1); // pointer 1 starts drag pan
        mouseMove(el, 0, 100 + DRAG, 100, 1);
        vi.clearAllMocks();
        mouseDown(el, 0, 200, 200, 2); // second pointer ignored
        vi.advanceTimersByTime(TAP_THRESHOLD_MS - 1);
        mouseUp(el, 0, 200, 200, 2);
        expect(handler.onTap).not.toHaveBeenCalled();
    });
});

// ─── InputManager tests ───────────────────────────────────────────────────────

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

// ─── InputManager - WASD Conflict Resolution ──────────────────────────────────

describe('InputManager - WASD Conflict Resolution', () => {
    let mockCamera: Camera;
    let mockWorldCursor: WorldCursor;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockCamera = {
            screenToWorldPlanePoint: (x: number, y: number) => {
                return { x: x * 10, y: y * 10, z: 0 };
            }
        } as unknown as Camera;
        mockWorldCursor = {} as WorldCursor;
        mockEvents = new EventTarget();
    });

    it('WASD held → drag-pan start blocked (no CURSOR_MOVE_TO emitted)', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let moveToEmitted = false;
        mockEvents.addEventListener('cursor:move_to', () => {
            moveToEmitted = true;
        });

        // Hold WASD
        manager.onMove({ x: 1, y: 0 }, false);

        // Try to start drag-pan
        manager.onDragPanStart({ x: 100, y: 100 });
        manager.onDragPan({ x: 100, y: 100 }, { x: 150, y: 150 });

        expect(moveToEmitted).toBe(false);
    });

    it('WASD held → existing drag-pan stops (event count does not increase)', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let moveToCount = 0;
        mockEvents.addEventListener('cursor:move_to', () => {
            moveToCount++;
        });

        // Start drag-pan without WASD
        manager.onDragPanStart({ x: 100, y: 100 });
        manager.onDragPan({ x: 100, y: 100 }, { x: 150, y: 150 });
        const countBeforeWASD = moveToCount;

        // Hold WASD
        manager.onMove({ x: 1, y: 0 }, false);

        // Continue drag-pan with WASD active
        manager.onDragPan({ x: 100, y: 100 }, { x: 200, y: 200 });

        expect(moveToCount).toBe(countBeforeWASD); // No new events
    });

    it('WASD released → drag-pan unblocked', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let moveToEmitted = false;
        mockEvents.addEventListener('cursor:move_to', () => {
            moveToEmitted = true;
        });

        // Hold WASD
        manager.onMove({ x: 1, y: 0 }, false);

        // Try to start drag-pan (blocked)
        manager.onDragPanStart({ x: 100, y: 100 });

        // Release WASD
        manager.onMove({ x: 0, y: 0 }, false);

        // End first drag-pan
        manager.onDragPanEnd({ x: 100, y: 100 });

        // Start new drag-pan (should work now)
        manager.onDragPanStart({ x: 100, y: 100 });
        manager.onDragPan({ x: 100, y: 100 }, { x: 150, y: 150 });

        expect(moveToEmitted).toBe(true);
    });

    it('WASD held → drag-rotate allowed (CURSOR_ROTATE_DELTA emitted)', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let rotateEmitted = false;
        mockEvents.addEventListener('cursor:rotate_delta', () => {
            rotateEmitted = true;
        });

        // Hold WASD
        manager.onMove({ x: 1, y: 0 }, false);

        // Drag rotate should still work
        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onDragRotate({ x: 100, y: 100 }, { x: 150, y: 150 });

        expect(rotateEmitted).toBe(true);
    });

    it('WASD held → wheel zoom allowed (CURSOR_ZOOM_DELTA emitted)', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let zoomEmitted = false;
        mockEvents.addEventListener('cursor:zoom_delta', () => {
            zoomEmitted = true;
        });

        // Hold WASD
        manager.onMove({ x: 1, y: 0 }, false);

        // Wheel zoom should still work
        manager.onZoomTo({ x: 100, y: 100 }, 50);

        expect(zoomEmitted).toBe(true);
    });
});

// ─── InputManager - Q/E Conflict Resolution ──────────────────────────────────

describe('InputManager - Q/E Conflict Resolution', () => {
    let mockCamera: Camera;
    let mockWorldCursor: WorldCursor;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockCamera = {
            screenToWorldPlanePoint: (x: number, y: number) => {
                return { x: x * 10, y: y * 10, z: 0 };
            }
        } as unknown as Camera;
        mockWorldCursor = {} as WorldCursor;
        mockEvents = new EventTarget();
    });

    it('Q/E held → drag-rotate blocked (no CURSOR_ROTATE_DELTA emitted)', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let rotateDeltaEmitted = false;
        mockEvents.addEventListener('cursor:rotate_delta', () => {
            rotateDeltaEmitted = true;
        });

        // Hold Q/E
        manager.onRotate(-1);

        // Try to start drag-rotate
        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onDragRotate({ x: 100, y: 100 }, { x: 150, y: 100 });

        expect(rotateDeltaEmitted).toBe(false);
    });

    it('drag-rotate active → Q/E pressed → Q/E blocked (bidirectional)', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);

        // Start drag-rotate first
        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onDragRotate({ x: 100, y: 100 }, { x: 150, y: 100 });

        // Try to activate Q/E - should be blocked
        manager.onRotate(-1);
        const state = manager.getLocomotionState();

        expect(state.rotateRate).toBe(0); // Blocked
    });

    it('Q/E released → drag-rotate unblocked', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let rotateDeltaEmitted = false;
        mockEvents.addEventListener('cursor:rotate_delta', () => {
            rotateDeltaEmitted = true;
        });

        // Hold Q/E
        manager.onRotate(-1);

        // Try to start drag-rotate (blocked)
        manager.onDragRotateStart({ x: 100, y: 100 });

        // Release Q/E
        manager.onRotate(0);

        // End first drag-rotate
        manager.onDragRotateEnd({ x: 100, y: 100 });

        // Start new drag-rotate (should work now)
        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onDragRotate({ x: 100, y: 100 }, { x: 150, y: 100 });

        expect(rotateDeltaEmitted).toBe(true);
    });

    it('Q/E held → drag-pan allowed (CURSOR_MOVE_TO emitted)', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let moveToEmitted = false;
        mockEvents.addEventListener('cursor:move_to', () => {
            moveToEmitted = true;
        });

        // Hold Q/E
        manager.onRotate(-1);

        // Drag-pan should still work
        manager.onDragPanStart({ x: 100, y: 100 });
        manager.onDragPan({ x: 100, y: 100 }, { x: 150, y: 150 });

        expect(moveToEmitted).toBe(true);
    });

    it('Q/E held → wheel zoom allowed (CURSOR_ZOOM_DELTA emitted)', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let zoomDeltaEmitted = false;
        mockEvents.addEventListener('cursor:zoom_delta', () => {
            zoomDeltaEmitted = true;
        });

        // Hold Q/E
        manager.onRotate(-1);

        // Wheel zoom should still work
        manager.onZoomTo({ x: 100, y: 100 }, 10);

        expect(zoomDeltaEmitted).toBe(true);
    });
});

// ─── InputManager - Gesture Events ───────────────────────────────────────────

describe('InputManager - Gesture Events', () => {
    let mockCamera: Camera;
    let mockWorldCursor: WorldCursor;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockCamera = {
            screenToWorldPlanePoint: (x: number, y: number) => {
                return { x: x * 10, y: y * 10, z: 0 };
            }
        } as unknown as Camera;
        mockWorldCursor = {} as WorldCursor;
        mockEvents = new EventTarget();
    });

    it('onTap emits CURSOR_MOVE_TO with raycasted world position', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let eventData: { pos: { x: number; y: number; z: number } } | null = null;
        mockEvents.addEventListener('cursor:move_to', (e: any) => {
            eventData = e.detail;
        });

        manager.onTap({ x: 10, y: 20 });

        expect(eventData).not.toBeNull();
        expect(eventData!.pos.x).toBe(100);
        expect(eventData!.pos.y).toBe(200);
        expect(eventData!.pos.z).toBe(0);
    });

    it('onPinch emits CURSOR_ZOOM_DELTA proportional to distance change', () => {
        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let eventData: { delta: number } | null = null;
        mockEvents.addEventListener('cursor:zoom_delta', (e: any) => {
            eventData = e.detail;
        });

        const start: [Point, Point] = [
            { x: 100, y: 100 },
            { x: 200, y: 100 }
        ];
        const current: [Point, Point] = [
            { x: 100, y: 100 },
            { x: 180, y: 100 }
        ];

        manager.onPinchStart(start, start);
        manager.onPinch(start, current);

        expect(eventData).not.toBeNull();
        expect(eventData!.delta).toBeCloseTo(20, 1);
    });
});

// ─── WorldCursor - Setters ───────────────────────────────────────────────────

describe('WorldCursor - Setters', () => {
    let cursor: WorldCursor;
    let mockRenderContext: RenderContext;
    let mockEvents: EventTarget;

    beforeEach(() => {
        mockRenderContext = {
            scene: { add: vi.fn(), remove: vi.fn() }
        } as unknown as RenderContext;
        mockEvents = new EventTarget();
        cursor = new WorldCursor(mockRenderContext, mockEvents);
    });

    it('setPosition should update position', () => {
        cursor.setPosition({ x: 100, y: 200, z: 0 });

        const target = cursor.getCameraTarget();
        expect(target.cursorPosition.x).toBe(100);
        expect(target.cursorPosition.y).toBe(200);
        expect(target.cursorPosition.z).toBe(0);
    });

    it('setYaw should normalize angle to [0, 2π)', () => {
        cursor.setYaw(Math.PI * 3); // 540 degrees

        const target = cursor.getCameraTarget();
        expect(target.yaw).toBeCloseTo(Math.PI, 5); // normalized to 180 degrees
    });

    it('setYaw with negative angle should normalize to [0, 2π)', () => {
        cursor.setYaw(-Math.PI / 2); // -90 degrees

        const target = cursor.getCameraTarget();
        expect(target.yaw).toBeCloseTo(Math.PI * 1.5, 5); // normalized to 270 degrees
    });

    it('setZoom should clamp to [MIN_CAMERA_DISTANCE, MAX_CAMERA_DISTANCE]', () => {
        cursor.setZoom(MIN_CAMERA_DISTANCE - 100);
        let target = cursor.getCameraTarget();
        expect(target.distance).toBe(MIN_CAMERA_DISTANCE);

        cursor.setZoom(MAX_CAMERA_DISTANCE + 100);
        target = cursor.getCameraTarget();
        expect(target.distance).toBe(MAX_CAMERA_DISTANCE);

        cursor.setZoom(500);
        target = cursor.getCameraTarget();
        expect(target.distance).toBe(500);
    });

    it('getCameraTarget returns consistent values after setters', () => {
        cursor.setPosition({ x: 50, y: 75, z: 0 });
        cursor.setYaw(Math.PI / 4);
        cursor.setZoom(400);

        const target = cursor.getCameraTarget();
        expect(target.cursorPosition.x).toBe(50);
        expect(target.cursorPosition.y).toBe(75);
        expect(target.yaw).toBeCloseTo(Math.PI / 4, 5);
        expect(target.distance).toBe(400);
    });
});

// ─── Events - Gesture Events Only ─────────────────────────────────────────────

describe('Events - Gesture Events Only', () => {
    it('CURSOR_MOVE_TO exists', () => {
        const mockCamera = {
            screenToWorldPlanePoint: (x: number, y: number) => {
                return { x: x * 10, y: y * 10, z: 0 };
            }
        } as unknown as Camera;
        const mockWorldCursor = {} as WorldCursor;
        const mockEvents = new EventTarget();

        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let eventReceived = false;

        mockEvents.addEventListener('cursor:move_to', () => {
            eventReceived = true;
        });

        manager.onTap({ x: 10, y: 20 });
        expect(eventReceived).toBe(true);
    });

    it('CURSOR_ROTATE_DELTA exists', () => {
        const mockCamera = {
            screenToWorldPlanePoint: (x: number, y: number) => {
                return { x: x * 10, y: y * 10, z: 0 };
            }
        } as unknown as Camera;
        const mockWorldCursor = {} as WorldCursor;
        const mockEvents = new EventTarget();

        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let eventReceived = false;

        mockEvents.addEventListener('cursor:rotate_delta', () => {
            eventReceived = true;
        });

        manager.onDragRotateStart({ x: 100, y: 100 });
        manager.onDragRotate({ x: 100, y: 100 }, { x: 150, y: 100 });
        expect(eventReceived).toBe(true);
    });

    it('CURSOR_ZOOM_DELTA exists', () => {
        const mockCamera = {
            screenToWorldPlanePoint: (x: number, y: number) => {
                return { x: x * 10, y: y * 10, z: 0 };
            }
        } as unknown as Camera;
        const mockWorldCursor = {} as WorldCursor;
        const mockEvents = new EventTarget();

        const manager = new InputManager(mockCamera, mockWorldCursor, mockEvents);
        let eventReceived = false;

        mockEvents.addEventListener('cursor:zoom_delta', () => {
            eventReceived = true;
        });

        manager.onZoomTo({ x: 100, y: 100 }, 10);
        expect(eventReceived).toBe(true);
    });
});
