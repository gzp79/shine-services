// @vitest-environment jsdom
import { type MockedObject, afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { InputHandler } from './input-handler';
import { InputManager } from './input-manager';

// jsdom does not implement ResizeObserver — stub it so RawPointerTracker can construct
vi.stubGlobal(
    'ResizeObserver',
    class {
        observe() {}
        unobserve() {}
        disconnect() {}
    }
);

describe('InputManager', () => {
    let manager: InputManager;
    let handler: MockedObject<InputHandler>;
    let container: HTMLElement;

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
        manager = new InputManager(handler, container);
    });

    afterEach(() => {
        manager.dispose();
    });

    describe('Schema tracking', () => {
        it('37. First schema activation → onSchemaChanged not called', () => {
            window.dispatchEvent(new KeyboardEvent('keydown', { key: 'w' }));

            expect(handler.onSchemaChanged).not.toHaveBeenCalled();
        });

        it("38. Switch from desktop → touch → onSchemaChanged('touch') called", () => {
            // Activate desktop, then release so it goes idle
            window.dispatchEvent(new KeyboardEvent('keydown', { key: 'w' }));
            window.dispatchEvent(new KeyboardEvent('keyup', { key: 'w' }));

            // Activate touch
            container.dispatchEvent(
                new TouchEvent('touchstart', {
                    touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
                })
            );
            container.dispatchEvent(
                new TouchEvent('touchend', {
                    changedTouches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch],
                    touches: []
                })
            );

            expect(handler.onSchemaChanged).toHaveBeenCalledWith('touch');
        });

        it("39. Switch from touch → desktop → onSchemaChanged('desktop') called", () => {
            // Activate touch, then release so it goes idle
            container.dispatchEvent(
                new TouchEvent('touchstart', {
                    touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
                })
            );
            container.dispatchEvent(
                new TouchEvent('touchend', {
                    changedTouches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch],
                    touches: []
                })
            );

            // Activate desktop
            window.dispatchEvent(new KeyboardEvent('keydown', { key: 'w' }));

            expect(handler.onSchemaChanged).toHaveBeenCalledWith('desktop');
        });

        it('40. No input → onSchemaChanged not called', () => {
            expect(handler.onSchemaChanged).not.toHaveBeenCalled();
        });
    });

    describe('Callback routing', () => {
        it('41. First event routed immediately to handler', () => {
            window.dispatchEvent(new KeyboardEvent('keydown', { key: 'w' }));

            expect(handler.onMoveRate).toHaveBeenCalled();
        });

        it('42. Touch blocked while desktop is mid-gesture', () => {
            // Start desktop drag (do not release)
            container.dispatchEvent(
                new PointerEvent('pointerdown', {
                    clientX: 100,
                    clientY: 100,
                    button: 0,
                    pointerType: 'mouse',
                    pointerId: 1
                })
            );
            container.dispatchEvent(
                new PointerEvent('pointermove', {
                    clientX: 150,
                    clientY: 150,
                    button: 0,
                    pointerType: 'mouse',
                    pointerId: 1
                })
            );

            vi.clearAllMocks();

            // Touch while desktop drag ongoing — should be blocked
            container.dispatchEvent(
                new TouchEvent('touchstart', {
                    touches: [{ identifier: 1, clientX: 200, clientY: 200 } as Touch]
                })
            );
            container.dispatchEvent(
                new TouchEvent('touchend', {
                    changedTouches: [{ identifier: 1, clientX: 200, clientY: 200 } as Touch],
                    touches: []
                })
            );

            expect(handler.onMoveTo).not.toHaveBeenCalled();
            expect(handler.onSchemaChanged).not.toHaveBeenCalled();
        });

        it('43. Touch allowed after desktop goes idle', () => {
            // Start and finish desktop tap
            container.dispatchEvent(
                new PointerEvent('pointerdown', {
                    clientX: 100,
                    clientY: 100,
                    button: 0,
                    pointerType: 'mouse',
                    pointerId: 1
                })
            );
            container.dispatchEvent(
                new PointerEvent('pointerup', {
                    clientX: 100,
                    clientY: 100,
                    button: 0,
                    pointerType: 'mouse',
                    pointerId: 1
                })
            );

            vi.clearAllMocks();

            // Touch should now activate and route
            container.dispatchEvent(
                new TouchEvent('touchstart', {
                    touches: [{ identifier: 2, clientX: 200, clientY: 200 } as Touch]
                })
            );
            container.dispatchEvent(
                new TouchEvent('touchend', {
                    changedTouches: [{ identifier: 2, clientX: 200, clientY: 200 } as Touch],
                    touches: []
                })
            );

            expect(handler.onMoveTo).toHaveBeenCalled();
            expect(handler.onSchemaChanged).toHaveBeenCalledWith('touch');
        });

        it('44. Desktop still routed while mid-gesture after touch attempt blocked', () => {
            // Start desktop drag
            container.dispatchEvent(
                new PointerEvent('pointerdown', {
                    clientX: 100,
                    clientY: 100,
                    button: 0,
                    pointerType: 'mouse',
                    pointerId: 1
                })
            );
            container.dispatchEvent(
                new PointerEvent('pointermove', {
                    clientX: 150,
                    clientY: 150,
                    button: 0,
                    pointerType: 'mouse',
                    pointerId: 1
                })
            );

            vi.clearAllMocks();

            // Blocked touch attempt
            container.dispatchEvent(
                new TouchEvent('touchstart', {
                    touches: [{ identifier: 1, clientX: 200, clientY: 200 } as Touch]
                })
            );

            // Desktop move still routed
            container.dispatchEvent(
                new PointerEvent('pointermove', {
                    clientX: 200,
                    clientY: 200,
                    button: 0,
                    pointerType: 'mouse',
                    pointerId: 1
                })
            );

            expect(handler.onMoveTo).toHaveBeenCalled();
        });
    });
});
