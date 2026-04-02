import { MOVE_KEYS } from '../../constants';
import { DesktopController } from './_desktop-controller';
import type { InputHandler, Point } from './_input-handler';
import { TouchController } from './_touch-controller';

export type PointerInfo = {
    id: number;
    startPos: Point;
    currentPos: Point;
    downTime: number;
    button: number;
};

export class InputController {
    private el: HTMLElement;
    private handler: InputHandler;
    private touchController: TouchController;
    private desktopController: DesktopController;
    private selected: 'touch' | 'desktop' = 'desktop';
    private active: 'touch' | 'desktop' | null = null;

    constructor(el: HTMLElement, handler: InputHandler) {
        this.el = el;
        this.handler = handler;
        this.touchController = new TouchController(handler);
        this.desktopController = new DesktopController(handler);
        this.bind();
    }

    private bind(): void {
        this.el.addEventListener('pointerdown', this.onPointerDown, { passive: false });
        this.el.addEventListener('pointermove', this.onPointerMove, { passive: false });
        this.el.addEventListener('pointerup', this.onPointerUp, { passive: false });
        this.el.addEventListener('pointercancel', this.onPointerCancel, { passive: false });
        this.el.addEventListener('pointerleave', this.onPointerLeave, { passive: false });
        this.el.addEventListener('wheel', this.onWheel, { passive: false });
        this.el.addEventListener('contextmenu', this.onContextMenu, { passive: false });
        window.addEventListener('keydown', this.onKeyDown, { passive: false });
        window.addEventListener('keyup', this.onKeyUp, { passive: false });
        window.addEventListener('blur', this.onBlur);
    }

    private unbind(): void {
        this.el.removeEventListener('pointerdown', this.onPointerDown);
        this.el.removeEventListener('pointermove', this.onPointerMove);
        this.el.removeEventListener('pointerup', this.onPointerUp);
        this.el.removeEventListener('pointercancel', this.onPointerCancel);
        this.el.removeEventListener('pointerleave', this.onPointerLeave);
        this.el.removeEventListener('wheel', this.onWheel);
        this.el.removeEventListener('contextmenu', this.onContextMenu);
        window.removeEventListener('keydown', this.onKeyDown);
        window.removeEventListener('keyup', this.onKeyUp);
        window.removeEventListener('blur', this.onBlur);
    }

    private onPointerDown = (ev: PointerEvent): void => {
        ev.preventDefault();

        const type = ev.pointerType === 'touch' ? 'touch' : 'desktop';
        if (!this.activateController(type)) {
            return;
        }
        if (this.active === 'touch') {
            this.touchController.handlePointerDown(ev);
        } else if (this.active === 'desktop') {
            this.desktopController.handlePointerDown(ev);
        }
    };

    private onPointerMove = (ev: PointerEvent): void => {
        // Only handle if we have an active controller
        // (moves are allowed even if target isn't canvas, as pointer may have moved outside)
        if (!this.active) {
            return;
        }

        ev.preventDefault();

        if (this.active === 'touch') {
            this.touchController.handlePointerMove(ev);
        } else if (this.active === 'desktop') {
            this.desktopController.handlePointerMove(ev);
        }
    };

    private onPointerUp = (ev: PointerEvent): void => {
        ev.preventDefault();

        if (this.active === 'touch') {
            this.touchController.handlePointerUp(ev);
        } else if (this.active === 'desktop') {
            this.desktopController.handlePointerUp(ev);
        }

        this.checkDeactivation();
    };

    private onPointerCancel = (ev: PointerEvent): void => {
        ev.preventDefault();

        if (this.active === 'touch') {
            this.touchController.handlePointerCancel(ev);
        } else if (this.active === 'desktop') {
            this.desktopController.handlePointerCancel(ev);
        }

        this.checkDeactivation();
    };

    private onPointerLeave = (_ev: PointerEvent): void => {
        // Pointer leaving the canvas - reset to stop all gestures and WASD movement
        // This ensures we clean up when clicking outside (e.g., debug panel)
        if (this.active === 'touch') {
            this.touchController.reset();
        } else if (this.active === 'desktop') {
            this.desktopController.reset();
        }

        this.active = null;
    };

    private onWheel = (ev: WheelEvent): void => {
        ev.preventDefault();

        if (!this.activateController('desktop')) {
            return;
        }

        if (this.active === 'desktop') {
            this.desktopController.handleWheel(ev);
        }
    };

    private onContextMenu = (ev: MouseEvent): void => {
        ev.preventDefault();
    };

    private onKeyDown = (ev: KeyboardEvent): void => {
        const key = ev.key.toLowerCase();

        // ESC resets all controllers
        if (key === 'escape') {
            ev.preventDefault();
            if (this.active === 'touch') {
                this.touchController.reset();
            } else if (this.active === 'desktop') {
                this.desktopController.reset();
            }
            this.active = null;
            return;
        }

        if (!MOVE_KEYS.includes(key)) {
            return;
        }

        if (!this.activateController('desktop')) {
            return;
        }

        if (this.active === 'desktop') {
            this.desktopController.handleKeyDown(ev);
        }
    };

    private onKeyUp = (ev: KeyboardEvent): void => {
        const key = ev.key.toLowerCase();
        if (!MOVE_KEYS.includes(key)) {
            return;
        }

        if (this.active === 'desktop') {
            this.desktopController.handleKeyUp(ev);
        }

        this.checkDeactivation();
    };

    private onBlur = (): void => {
        if (this.active === 'touch') {
            this.touchController.reset();
        } else if (this.active === 'desktop') {
            this.desktopController.reset();
        }
        this.active = null;
    };

    private activateController(type: 'touch' | 'desktop'): boolean {
        if (this.active !== null && this.active !== type) {
            if (this.active === 'touch' && this.touchController.isActive()) {
                return false;
            }
            if (this.active === 'desktop' && this.desktopController.isActive()) {
                return false;
            }
        }

        if (this.active !== type) {
            this.active = type;
            if (this.selected !== type) {
                this.selected = type;
                this.handler.onControllerChanged(type);
            }
        }

        return true;
    }

    private checkDeactivation(): void {
        if (this.active === 'touch' && !this.touchController.isActive()) {
            this.active = null;
        } else if (this.active === 'desktop' && !this.desktopController.isActive()) {
            this.active = null;
        }
    }

    dispose(): void {
        this.unbind();
        this.onBlur();
    }
}
