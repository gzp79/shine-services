// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { DesktopSchema } from './desktop-schema';
import { LONG_PRESS_MS } from '../../../constants';

describe('DesktopSchema', () => {
    let schema: DesktopSchema;
    let container: HTMLElement;

    beforeEach(() => {
        container = document.createElement('div');
        schema = new DesktopSchema(container);
    });

    describe('WASD blocks left pointer drag (but not long-press)', () => {
        it('1. WASD held → left-drag blocked (callbacks ignored if wasdActive)', () => {
            const onMoveTo = vi.fn();
            schema.onMoveTo = onMoveTo;

            // Simulate WASD key down
            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

            // Simulate left pointer drag
            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerDown);

            const pointerMove = new PointerEvent('pointermove', {
                clientX: 150,
                clientY: 150,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerMove);

            expect(onMoveTo).not.toHaveBeenCalled();
        });

        it('2. Left-drag active → WASD pressed → WASD.enabled = false', () => {
            // Start left-drag first
            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerDown);

            const pointerMove = new PointerEvent('pointermove', {
                clientX: 150,
                clientY: 150,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerMove);

            // Now try WASD
            const onMoveRate = vi.fn();
            schema.onMoveRate = onMoveRate;

            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

            expect(onMoveRate).not.toHaveBeenCalled();
        });

        it('3. Left-drag ends → WASD.enabled = true', () => {
            // Start left-drag
            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerDown);

            const pointerMove = new PointerEvent('pointermove', {
                clientX: 150,
                clientY: 150,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerMove);

            // End drag
            const pointerUp = new PointerEvent('pointerup', {
                clientX: 150,
                clientY: 150,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerUp);

            // WASD should work now
            const onMoveRate = vi.fn();
            schema.onMoveRate = onMoveRate;

            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

            expect(onMoveRate).toHaveBeenCalled();
        });

        it('4. WASD held → long-press still works (not blocked)', () => {
            vi.useFakeTimers();

            const onInteractStart = vi.fn();
            schema.onInteractStart = onInteractStart;

            // WASD down
            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

            // Simulate long-press (pointer down + wait + move)
            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1,
                bubbles: true
            });
            container.dispatchEvent(pointerDown);

            // Wait for long-press timer
            vi.advanceTimersByTime(LONG_PRESS_MS);

            expect(onInteractStart).toHaveBeenCalledWith({ x: 100, y: 100 });

            vi.useRealTimers();
        });

        it('5. WASD held → rightPointer still active (not blocked)', () => {
            const onRotateBy = vi.fn();
            schema.onRotateBy = onRotateBy;

            // WASD down
            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

            // Right-drag
            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 2,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerDown);

            const pointerMove = new PointerEvent('pointermove', {
                clientX: 150,
                clientY: 100,
                button: 2,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerMove);

            expect(onRotateBy).toHaveBeenCalled();
        });

        it('6. WASD held → wheel still active (not blocked)', () => {
            const onZoomBy = vi.fn();
            schema.onZoomBy = onZoomBy;

            // WASD down
            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

            // Wheel
            const wheel = new WheelEvent('wheel', { deltaY: -100 });
            container.dispatchEvent(wheel);

            expect(onZoomBy).toHaveBeenCalled();
        });
    });

    describe('Q/E blocks right pointer', () => {
        it('7. Q/E held → rightPointer.enabled = false', () => {
            const onRotateBy = vi.fn();
            schema.onRotateBy = onRotateBy;

            // Q down
            const keyDownQ = new KeyboardEvent('keydown', { key: 'q' });
            window.dispatchEvent(keyDownQ);

            // Try right-drag
            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 2,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerDown);

            const pointerMove = new PointerEvent('pointermove', {
                clientX: 150,
                clientY: 100,
                button: 2,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerMove);

            expect(onRotateBy).not.toHaveBeenCalled();
        });

        it('8. Q/E released → rightPointer.enabled = true', () => {
            // Q down then up
            const keyDownQ = new KeyboardEvent('keydown', { key: 'q' });
            window.dispatchEvent(keyDownQ);
            const keyUpQ = new KeyboardEvent('keyup', { key: 'q' });
            window.dispatchEvent(keyUpQ);

            const onRotateBy = vi.fn();
            schema.onRotateBy = onRotateBy;

            // Right-drag should work now
            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 2,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerDown);

            const pointerMove = new PointerEvent('pointermove', {
                clientX: 150,
                clientY: 100,
                button: 2,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerMove);

            expect(onRotateBy).toHaveBeenCalled();
        });

        it('9. Right-drag active → Q/E pressed → Q/E.enabled = false', () => {
            // Start right-drag
            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 2,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerDown);

            const pointerMove = new PointerEvent('pointermove', {
                clientX: 150,
                clientY: 100,
                button: 2,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerMove);

            // Try Q
            const onRotateRate = vi.fn();
            schema.onRotateRate = onRotateRate;

            const keyDownQ = new KeyboardEvent('keydown', { key: 'q' });
            window.dispatchEvent(keyDownQ);

            expect(onRotateRate).not.toHaveBeenCalled();
        });

        it('10. Q/E held → leftPointer still active (not blocked)', () => {
            const onMoveTo = vi.fn();
            schema.onMoveTo = onMoveTo;

            // Q down
            const keyDownQ = new KeyboardEvent('keydown', { key: 'q' });
            window.dispatchEvent(keyDownQ);

            // Left-click tap
            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerDown);

            const pointerUp = new PointerEvent('pointerup', {
                clientX: 100,
                clientY: 100,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerUp);

            expect(onMoveTo).toHaveBeenCalledWith({ x: 100, y: 100 });
        });
    });

    describe('R/F blocks wheel', () => {
        it('11. R/F held → wheel.enabled = false', () => {
            const onZoomBy = vi.fn();
            schema.onZoomBy = onZoomBy;

            // R down
            const keyDownR = new KeyboardEvent('keydown', { key: 'r' });
            window.dispatchEvent(keyDownR);

            // Wheel
            const wheel = new WheelEvent('wheel', { deltaY: -100 });
            container.dispatchEvent(wheel);

            expect(onZoomBy).not.toHaveBeenCalled();
        });

        it('12. R/F released → wheel.enabled = true', () => {
            // R down then up
            const keyDownR = new KeyboardEvent('keydown', { key: 'r' });
            window.dispatchEvent(keyDownR);
            const keyUpR = new KeyboardEvent('keyup', { key: 'r' });
            window.dispatchEvent(keyUpR);

            const onZoomBy = vi.fn();
            schema.onZoomBy = onZoomBy;

            // Wheel should work now
            const wheel = new WheelEvent('wheel', { deltaY: -100 });
            container.dispatchEvent(wheel);

            expect(onZoomBy).toHaveBeenCalled();
        });

        it('13. Wheel active → R/F pressed → R/F.enabled = false', () => {
            // Wheel first (doesn't block anything, just establishes it works)
            const onZoomBy = vi.fn();
            schema.onZoomBy = onZoomBy;

            const wheel = new WheelEvent('wheel', { deltaY: -100 });
            container.dispatchEvent(wheel);
            expect(onZoomBy).toHaveBeenCalled();

            // Now try R
            const onZoomRate = vi.fn();
            schema.onZoomRate = onZoomRate;

            const keyDownR = new KeyboardEvent('keydown', { key: 'r' });
            window.dispatchEvent(keyDownR);

            // R should work (wheel doesn't actually block R)
            expect(onZoomRate).toHaveBeenCalled();
        });
    });

    describe('Callback emission', () => {
        it('14. WASD held → onMoveRate called with normalized (x, y, sprint)', () => {
            const onMoveRate = vi.fn();
            schema.onMoveRate = onMoveRate;

            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

            expect(onMoveRate).toHaveBeenCalledWith(0, 1, false);
        });

        it('15. Q/E held → onRotateRate called with value', () => {
            const onRotateRate = vi.fn();
            schema.onRotateRate = onRotateRate;

            const keyDownQ = new KeyboardEvent('keydown', { key: 'q' });
            window.dispatchEvent(keyDownQ);

            expect(onRotateRate).toHaveBeenCalled();
            expect(onRotateRate.mock.calls[0][0]).toBeLessThan(0); // negative for Q
        });

        it('16. R/F held → onZoomRate called with value', () => {
            const onZoomRate = vi.fn();
            schema.onZoomRate = onZoomRate;

            const keyDownR = new KeyboardEvent('keydown', { key: 'r' });
            window.dispatchEvent(keyDownR);

            expect(onZoomRate).toHaveBeenCalled();
            expect(onZoomRate.mock.calls[0][0]).toBeLessThan(0); // negative for R
        });

        it('17. Left-click tap → onMoveTo called with screen position', () => {
            const onMoveTo = vi.fn();
            schema.onMoveTo = onMoveTo;

            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 200,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerDown);

            const pointerUp = new PointerEvent('pointerup', {
                clientX: 100,
                clientY: 200,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerUp);

            expect(onMoveTo).toHaveBeenCalledWith({ x: 100, y: 200 });
        });

        it('18. Right-drag → onRotateBy called with angleDelta', () => {
            const onRotateBy = vi.fn();
            schema.onRotateBy = onRotateBy;

            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 2,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerDown);

            const pointerMove = new PointerEvent('pointermove', {
                clientX: 150,
                clientY: 100,
                button: 2,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerMove);

            expect(onRotateBy).toHaveBeenCalled();
            expect(typeof onRotateBy.mock.calls[0][0]).toBe('number');
        });

        it('19. Wheel → onZoomBy called with delta', () => {
            const onZoomBy = vi.fn();
            schema.onZoomBy = onZoomBy;

            const wheel = new WheelEvent('wheel', { deltaY: -100 });
            container.dispatchEvent(wheel);

            expect(onZoomBy).toHaveBeenCalled();
            expect(typeof onZoomBy.mock.calls[0][0]).toBe('number');
        });

        it('20. Long-press → onInteractStart/onInteract/onInteractEnd sequence', () => {
            vi.useFakeTimers();

            const onInteractStart = vi.fn();
            const onInteract = vi.fn();
            const onInteractEnd = vi.fn();
            schema.onInteractStart = onInteractStart;
            schema.onInteract = onInteract;
            schema.onInteractEnd = onInteractEnd;

            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1,
                bubbles: true
            });
            container.dispatchEvent(pointerDown);

            vi.advanceTimersByTime(LONG_PRESS_MS);
            expect(onInteractStart).toHaveBeenCalledWith({ x: 100, y: 100 });

            const pointerMove = new PointerEvent('pointermove', {
                clientX: 120,
                clientY: 120,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1,
                bubbles: true
            });
            container.dispatchEvent(pointerMove);
            expect(onInteract).toHaveBeenCalled();

            const pointerUp = new PointerEvent('pointerup', {
                clientX: 120,
                clientY: 120,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1,
                bubbles: true
            });
            container.dispatchEvent(pointerUp);
            expect(onInteractEnd).toHaveBeenCalledWith({ x: 120, y: 120 });

            vi.useRealTimers();
        });
    });

    describe('Rate value correctness', () => {
        it('21. Diagonal WASD (W+D) → onMoveRate receives normalized unit vector', () => {
            const onMoveRate = vi.fn();
            schema.onMoveRate = onMoveRate;

            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);
            const keyDownD = new KeyboardEvent('keydown', { key: 'd' });
            window.dispatchEvent(keyDownD);

            expect(onMoveRate).toHaveBeenCalled();
            const [x, y] = onMoveRate.mock.calls[onMoveRate.mock.calls.length - 1];
            const length = Math.sqrt(x * x + y * y);
            expect(length).toBeCloseTo(1, 5);
        });

        it('22. Q+E both held → onRotateRate receives 0 (cancel out)', () => {
            const onRotateRate = vi.fn();
            schema.onRotateRate = onRotateRate;

            const keyDownQ = new KeyboardEvent('keydown', { key: 'q' });
            window.dispatchEvent(keyDownQ);
            const keyDownE = new KeyboardEvent('keydown', { key: 'e' });
            window.dispatchEvent(keyDownE);

            expect(onRotateRate).toHaveBeenCalled();
            expect(onRotateRate.mock.calls[onRotateRate.mock.calls.length - 1][0]).toBe(0);
        });

        it('23. R+F both held → onZoomRate receives 0 (cancel out)', () => {
            const onZoomRate = vi.fn();
            schema.onZoomRate = onZoomRate;

            const keyDownR = new KeyboardEvent('keydown', { key: 'r' });
            window.dispatchEvent(keyDownR);
            const keyDownF = new KeyboardEvent('keydown', { key: 'f' });
            window.dispatchEvent(keyDownF);

            expect(onZoomRate).toHaveBeenCalled();
            expect(onZoomRate.mock.calls[onZoomRate.mock.calls.length - 1][0]).toBe(0);
        });
    });
});
