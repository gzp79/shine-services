import type { Point } from '../input-handler';
import { MOVE_THRESHOLD_PX, LONG_PRESS_MS } from '../../../constants';

/**
 * RawPointer handles pointer gestures for a single button.
 * Works with mouse and pen only (touch is handled by RawTouch).
 * Subscribes to pointer events on the provided target.
 *
 * Gesture detection:
 * - Tap: down → up without movement, within threshold time
 * - Drag: down → move past threshold → up
 * - Long drag: down → wait (no movement) → move → up
 */
export class RawPointer {
    onTap?: (pos: Point) => void;
    onDragStart?: (pos: Point) => void;
    onDrag?: (start: Point, prev: Point, current: Point) => void;
    onDragEnd?: (start: Point, end: Point) => void;
    onLongDragStart?: (pos: Point) => void;
    onLongDrag?: (start: Point, prev: Point, current: Point) => void;
    onLongDragEnd?: (start: Point, end: Point) => void;

    private _enabled = true;
    private pointerId: number | null = null;
    private startPos: Point | null = null;
    private lastPos: Point | null = null;
    private movedPastThreshold = false;
    private longPressTimer: number | null = null;
    private isLongDrag = false;

    constructor(
        private readonly button: number, // 0=left, 1=middle, 2=right
        private readonly enableLongDrag: boolean,
        private readonly target: HTMLElement,
    ) {
        this.target.addEventListener('pointerdown', this.handlePointerDown);
        this.target.addEventListener('pointermove', this.handlePointerMove);
        this.target.addEventListener('pointerup', this.handlePointerUp);
        this.target.addEventListener('pointercancel', this.handlePointerCancel);
    }

    get enabled(): boolean {
        return this._enabled;
    }

    set enabled(value: boolean) {
        if (this._enabled === value) return;

        this._enabled = value;

        if (!value) {
            this.reset();
        }
    }

    isActive(): boolean {
        return this.pointerId !== null;
    }

    cancel(): void {
        if (this.pointerId === null) return;
        this.cancelLongPressTimer();
        if (this.movedPastThreshold && this.startPos && this.lastPos) {
            if (this.isLongDrag) {
                this.onLongDragEnd?.(this.startPos, this.lastPos);
            } else {
                this.onDragEnd?.(this.startPos, this.lastPos);
            }
        }
        this.reset();
    }

    dispose(): void {
        this.target.removeEventListener('pointerdown', this.handlePointerDown);
        this.target.removeEventListener('pointermove', this.handlePointerMove);
        this.target.removeEventListener('pointerup', this.handlePointerUp);
        this.target.removeEventListener('pointercancel', this.handlePointerCancel);
    }

    private handlePointerDown = (ev: PointerEvent): void => {
        // Exclude touch - only handle mouse and pen
        if (ev.pointerType === 'touch') return;
        if (!this.enabled || ev.button !== this.button || this.pointerId !== null) return;

        this.pointerId = ev.pointerId;
        this.startPos = { x: ev.clientX, y: ev.clientY };
        this.lastPos = { x: ev.clientX, y: ev.clientY };
        this.movedPastThreshold = false;
        this.isLongDrag = false;

        // Start long press timer if enabled
        if (this.enableLongDrag) {
            this.longPressTimer = window.setTimeout(() => {
                if (this.pointerId !== null && !this.movedPastThreshold) {
                    this.isLongDrag = true;
                    if (this.startPos) {
                        this.onLongDragStart?.(this.startPos);
                    }
                }
            }, LONG_PRESS_MS);
        }
    };

    private handlePointerMove = (ev: PointerEvent): void => {
        if (ev.pointerType === 'touch') return;
        if (!this.enabled || this.pointerId === null || ev.pointerId !== this.pointerId) return;
        if (!this.startPos || !this.lastPos) return;

        const currentPos = { x: ev.clientX, y: ev.clientY };
        const dx = currentPos.x - this.startPos.x;
        const dy = currentPos.y - this.startPos.y;
        const distance = Math.sqrt(dx * dx + dy * dy);

        // Check if moved past threshold
        if (!this.movedPastThreshold && distance >= MOVE_THRESHOLD_PX) {
            this.movedPastThreshold = true;
            this.cancelLongPressTimer();

            if (this.isLongDrag) {
                this.onLongDrag?.(this.startPos, this.lastPos, currentPos);
            } else {
                this.onDragStart?.(currentPos);
            }
        }

        if (this.movedPastThreshold) {
            if (this.isLongDrag) {
                this.onLongDrag?.(this.startPos, this.lastPos, currentPos);
            } else {
                this.onDrag?.(this.startPos, this.lastPos, currentPos);
            }
        }

        this.lastPos = currentPos;
    };

    private handlePointerUp = (ev: PointerEvent): void => {
        if (ev.pointerType === 'touch') return;
        if (this.pointerId === null || ev.pointerId !== this.pointerId) return;

        this.cancelLongPressTimer();

        if (!this.movedPastThreshold && this.startPos) {
            this.onTap?.(this.startPos);
        } else if (this.movedPastThreshold && this.startPos && this.lastPos) {
            if (this.isLongDrag) {
                this.onLongDragEnd?.(this.startPos, this.lastPos);
            } else {
                this.onDragEnd?.(this.startPos, this.lastPos);
            }
        }

        this.reset();
    };

    private handlePointerCancel = (ev: PointerEvent): void => {
        if (ev.pointerType === 'touch') return;
        if (this.pointerId !== null && ev.pointerId === this.pointerId) {
            this.reset();
        }
    };

    private reset(): void {
        this.cancelLongPressTimer();
        this.pointerId = null;
        this.startPos = null;
        this.lastPos = null;
        this.movedPastThreshold = false;
        this.isLongDrag = false;
    }

    private cancelLongPressTimer(): void {
        if (this.longPressTimer !== null) {
            clearTimeout(this.longPressTimer);
            this.longPressTimer = null;
        }
    }
}
