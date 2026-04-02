import { EventDispatcher } from '../events';
import { MOVE_KEYS } from './constants';
import { DesktopController } from './desktop-controller';
import { INPUT_CONTROLLER_CHANGED, type InputControllerChangedEvent } from './events';
import { TouchController } from './touch-controller';

export class InputController {
    private el: HTMLElement;
    private dispatcher: EventDispatcher;
    private touchController: TouchController;
    private desktopController: DesktopController;
    private selected: 'touch' | 'desktop' = 'desktop';
    private active: 'touch' | 'desktop' | null = null;

    constructor(el: HTMLElement, events: EventTarget) {
        this.el = el;
        this.dispatcher = new EventDispatcher(events);
        this.touchController = new TouchController(this.dispatcher);
        this.desktopController = new DesktopController(this.dispatcher);
        this.bind();
    }

    private bind(): void {
        this.el.addEventListener('pointerdown', this.onPointerDown, { passive: false });
        this.el.addEventListener('pointermove', this.onPointerMove, { passive: false });
        this.el.addEventListener('pointerup', this.onPointerUp, { passive: false });
        this.el.addEventListener('pointercancel', this.onPointerCancel, { passive: false });
        this.el.addEventListener('wheel', this.onWheel, { passive: false });
        window.addEventListener('keydown', this.onKeyDown, { passive: false });
        window.addEventListener('keyup', this.onKeyUp, { passive: false });
        window.addEventListener('blur', this.onBlur);
    }

    private unbind(): void {
        this.el.removeEventListener('pointerdown', this.onPointerDown);
        this.el.removeEventListener('pointermove', this.onPointerMove);
        this.el.removeEventListener('pointerup', this.onPointerUp);
        this.el.removeEventListener('pointercancel', this.onPointerCancel);
        this.el.removeEventListener('wheel', this.onWheel);
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

    private onWheel = (ev: WheelEvent): void => {
        ev.preventDefault();

        if (!this.activateController('desktop')) {
            return;
        }

        if (this.active === 'desktop') {
            this.desktopController.handleWheel(ev);
        }
    };

    private onKeyDown = (ev: KeyboardEvent): void => {
        const key = ev.key.toLowerCase();
        if (!MOVE_KEYS.includes(key)) {
            return; // Ignore non-move keys
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
                this.dispatcher.dispatch<InputControllerChangedEvent>(INPUT_CONTROLLER_CHANGED, {
                    controller: type
                });
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
