// @vitest-environment jsdom
import { type MockedObject, afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { InputConst } from '../../../constants';
import type { InputHandler } from '../input-handler';
import { DesktopSchema } from './desktop-schema';

// jsdom does not implement ResizeObserver — stub it so RawPointerTracker can construct
vi.stubGlobal(
    'ResizeObserver',
    class {
        observe() {}
        unobserve() {}
        disconnect() {}
    }
);

describe('DesktopSchema', () => {
    let schema: DesktopSchema;
    let container: HTMLElement;
    let handler: MockedObject<InputHandler>;

    beforeEach(() => {
        container = document.createElement('div');
        handler = {
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
            onSchemaChanged: vi.fn(),
            onGesture: vi.fn()
        } satisfies InputHandler;

        schema = new DesktopSchema(container, handler);
    });

    afterEach(() => {
        schema.dispose();
    });

    describe('WASD blocks left pointer drag (but not long-press)', () => {
        it('1. WASD held → left-drag blocked (callbacks ignored if wasdActive)', () => {
            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

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

            expect(handler.onMoveTo).not.toHaveBeenCalled();
        });

        it('2. Left-drag active → WASD pressed → WASD.enabled = false', () => {
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

            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

            expect(handler.onMoveRate).not.toHaveBeenCalled();
        });

        it('3. Left-drag ends → WASD.enabled = true', () => {
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

            const pointerUp = new PointerEvent('pointerup', {
                clientX: 150,
                clientY: 150,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerUp);

            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

            expect(handler.onMoveRate).toHaveBeenCalled();
        });

        it('4. WASD held → long-press still works (not blocked)', () => {
            vi.useFakeTimers();

            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1,
                bubbles: true
            });
            container.dispatchEvent(pointerDown);

            vi.advanceTimersByTime(InputConst.LONG_PRESS_MS);

            expect(handler.onInteractStart).toHaveBeenCalledWith({ x: 100, y: 100 });

            vi.useRealTimers();
        });

        it('5. WASD held → rightPointer still active (not blocked)', () => {
            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

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

            expect(handler.onRotateBy).toHaveBeenCalled();
        });

        it('6. WASD held → wheel still active (not blocked)', () => {
            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

            const wheel = new WheelEvent('wheel', { deltaY: -100 });
            container.dispatchEvent(wheel);

            expect(handler.onZoomBy).toHaveBeenCalled();
        });
    });

    describe('Q/E blocks right pointer', () => {
        it('7. Q/E held → rightPointer.enabled = false', () => {
            const keyDownQ = new KeyboardEvent('keydown', { key: 'q' });
            window.dispatchEvent(keyDownQ);

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

            expect(handler.onRotateBy).not.toHaveBeenCalled();
        });

        it('8. Q/E released → rightPointer.enabled = true', () => {
            const keyDownQ = new KeyboardEvent('keydown', { key: 'q' });
            window.dispatchEvent(keyDownQ);
            const keyUpQ = new KeyboardEvent('keyup', { key: 'q' });
            window.dispatchEvent(keyUpQ);

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

            expect(handler.onRotateBy).toHaveBeenCalled();
        });

        it('9. Right-drag active → Q/E pressed → Q/E.enabled = false', () => {
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

            const keyDownQ = new KeyboardEvent('keydown', { key: 'q' });
            window.dispatchEvent(keyDownQ);

            expect(handler.onRotateRate).not.toHaveBeenCalled();
        });

        it('10. Q/E held → leftPointer still active (not blocked)', () => {
            const keyDownQ = new KeyboardEvent('keydown', { key: 'q' });
            window.dispatchEvent(keyDownQ);

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

            expect(handler.onMoveTo).toHaveBeenCalledWith({ x: 100, y: 100 });
        });
    });

    describe('R/F blocks wheel', () => {
        it('11. R/F held → wheel.enabled = false', () => {
            const keyDownR = new KeyboardEvent('keydown', { key: 'r' });
            window.dispatchEvent(keyDownR);

            const wheel = new WheelEvent('wheel', { deltaY: -100 });
            container.dispatchEvent(wheel);

            expect(handler.onZoomBy).not.toHaveBeenCalled();
        });

        it('12. R/F released → wheel.enabled = true', () => {
            const keyDownR = new KeyboardEvent('keydown', { key: 'r' });
            window.dispatchEvent(keyDownR);
            const keyUpR = new KeyboardEvent('keyup', { key: 'r' });
            window.dispatchEvent(keyUpR);

            const wheel = new WheelEvent('wheel', { deltaY: -100 });
            container.dispatchEvent(wheel);

            expect(handler.onZoomBy).toHaveBeenCalled();
        });

        it('13. Wheel active → R/F pressed → wheel.enabled = false', () => {
            const wheel = new WheelEvent('wheel', { deltaY: -100 });
            container.dispatchEvent(wheel);
            expect(handler.onZoomBy).toHaveBeenCalled();

            const keyDownR = new KeyboardEvent('keydown', { key: 'r' });
            window.dispatchEvent(keyDownR);

            expect(handler.onZoomRate).toHaveBeenCalled();
        });
    });

    describe('Callback emission', () => {
        it('14. WASD held → onMoveRate called with normalized (x, y, sprint)', () => {
            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);

            expect(handler.onMoveRate).toHaveBeenCalledWith(0, 1, false);
        });

        it('15. Q/E held → onRotateRate called with value', () => {
            const keyDownQ = new KeyboardEvent('keydown', { key: 'q' });
            window.dispatchEvent(keyDownQ);

            expect(handler.onRotateRate).toHaveBeenCalled();
            expect(handler.onRotateRate.mock.calls[0][0]).toBeLessThan(0);
        });

        it('16. R/F held → onZoomRate called with value', () => {
            const keyDownR = new KeyboardEvent('keydown', { key: 'r' });
            window.dispatchEvent(keyDownR);

            expect(handler.onZoomRate).toHaveBeenCalled();
            expect(handler.onZoomRate.mock.calls[0][0]).toBeLessThan(0);
        });

        it('17. Left-click tap → onMoveTo called with screen position', () => {
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

            expect(handler.onMoveTo).toHaveBeenCalledWith({ x: 100, y: 200 });
        });

        it('18. Right-drag → onRotateBy called with angleDelta', () => {
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

            expect(handler.onRotateBy).toHaveBeenCalled();
            expect(typeof handler.onRotateBy.mock.calls[0][0]).toBe('number');
        });

        it('19. Wheel → onZoomBy called with delta', () => {
            const wheel = new WheelEvent('wheel', { deltaY: -100 });
            container.dispatchEvent(wheel);

            expect(handler.onZoomBy).toHaveBeenCalled();
            expect(typeof handler.onZoomBy.mock.calls[0][0]).toBe('number');
        });

        it('20. Long-press → onInteractStart/onInteract/onInteractEnd sequence', () => {
            vi.useFakeTimers();

            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1,
                bubbles: true
            });
            container.dispatchEvent(pointerDown);

            vi.advanceTimersByTime(InputConst.LONG_PRESS_MS);
            expect(handler.onInteractStart).toHaveBeenCalledWith({ x: 100, y: 100 });

            const pointerMove = new PointerEvent('pointermove', {
                clientX: 120,
                clientY: 120,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1,
                bubbles: true
            });
            container.dispatchEvent(pointerMove);
            expect(handler.onInteract).toHaveBeenCalled();

            const pointerUp = new PointerEvent('pointerup', {
                clientX: 120,
                clientY: 120,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1,
                bubbles: true
            });
            container.dispatchEvent(pointerUp);
            expect(handler.onInteractEnd).toHaveBeenCalledWith({ x: 100, y: 100 }, { x: 120, y: 120 });

            vi.useRealTimers();
        });
    });

    describe('Rate value correctness', () => {
        it('21. Diagonal WASD (W+D) → onMoveRate receives normalized unit vector', () => {
            const keyDownW = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDownW);
            const keyDownD = new KeyboardEvent('keydown', { key: 'd' });
            window.dispatchEvent(keyDownD);

            expect(handler.onMoveRate).toHaveBeenCalled();
            const lastCall = handler.onMoveRate.mock.calls[handler.onMoveRate.mock.calls.length - 1];
            const [x, y] = lastCall;
            const length = Math.sqrt(x * x + y * y);
            expect(length).toBeCloseTo(1, 5);
        });

        it('22. Q+E both held → onRotateRate receives 0 (cancel out)', () => {
            const keyDownQ = new KeyboardEvent('keydown', { key: 'q' });
            window.dispatchEvent(keyDownQ);
            const keyDownE = new KeyboardEvent('keydown', { key: 'e' });
            window.dispatchEvent(keyDownE);

            expect(handler.onRotateRate).toHaveBeenCalled();
            const lastCall = handler.onRotateRate.mock.calls[handler.onRotateRate.mock.calls.length - 1];
            expect(lastCall[0]).toBe(0);
        });

        it('23. R+F both held → onZoomRate receives 0 (cancel out)', () => {
            const keyDownR = new KeyboardEvent('keydown', { key: 'r' });
            window.dispatchEvent(keyDownR);
            const keyDownF = new KeyboardEvent('keydown', { key: 'f' });
            window.dispatchEvent(keyDownF);

            expect(handler.onZoomRate).toHaveBeenCalled();
            const lastCall = handler.onZoomRate.mock.calls[handler.onZoomRate.mock.calls.length - 1];
            expect(lastCall[0]).toBe(0);
        });
    });
});
