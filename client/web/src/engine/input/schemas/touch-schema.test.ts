// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { TouchSchema } from './touch-schema';
import { LONG_PRESS_MS } from '../../../constants';

describe('TouchSchema', () => {
    let schema: TouchSchema;
    let container: HTMLElement;

    beforeEach(() => {
        container = document.createElement('div');
        schema = new TouchSchema(container);
    });

    describe('Single-finger blocks two-finger', () => {
        it('24. Single-finger drag starts → twoFingerGesture.enabled = false', () => {
            const onPinchStart = vi.fn();
            schema.onPinchStart = onPinchStart;

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

            expect(onPinchStart).not.toHaveBeenCalled();
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
            const onPinchStart = vi.fn();
            schema.onPinchStart = onPinchStart;

            const touchStart2 = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 2, clientX: 100, clientY: 100 } as Touch,
                    { identifier: 3, clientX: 200, clientY: 200 } as Touch
                ]
            });
            container.dispatchEvent(touchStart2);

            expect(onPinchStart).toHaveBeenCalled();
        });

        it('26. Single-finger long-drag starts → twoFingerGesture.enabled = false', () => {
            const onPinchStart = vi.fn();
            schema.onPinchStart = onPinchStart;

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

            expect(onPinchStart).not.toHaveBeenCalled();

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
            const onPinchStart = vi.fn();
            schema.onPinchStart = onPinchStart;

            const touchStart2 = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 2, clientX: 100, clientY: 100 } as Touch,
                    { identifier: 3, clientX: 200, clientY: 200 } as Touch
                ],
                bubbles: true
            });
            container.dispatchEvent(touchStart2);

            expect(onPinchStart).toHaveBeenCalled();
        });
    });

    describe('Two-finger blocks single-finger', () => {
        it('28. Two-finger starts → singleTouch.enabled = false', () => {
            const onMoveTo = vi.fn();
            schema.onMoveTo = onMoveTo;

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

            expect(onMoveTo).not.toHaveBeenCalled();
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

            // Single-touch callbacks should still be blocked
            const onMoveTo = vi.fn();
            schema.onMoveTo = onMoveTo;

            const touchMove = new TouchEvent('touchmove', {
                touches: [{ identifier: 1, clientX: 110, clientY: 110 } as Touch]
            });
            container.dispatchEvent(touchMove);

            expect(onMoveTo).not.toHaveBeenCalled();
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
            const onMoveTo = vi.fn();
            schema.onMoveTo = onMoveTo;

            const touchStart2 = new TouchEvent('touchstart', {
                touches: [{ identifier: 3, clientX: 150, clientY: 150 } as Touch]
            });
            container.dispatchEvent(touchStart2);

            const touchEnd3 = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 3, clientX: 150, clientY: 150 } as Touch],
                touches: []
            });
            container.dispatchEvent(touchEnd3);

            expect(onMoveTo).toHaveBeenCalled();
        });
    });

    describe('Callback emission', () => {
        it('31. Single-finger tap → onMoveTo called', () => {
            const onMoveTo = vi.fn();
            schema.onMoveTo = onMoveTo;

            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchStart);

            const touchEnd = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch],
                touches: []
            });
            container.dispatchEvent(touchEnd);

            expect(onMoveTo).toHaveBeenCalledWith({ x: 100, y: 100 });
        });

        it('32. Single-finger drag → onMoveTo called per frame', () => {
            const onMoveTo = vi.fn();
            schema.onMoveTo = onMoveTo;

            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchStart);

            const touchMove = new TouchEvent('touchmove', {
                touches: [{ identifier: 1, clientX: 150, clientY: 150 } as Touch]
            });
            container.dispatchEvent(touchMove);

            expect(onMoveTo).toHaveBeenCalled();
        });

        it('33. Two-finger pinch start → onPinchStart(pos1, pos2) called', () => {
            const onPinchStart = vi.fn();
            schema.onPinchStart = onPinchStart;

            const touchStart = new TouchEvent('touchstart', {
                touches: [
                    { identifier: 1, clientX: 100, clientY: 100 } as Touch,
                    { identifier: 2, clientX: 200, clientY: 200 } as Touch
                ]
            });
            container.dispatchEvent(touchStart);

            expect(onPinchStart).toHaveBeenCalled();
            const [pos1, pos2] = onPinchStart.mock.calls[0];
            expect(pos1).toEqual({ x: 100, y: 100 });
            expect(pos2).toEqual({ x: 200, y: 200 });
        });

        it('34. Two-finger pinch move → onPinch(pos1, pos2) called per frame', () => {
            const onPinch = vi.fn();
            schema.onPinch = onPinch;

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

            expect(onPinch).toHaveBeenCalled();
        });

        it('35. Two-finger pinch end → onPinchEnd() called', () => {
            const onPinchEnd = vi.fn();
            schema.onPinchEnd = onPinchEnd;

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

            expect(onPinchEnd).toHaveBeenCalled();
        });

        it('36. Long-press → onInteractStart/onInteract/onInteractEnd sequence', () => {
            vi.useFakeTimers();

            const onInteractStart = vi.fn();
            const onInteract = vi.fn();
            const onInteractEnd = vi.fn();
            schema.onInteractStart = onInteractStart;
            schema.onInteract = onInteract;
            schema.onInteractEnd = onInteractEnd;

            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch],
                bubbles: true
            });
            container.dispatchEvent(touchStart);

            vi.advanceTimersByTime(LONG_PRESS_MS);
            expect(onInteractStart).toHaveBeenCalledWith({ x: 100, y: 100 });

            const touchMove = new TouchEvent('touchmove', {
                touches: [{ identifier: 1, clientX: 120, clientY: 120 } as Touch],
                bubbles: true
            });
            container.dispatchEvent(touchMove);
            expect(onInteract).toHaveBeenCalled();

            const touchEnd = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 1, clientX: 120, clientY: 120 } as Touch],
                touches: [],
                bubbles: true
            });
            container.dispatchEvent(touchEnd);
            expect(onInteractEnd).toHaveBeenCalledWith({ x: 120, y: 120 });

            vi.useRealTimers();
        });
    });
});
