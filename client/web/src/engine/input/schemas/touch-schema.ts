import type { InputHandler } from '../input-handler';
import { RawSingleTouch } from '../raw/raw-single-touch';
import { RawTwoFingerGesture } from '../raw/raw-two-finger-gesture';
import { InputSchema } from './input-schema';

/**
 * TouchSchema handles touch input (single and two-finger gestures).
 * Built from generic raw inputs (RawSingleTouch, RawTwoFingerGesture).
 *
 * Conflict rules (first to start wins):
 * - Single-finger ↔ two-finger gestures
 */
export class TouchSchema extends InputSchema {
    private readonly singleTouch: RawSingleTouch;
    private readonly twoFingerGesture: RawTwoFingerGesture;

    constructor(container: HTMLElement, handler?: InputHandler) {
        super('touch', handler);

        this.singleTouch = new RawSingleTouch(container);
        this.twoFingerGesture = new RawTwoFingerGesture(container);

        // Wire single-finger gestures
        this.singleTouch.onTap = (pos) => {
            this.activate();
            this.handler?.onMoveTo(pos);
        };
        this.singleTouch.onDragStart = (start) => {
            this.activate();
            this.twoFingerGesture.enabled = false;
            this.handler?.onMoveTo(start);
        };
        this.singleTouch.onDrag = (_start, _prev, current) => {
            this.handler?.onMoveTo(current);
        };
        this.singleTouch.onDragEnd = (_start, end) => {
            this.handler?.onMoveTo(end);
            this.twoFingerGesture.enabled = true;
        };
        this.singleTouch.onLongDragStart = (start) => {
            this.activate();
            this.twoFingerGesture.enabled = false;
            this.handler?.onInteractStart(start);
        };
        this.singleTouch.onLongDrag = (start, prev, current) => {
            this.handler?.onInteract(start, prev, current);
        };
        this.singleTouch.onLongDragEnd = (start, end) => {
            this.handler?.onInteractEnd(start, end);
            this.twoFingerGesture.enabled = true;
        };

        // Wire two-finger gestures
        this.twoFingerGesture.onPinchStart = (start) => {
            this.activate();
            this.singleTouch.enabled = false;
            this.handler?.onPinchStart(start);
        };
        this.twoFingerGesture.onPinch = (start, prev, current) => {
            this.handler?.onPinch(start, prev, current);
        };
        this.twoFingerGesture.onPinchEnd = (start, end) => {
            this.handler?.onPinchEnd(start, end);
        };
        this.twoFingerGesture.onAllFingersReleased = () => {
            this.singleTouch.enabled = true;
        };
    }

    get isIdle(): boolean {
        return !this.singleTouch.isActive() && !this.twoFingerGesture.isActive();
    }

    state(): Record<string, string> {
        const en = (v: boolean) => (v ? 'on' : 'off');
        const ac = (v: boolean) => (v ? ' [active]' : '');
        return {
            idle: String(this.isIdle),
            single: `${en(this.singleTouch.enabled)}${ac(this.singleTouch.isActive())}`,
            two: `${en(this.twoFingerGesture.enabled)}${ac(this.twoFingerGesture.isActive())}`
        };
    }

    cancel(): void {
        this.singleTouch.cancel();
        this.twoFingerGesture.cancel();
    }

    dispose(): void {
        this.singleTouch.dispose();
        this.twoFingerGesture.dispose();
    }
}
