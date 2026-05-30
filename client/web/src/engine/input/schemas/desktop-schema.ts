import type { InputHandler } from '../input-handler';
import { InputSchema } from './input-schema';
import { RawKeyAxis2D } from '../raw/raw-key-axis-2d';
import { RawKeyAxis1D } from '../raw/raw-key-axis-1d';
import { RawPointer } from '../raw/raw-pointer';
import { RawWheel } from '../raw/raw-wheel';
import { ROTATE_KEY_SPEED, ROTATE_SENSITIVITY, ZOOM_KEY_SPEED, ZOOM_SENSITIVITY } from '../../../constants';

/**
 * DesktopSchema handles desktop input (keyboard + mouse).
 * Built from generic raw inputs (RawKeyAxis2D, RawKeyAxis1D, RawPointer).
 *
 * Conflict rules (first to start wins):
 * - WASD ↔ left-drag-pan
 * - Q/E ↔ right-drag-rotate
 * - R/F ↔ wheel
 */
export class DesktopSchema extends InputSchema {
    private moveX = 0;
    private moveY = 0;
    private rotate = 0;
    private zoom = 0;
    private sprint = false;
    private wasdActive = false;

    private readonly wasd: RawKeyAxis2D;
    private readonly qe: RawKeyAxis1D;
    private readonly rf: RawKeyAxis1D;
    private readonly leftPointer: RawPointer;
    private readonly rightPointer: RawPointer;
    private readonly wheel: RawWheel;
    private readonly container: HTMLElement;

    constructor(container: HTMLElement, handler?: InputHandler) {
        super('desktop', handler);
        this.container = container;
        container.addEventListener('contextmenu', this.handleContextMenu);

        this.wasd = new RawKeyAxis2D({ up: 'w', down: 's', left: 'a', right: 'd', sprint: 'Shift' }, window);
        this.qe = new RawKeyAxis1D({ negative: 'q', positive: 'e' }, window);
        this.rf = new RawKeyAxis1D({ negative: 'r', positive: 'f' }, window);
        this.leftPointer = new RawPointer(0, true, container);
        this.rightPointer = new RawPointer(2, false, container);
        this.wheel = new RawWheel(container);

        this.wasd.onStart = () => {
            this.activate();
            this.wasdActive = true;
        };
        this.wasd.onChange = (x, y, sprint) => {
            const lengthSquared = x * x + y * y;
            if (lengthSquared > 0) {
                const length = Math.sqrt(lengthSquared);
                this.moveX = x / length;
                this.moveY = y / length;
            } else {
                this.moveX = 0;
                this.moveY = 0;
            }
            this.sprint = sprint;
            this.handler?.onMoveRate(this.moveX, this.moveY, this.sprint);
        };
        this.wasd.onEnd = () => { this.wasdActive = false; };

        this.leftPointer.onTap = (pos) => {
            if (this.wasdActive) return;
            this.activate();
            this.handler?.onMoveTo(pos);
        };
        this.leftPointer.onDragStart = (pos) => {
            if (this.wasdActive) return;
            this.activate();
            this.wasd.enabled = false;
            this.handler?.onMoveTo(pos);
        };
        this.leftPointer.onDrag = (_start, _prev, current) => {
            if (this.wasdActive) return;
            this.handler?.onMoveTo(current);
        };
        this.leftPointer.onDragEnd = (_start, end) => {
            if (this.wasdActive) return;
            this.handler?.onMoveTo(end);
            this.wasd.enabled = true;
        };
        this.leftPointer.onLongDragStart = (start) => {
            this.activate();
            this.handler?.onInteractStart(start);
        };
        this.leftPointer.onLongDrag = (start, prev, current) => {
            this.handler?.onInteract(start, prev, current);
        };
        this.leftPointer.onLongDragEnd = (start, end) => {
            this.handler?.onInteractEnd(start, end);
        };

        this.qe.onStart = () => {
            this.activate();
            this.rightPointer.enabled = false;
        };
        this.qe.onChange = (value) => {
            this.rotate = value * ROTATE_KEY_SPEED;
            this.handler?.onRotateRate(this.rotate);
        };
        this.qe.onEnd = () => { this.rightPointer.enabled = true; };

        this.rightPointer.onDragStart = () => {
            this.activate();
            this.qe.enabled = false;
        };
        this.rightPointer.onDrag = (delta) => {
            const angleDelta = delta.x * ROTATE_SENSITIVITY;
            this.handler?.onRotateBy(angleDelta);
        };
        this.rightPointer.onDragEnd = () => { this.qe.enabled = true; };

        this.rf.onStart = () => {
            this.activate();
            this.wheel.enabled = false;
        };
        this.rf.onChange = (value) => {
            this.zoom = value * ZOOM_KEY_SPEED;
            this.handler?.onZoomRate(this.zoom);
        };
        this.rf.onEnd = () => { this.wheel.enabled = true; };

        this.wheel.onZoom = (delta) => {
            this.activate();
            this.handler?.onZoomBy(delta * ZOOM_SENSITIVITY);
        };
    }

    get isIdle(): boolean {
        return !this.wasd.isActive() &&
               !this.qe.isActive() &&
               !this.rf.isActive() &&
               !this.leftPointer.isActive() &&
               !this.rightPointer.isActive();
    }

    state(): string {
        const en = (v: boolean) => v ? 'on ' : 'off';
        const ac = (v: boolean) => v ? ' [active]' : '';
        return [
            `idle:  ${this.isIdle}`,
            `wasd:  ${en(this.wasd.enabled)}${ac(this.wasd.isActive())}`,
            `qe:    ${en(this.qe.enabled)}${ac(this.qe.isActive())}`,
            `rf:    ${en(this.rf.enabled)}${ac(this.rf.isActive())}`,
            `left:  ${en(this.leftPointer.enabled)}${ac(this.leftPointer.isActive())}`,
            `right: ${en(this.rightPointer.enabled)}${ac(this.rightPointer.isActive())}`,
            `wheel: ${en(this.wheel.enabled)}`,
        ].join('\n');
    }

    cancel(): void {
        this.wasd.cancel();
        this.qe.cancel();
        this.rf.cancel();
        this.leftPointer.cancel();
        this.rightPointer.cancel();
    }

    dispose(): void {
        this.container.removeEventListener('contextmenu', this.handleContextMenu);
        this.wasd.dispose();
        this.qe.dispose();
        this.rf.dispose();
        this.leftPointer.dispose();
        this.rightPointer.dispose();
        this.wheel.dispose();
    }

    private handleContextMenu = (e: Event): void => {
        e.preventDefault();
    };
}
