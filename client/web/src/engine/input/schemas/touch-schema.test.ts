// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach, afterEach, type MockedObject } from 'vitest';
import { TouchSchema } from './touch-schema';
import { LONG_PRESS_MS } from '../../../constants';
import type { InputHandler } from '../input-handler';

describe('TouchSchema', () => {
    let schema: TouchSchema;
    let container: HTMLElement;
    let handler: MockedObject<InputHandler>;

    beforeEach(() => {
        container = document.createElement('div');
        handler = {
            onSchemaChanged: vi.fn(),
            onPointerAt: vi.fn(),
            onPointerLeave: vi.fn(),
            onMoveTo: vi.fn(),
            onRotateBy: vi.fn(),
            onZoomBy: vi.fn(),
            onMoveRate: vi.fn(),
            onRotateRate: vi.fn(),
            onZoomRate: vi.fn(),
            onPinchStart: vi.fn(),
            onPinch: vi.fn(),
            onPinchEnd: vi.fn(),
            onInteractStart: vi.fn(),
            onInteract: vi.fn(),
            onInteractEnd: vi.fn(),
            onGesture: vi.fn()
        } satisfies InputHandler;
        schema = new TouchSchema(container, handler);
    });

    afterEach(() => {
        schema.dispose();
    });

    describe('Single-finger blocks two-finger', () => {
        it('24. Single-finger drag starts → twoFingerGesture.enabled = false', () => {
            // Start single-finger drag
            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchStart);

            const touchMove = new TouchEvent('touchmove', {
                touches: [{ identifier: 1, clientX: 150, clientY: 150 } as Touch]
            });
            container.dispatchEvent(touchMove);

            // Try to start two-finger (should be blocked)
            const touchStart2 = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 1, clientX: 150, clientY: 150 } as Touch,
                    { identifier: 2, clientX: 200, clientY: 200 } as Touch
                ]
            });
            container.dispatchEvent(touchStart2);

            expect(handler.onPinchStart).not.toHaveBeenCalled();
        });

        it('25. Single-finger drag ends → twoFingerGesture.enabled = true', () => {
            // Start and end single-finger drag
            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchStart);

            const touchMove = new TouchEvent('touchmove', {
                touches: [{ identifier: 1, clientX: 150, clientY: 150 } as Touch]
            });
            container.dispatchEvent(touchMove);

            const touchEnd = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 1, clientX: 150, clientY: 150 } as Touch],
                touches: []
            });
            container.dispatchEvent(touchEnd);

            // Two-finger should work now
            const touchStart2 = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 2, clientX: 100, clientY: 100 } as Touch,
                    { identifier: 3, clientX: 200, clientY: 200 } as Touch
                ]
            });
            container.dispatchEvent(touchStart2);

            expect(handler.onPinchStart).toHaveBeenCalled();
        });

        it('26. Single-finger long-drag starts → twoFingerGesture.enabled = false', () => {
            vi.useFakeTimers();

            // Start single-finger long-press
            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchStart);

            vi.advanceTimersByTime(LONG_PRESS_MS); // Wait for long-press

            // Try two-finger (should be blocked)
            const touchStart2 = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 1, clientX: 100, clientY: 100 } as Touch,
                    { identifier: 2, clientX: 200, clientY: 200 } as Touch
                ]
            });
            container.dispatchEvent(touchStart2);

            expect(handler.onPinchStart).not.toHaveBeenCalled();

            vi.useRealTimers();
        });

        it('27. Single-finger long-drag ends → twoFingerGesture.enabled = true', () => {
            vi.useFakeTimers();

            // Start long-drag
            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch],
                bubbles: true
            });
            container.dispatchEvent(touchStart);

            vi.advanceTimersByTime(LONG_PRESS_MS);

            // Move to trigger long-drag
            const touchMove = new TouchEvent('touchmove', {
                touches: [{ identifier: 1, clientX: 120, clientY: 120 } as Touch],
                bubbles: true
            });
            container.dispatchEvent(touchMove);

            // End long-drag
            const touchEnd = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 1, clientX: 120, clientY: 120 } as Touch],
                touches: [],
                bubbles: true
            });
            container.dispatchEvent(touchEnd);

            vi.useRealTimers();

            // Two-finger should work now
            const touchStart2 = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 2, clientX: 100, clientY: 100 } as Touch,
                    { identifier: 3, clientX: 200, clientY: 200 } as Touch
                ],
                bubbles: true
            });
            container.dispatchEvent(touchStart2);

            expect(handler.onPinchStart).toHaveBeenCalled();
        });
    });

    describe('Two-finger blocks single-finger', () => {
        it('28. Two-finger starts → singleTouch.enabled = false', () => {
            // Start two-finger
            const touchStart = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 1, clientX: 100, clientY: 100 } as Touch,
                    { identifier: 2, clientX: 200, clientY: 200 } as Touch
                ]
            });
            container.dispatchEvent(touchStart);

            // Try single-finger tap (should be blocked)
            const touchStart2 = new TouchEvent('touchstart', {
                touches: [{ identifier: 3, clientX: 150, clientY: 150 } as Touch]
            });
            container.dispatchEvent(touchStart2);

            expect(handler.onMoveTo).not.toHaveBeenCalled();
        });

        it('29. Two-finger ends, 1 finger remains → singleTouch still disabled', () => {
            // Start two-finger
            const touchStart = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 1, clientX: 100, clientY: 100 } as Touch,
                    { identifier: 2, clientX: 200, clientY: 200 } as Touch
                ]
            });
            container.dispatchEvent(touchStart);

            // One finger lifts
            const touchEnd = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 2, clientX: 200, clientY: 200 } as Touch],
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchEnd);

            vi.clearAllMocks();

            const touchMove = new TouchEvent('touchmove', {
                touches: [{ identifier: 1, clientX: 110, clientY: 110 } as Touch]
            });
            container.dispatchEvent(touchMove);

            expect(handler.onMoveTo).not.toHaveBeenCalled();
        });

        it('30. All fingers released → singleTouch.enabled = true', () => {
            // Start two-finger
            const touchStart = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 1, clientX: 100, clientY: 100 } as Touch,
                    { identifier: 2, clientX: 200, clientY: 200 } as Touch
                ]
            });
            container.dispatchEvent(touchStart);

            // One finger lifts
            const touchEnd1 = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 2, clientX: 200, clientY: 200 } as Touch],
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchEnd1);

            // All fingers lift
            const touchEnd2 = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch],
                touches: []
            });
            container.dispatchEvent(touchEnd2);

            // Single-finger should work now
            const touchStart2 = new TouchEvent('touchstart', {
                touches: [{ identifier: 3, clientX: 150, clientY: 150 } as Touch]
            });
            container.dispatchEvent(touchStart2);

            const touchEnd3 = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 3, clientX: 150, clientY: 150 } as Touch],
                touches: []
            });
            container.dispatchEvent(touchEnd3);

            expect(handler.onMoveTo).toHaveBeenCalled();
        });
    });

    describe('Callback emission', () => {
        it('31. Single-finger tap → onMoveTo called', () => {
            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchStart);

            const touchEnd = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch],
                touches: []
            });
            container.dispatchEvent(touchEnd);

            expect(handler.onMoveTo).toHaveBeenCalledWith({ x: 100, y: 100 });
        });

        it('32. Single-finger drag → onMoveTo called per frame', () => {
            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchStart);

            const touchMove = new TouchEvent('touchmove', {
                touches: [{ identifier: 1, clientX: 150, clientY: 150 } as Touch]
            });
            container.dispatchEvent(touchMove);

            expect(handler.onMoveTo).toHaveBeenCalled();
        });

        it('33. Two-finger pinch start → onPinchStart called', () => {
            const touchStart = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 1, clientX: 100, clientY: 100 } as Touch,
                    { identifier: 2, clientX: 200, clientY: 200 } as Touch
                ]
            });
            container.dispatchEvent(touchStart);

            expect(handler.onPinchStart).toHaveBeenCalled();
        });

        it('34. Two-finger pinch move → onPinch called per frame', () => {
            const touchStart = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 1, clientX: 100, clientY: 100 } as Touch,
                    { identifier: 2, clientX: 200, clientY: 200 } as Touch
                ]
            });
            container.dispatchEvent(touchStart);

            const touchMove = new TouchEvent('touchmove', {
                touches: [
                    { identifier: 1, clientX: 110, clientY: 110 } as Touch,
                    { identifier: 2, clientX: 210, clientY: 210 } as Touch
                ]
            });
            container.dispatchEvent(touchMove);

            expect(handler.onPinch).toHaveBeenCalled();
        });

        it('35. Two-finger pinch end → onPinchEnd called', () => {
            const touchStart = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 1, clientX: 100, clientY: 100 } as Touch,
                    { identifier: 2, clientX: 200, clientY: 200 } as Touch
                ]
            });
            container.dispatchEvent(touchStart);

            const touchEnd = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 2, clientX: 200, clientY: 200 } as Touch],
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchEnd);

            expect(handler.onPinchEnd).toHaveBeenCalled();
        });

        it('36. Long-press → onInteractStart/onInteract/onInteractEnd sequence', () => {
            vi.useFakeTimers();

            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch],
                bubbles: true
            });
            container.dispatchEvent(touchStart);

            vi.advanceTimersByTime(LONG_PRESS_MS);
            expect(handler.onInteractStart).toHaveBeenCalledWith({ x: 100, y: 100 });

            const touchMove = new TouchEvent('touchmove', {
                touches: [{ identifier: 1, clientX: 120, clientY: 120 } as Touch],
                bubbles: true
            });
            container.dispatchEvent(touchMove);
            expect(handler.onInteract).toHaveBeenCalled();

            const touchEnd = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 1, clientX: 120, clientY: 120 } as Touch],
                touches: [],
                bubbles: true
            });
            container.dispatchEvent(touchEnd);
            expect(handler.onInteractEnd).toHaveBeenCalledWith({ x: 100, y: 100 }, { x: 120, y: 120 });

            vi.useRealTimers();
        });
    });
});
