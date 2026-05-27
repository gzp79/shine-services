import type { Point } from '../input-handler';
import type { InputSchema } from './input-schema';
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
export class DesktopSchema implements InputSchema {
    onMoveTo?: (screenPos: Point) => void;
    onRotateBy?: (angleDelta: number) => void;
    onZoomBy?: (delta: number) => void;
    onMoveRate?: (x: number, y: number, sprint: boolean) => void;
    onRotateRate?: (value: number) => void;
    onZoomRate?: (value: number) => void;
    onPinchStart?: (pos1: Point, pos2: Point) => void;
    onPinch?: (pos1: Point, pos2: Point) => void;
    onPinchEnd?: () => void;
    onInteractStart?: (pos: Point) => void;
    onInteract?: (pos: Point) => void;
    onInteractEnd?: (pos: Point) => void;

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

    constructor(container: HTMLElement) {

        this.wasd = new RawKeyAxis2D({ up: 'w', down: 's', left: 'a', right: 'd', sprint: 'Shift' }, window);
        this.qe = new RawKeyAxis1D({ negative: 'q', positive: 'e' }, window);
        this.rf = new RawKeyAxis1D({ negative: 'r', positive: 'f' }, window);
        this.leftPointer = new RawPointer(0, true, container);
        this.rightPointer = new RawPointer(2, false, container);
        this.wheel = new RawWheel(container);

        this.wasd.onStart = () => { this.wasdActive = true; };
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
            this.emitMoveRate();
        };
        this.wasd.onEnd = () => { this.wasdActive = false; };

        this.leftPointer.onTap = (pos) => {
            if (this.wasdActive) return;
            this.onMoveTo?.(pos);
        };
        this.leftPointer.onDragStart = (pos) => {
            if (this.wasdActive) return;
            this.wasd.enabled = false;
            this.onMoveTo?.(pos);
        };
        this.leftPointer.onDrag = (pos) => {
            if (this.wasdActive) return;
            this.onMoveTo?.(pos);
        };
        this.leftPointer.onDragEnd = (pos) => {
            if (this.wasdActive) return;
            this.onMoveTo?.(pos);
            this.wasd.enabled = true;
        };
        this.leftPointer.onLongDragStart = (pos) => {
            this.onInteractStart?.(pos);
        };
        this.leftPointer.onLongDrag = (pos) => {
            this.onInteract?.(pos);
        };
        this.leftPointer.onLongDragEnd = (pos) => {
            this.onInteractEnd?.(pos);
        };

        this.qe.onStart = () => { this.rightPointer.enabled = false; };
        this.qe.onChange = (value) => {
            this.rotate = value * ROTATE_KEY_SPEED;
            this.emitRotateRate();
        };
        this.qe.onEnd = () => { this.rightPointer.enabled = true; };

        this.rightPointer.onDragStart = () => { this.qe.enabled = false; };
        this.rightPointer.onDrag = (delta) => {
            const angleDelta = delta.x * ROTATE_SENSITIVITY;
            this.onRotateBy?.(angleDelta);
        };
        this.rightPointer.onDragEnd = () => { this.qe.enabled = true; };

        this.rf.onStart = () => { this.wheel.enabled = false; };
        this.rf.onChange = (value) => {
            this.zoom = value * ZOOM_KEY_SPEED;
            this.emitZoomRate();
        };
        this.rf.onEnd = () => { this.wheel.enabled = true; };

        this.wheel.onZoom = (delta) => {
            this.onZoomBy?.(delta * ZOOM_SENSITIVITY);
        };
    }

    isActive(): boolean {
        return this.wasd.isActive() ||
               this.qe.isActive() ||
               this.rf.isActive() ||
               this.leftPointer.isActive() ||
               this.rightPointer.isActive();
    }

    private emitMoveRate(): void {
        this.onMoveRate?.(this.moveX, this.moveY, this.sprint);
    }

    private emitRotateRate(): void {
        this.onRotateRate?.(this.rotate);
    }

    private emitZoomRate(): void {
        this.onZoomRate?.(this.zoom);
    }

    dispose(): void {
        this.wasd.dispose();
        this.qe.dispose();
        this.rf.dispose();
        this.leftPointer.dispose();
        this.rightPointer.dispose();
        this.wheel.dispose();
    }
}


