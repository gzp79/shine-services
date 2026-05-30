import type { PointPair, Point } from '../input-handler';

/**
 * RawTwoFingerGesture handles two-finger touch gestures (pinch, two-finger pan).
 * Subscribes to touch events on the provided target.
 */
export class RawTwoFingerGesture {
    onPinchStart?: (start: PointPair) => void;
    onPinch?: (start: PointPair, prev: PointPair, current: PointPair) => void;
    onPinchEnd?: (start: PointPair, end: PointPair) => void;
    onAllFingersReleased?: () => void;

    private _enabled = true;
    private firstTouchId: number | null = null;
    private secondTouchId: number | null = null;
    private touches: Map<number, Point> = new Map();
    private active = false;
    private everStarted = false;
    private startPair: PointPair | null = null;
    private lastPair: PointPair | null = null;

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
            this.cancel();
        }
    }

    cancel(): void {
        if (!this.active) return;
        const startPair = this.startPair!;
        const lastPair = this.lastPair!;
        this.active = false;
        this.everStarted = false;
        this.firstTouchId = null;
        this.secondTouchId = null;
        this.touches.clear();
        this.startPair = null;
        this.lastPair = null;
        this.onPinchEnd?.(startPair, lastPair);
        this.onAllFingersReleased?.();
    }

    isActive(): boolean {
        return this.active;
    }

    dispose(): void {
        this.target.removeEventListener('touchstart', this.handleTouchStart);
        this.target.removeEventListener('touchmove', this.handleTouchMove);
        this.target.removeEventListener('touchend', this.handleTouchEnd);
        this.target.removeEventListener('touchcancel', this.handleTouchCancel);
    }

    private handleTouchStart = (ev: TouchEvent): void => {
        if (!this.enabled) return;

        // Track the first two touches that appear
        for (let i = 0; i < ev.touches.length; i++) {
            const touch = ev.touches[i];

            if (this.firstTouchId === null) {
                this.firstTouchId = touch.identifier;
                this.touches.set(touch.identifier, { x: touch.clientX, y: touch.clientY });
            } else if (this.secondTouchId === null && touch.identifier !== this.firstTouchId) {
                this.secondTouchId = touch.identifier;
                this.touches.set(touch.identifier, { x: touch.clientX, y: touch.clientY });
                break;
            }
        }

        // Emit onPinchStart when transitioning from 1 to 2 fingers
        if (this.firstTouchId !== null && this.secondTouchId !== null && !this.active) {
            this.active = true;
            this.everStarted = true;
            const [p1, p2] = Array.from(this.touches.values());
            this.startPair = [p1, p2];
            this.lastPair = [p1, p2];
            this.onPinchStart?.(this.startPair);
        }
    };

    private handleTouchMove = (ev: TouchEvent): void => {
        if (!this.enabled || !this.active) return;

        // Update only the two tracked touches
        for (let i = 0; i < ev.touches.length; i++) {
            const touch = ev.touches[i];
            if (touch.identifier === this.firstTouchId || touch.identifier === this.secondTouchId) {
                this.touches.set(touch.identifier, { x: touch.clientX, y: touch.clientY });
            }
        }

        if (this.touches.size === 2) {
            const prev = this.lastPair!;
            const [p1, p2] = Array.from(this.touches.values());
            const current: PointPair = [p1, p2];
            this.lastPair = current;
            this.onPinch?.(this.startPair!, prev, current);
        }
    };

    private handleTouchEnd = (ev: TouchEvent): void => {
        // Check if one of our tracked touches ended
        for (let i = 0; i < ev.changedTouches.length; i++) {
            const touch = ev.changedTouches[i];
            if (touch.identifier === this.firstTouchId) {
                this.firstTouchId = null;
                this.touches.delete(touch.identifier);
            } else if (touch.identifier === this.secondTouchId) {
                this.secondTouchId = null;
                this.touches.delete(touch.identifier);
            }
        }

        // Emit onPinchEnd when going from 2 fingers to 1 finger (either tracked touch lifted)
        if (this.active && (this.firstTouchId === null || this.secondTouchId === null)) {
            this.active = false;
            this.onPinchEnd?.(this.startPair!, this.lastPair!);
        }

        // Emit onAllFingersReleased when ALL fingers are released (0 touches)
        if (this.everStarted && this.firstTouchId === null && this.secondTouchId === null) {
            this.everStarted = false;
            this.touches.clear();
            this.startPair = null;
            this.lastPair = null;
            this.onAllFingersReleased?.();
        }
    };

    private handleTouchCancel = (ev: TouchEvent): void => {
        this.handleTouchEnd(ev);
    };
}
