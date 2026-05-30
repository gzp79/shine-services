import type { Point } from '../input-handler';
import { MOVE_THRESHOLD_PX, LONG_PRESS_MS } from '../../../constants';

/**
 * RawSingleTouch handles single-finger touch gestures.
 * Subscribes to touch events on the provided target.
 *
 * Gesture detection:
 * - Tap: touch → release without movement, within threshold time
 * - Drag: touch → move past threshold → release
 * - Long drag: touch → wait (no movement) → move → release
 */
export class RawSingleTouch {
    onTap?: (pos: Point) => void;
    onDragStart?: (start: Point) => void;
    onDrag?: (start: Point, prev: Point, current: Point) => void;
    onDragEnd?: (start: Point, end: Point) => void;
    onLongDragStart?: (start: Point) => void;
    onLongDrag?: (start: Point, prev: Point, current: Point) => void;
    onLongDragEnd?: (start: Point, end: Point) => void;

    private _enabled = true;
    private touchId: number | null = null;
    private startPos: Point | null = null;
    private lastPos: Point | null = null;
    private movedPastThreshold = false;
    private longPressTimer: number | null = null;
    private isLongDrag = false;

    constructor(private readonly target: HTMLElement) {
        this.target.addEventListener('touchstart', this.handleTouchStart);
        this.target.addEventListener('touchmove', this.handleTouchMove);
        this.target.addEventListener('touchend', this.handleTouchEnd);
        this.target.addEventListener('touchcancel', this.handleTouchCancel);
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
        return this.touchId !== null;
    }

    cancel(): void {
        if (this.touchId === null) return;
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
        this.target.removeEventListener('touchstart', this.handleTouchStart);
        this.target.removeEventListener('touchmove', this.handleTouchMove);
        this.target.removeEventListener('touchend', this.handleTouchEnd);
        this.target.removeEventListener('touchcancel', this.handleTouchCancel);
    }

    private handleTouchStart = (ev: TouchEvent): void => {
        if (!this.enabled || this.touchId !== null || ev.touches.length !== 1) return;

        const touch = ev.touches[0];
        this.touchId = touch.identifier;
        this.startPos = { x: touch.clientX, y: touch.clientY };
        this.lastPos = { x: touch.clientX, y: touch.clientY };
        this.movedPastThreshold = false;
        this.isLongDrag = false;

        this.longPressTimer = window.setTimeout(() => {
            if (this.touchId !== null && !this.movedPastThreshold && this.startPos) {
                this.isLongDrag = true;
                this.onLongDragStart?.(this.startPos);
            }
        }, LONG_PRESS_MS);
    };

    private handleTouchMove = (ev: TouchEvent): void => {
        if (!this.enabled || this.touchId === null) return;

        const touch = Array.from(ev.touches).find(t => t.identifier === this.touchId);
        if (!touch || !this.startPos || !this.lastPos) return;

        const currentPos = { x: touch.clientX, y: touch.clientY };
        const dx = currentPos.x - this.startPos.x;
        const dy = currentPos.y - this.startPos.y;
        const distance = Math.sqrt(dx * dx + dy * dy);

        if (!this.movedPastThreshold && distance >= MOVE_THRESHOLD_PX) {
            this.movedPastThreshold = true;
            this.cancelLongPressTimer();

            if (!this.isLongDrag) {
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

    private handleTouchEnd = (ev: TouchEvent): void => {
        if (this.touchId === null) return;

        const touch = Array.from(ev.changedTouches).find(t => t.identifier === this.touchId);
        if (!touch) return;

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

    private handleTouchCancel = (ev: TouchEvent): void => {
        if (this.touchId === null) return;
        const touch = Array.from(ev.changedTouches).find(t => t.identifier === this.touchId);
        if (touch) this.cancel();
    };

    private reset(): void {
        this.cancelLongPressTimer();
        this.touchId = null;
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
