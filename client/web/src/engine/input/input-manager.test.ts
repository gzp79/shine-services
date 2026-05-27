// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { InputManager } from './input-manager';
import type { InputHandler } from './input-handler';
import { DesktopSchema } from './schemas/desktop-schema';
import { TouchSchema } from './schemas/touch-schema';

describe('InputManager', () => {
    let manager: InputManager;
    let handler: InputHandler;
    let container: HTMLElement;

    beforeEach(() => {
        container = document.createElement('div');
        handler = {
            onSchemaChanged: vi.fn(),
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
            onInteractEnd: vi.fn()
        };
        manager = new InputManager(handler, container);
    });

    describe('Schema tracking', () => {
        it('37. DesktopSchema becomes active → lastUsedSchema = desktop', () => {
            // Trigger desktop schema to become active by simulating keyboard input
            const keyDown = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDown);

            // Call update to poll schemas
            manager.update();

            expect(handler.onSchemaChanged).toHaveBeenCalledWith('desktop');
        });

        it('38. Schema change → handler.onSchemaChanged(\'desktop\') called', () => {
            const keyDown = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDown);

            manager.update();

            expect(handler.onSchemaChanged).toHaveBeenCalledWith('desktop');
        });

        it('39. TouchSchema becomes active → lastUsedSchema = touch, onSchemaChanged(\'touch\')', () => {
            // Trigger touch schema by simulating touch
            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchStart);

            manager.update();

            expect(handler.onSchemaChanged).toHaveBeenCalledWith('touch');
        });

        it('40. No schema active → lastUsedSchema = null', () => {
            // Initially no schema is active
            manager.update();

            // Should not have called onSchemaChanged yet (no schema became active)
            expect(handler.onSchemaChanged).not.toHaveBeenCalled();
        });
    });

    describe('Callback routing', () => {
        it('41. Active schema callback → routed to handler', () => {
            // Make desktop schema active
            const keyDown = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDown);
            manager.update();

            // Desktop schema should emit onMoveRate
            expect(handler.onMoveRate).toHaveBeenCalled();
        });

        it('42. Inactive schema callback → blocked (not routed)', () => {
            // Make desktop schema active
            const keyDown = new KeyboardEvent('keydown', { key: 'w' });
            window.dispatchEvent(keyDown);
            manager.update();

            vi.clearAllMocks();

            // Try touch schema callback (should be blocked)
            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchStart);

            const touchEnd = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch],
                touches: []
            });
            container.dispatchEvent(touchEnd);

            // Touch onMoveTo should not reach handler because desktop is active
            expect(handler.onMoveTo).not.toHaveBeenCalled();
        });

        it('43. No active schema → first callback from any schema is routed', () => {
            // No schema active initially
            // Touch schema emits callback
            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch]
            });
            container.dispatchEvent(touchStart);

            const touchEnd = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 1, clientX: 100, clientY: 100 } as Touch],
                touches: []
            });
            container.dispatchEvent(touchEnd);

            // Should be routed
            expect(handler.onMoveTo).toHaveBeenCalled();
        });

        it('44. Schema switch mid-gesture → new schema callbacks routed, old blocked', () => {
            // Desktop schema active (start drag, don't release)
            const pointerDown = new PointerEvent('pointerdown', {
                clientX: 100,
                clientY: 100,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerDown);
            manager.update();

            expect(handler.onSchemaChanged).toHaveBeenCalledWith('desktop');

            // Desktop drag continues
            const pointerMove1 = new PointerEvent('pointermove', {
                clientX: 150,
                clientY: 150,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerMove1);

            vi.clearAllMocks();

            // Try to start touch input (while desktop drag is ongoing)
            const touchStart = new TouchEvent('touchstart', {
                touches: [{ identifier: 1, clientX: 200, clientY: 200 } as Touch]
            });
            container.dispatchEvent(touchStart);

            const touchEnd = new TouchEvent('touchend', {
                changedTouches: [{ identifier: 1, clientX: 200, clientY: 200 } as Touch],
                touches: []
            });
            container.dispatchEvent(touchEnd);

            // Touch onMoveTo should be blocked (desktop is active)
            expect(handler.onMoveTo).not.toHaveBeenCalled();
            // No schema change should occur
            expect(handler.onSchemaChanged).not.toHaveBeenCalled();

            // Desktop callbacks should still be routed
            const pointerMove2 = new PointerEvent('pointermove', {
                clientX: 200,
                clientY: 200,
                button: 0,
                pointerType: 'mouse',
                pointerId: 1
            });
            container.dispatchEvent(pointerMove2);
            expect(handler.onMoveTo).toHaveBeenCalled();
        });
    });
});
